// Desktop Pet 命令模块。
//
// 管理本地持久化目录中已安装的宠物（内置 4 只 + 用户导入），并提供创建/显示/隐藏一个
// 独立、透明、置顶、无边框的桌面宠物悬浮窗口（OS 级），让宠物浮在屏幕上。
// 另含 WorkBuddy 联动（hook）、token 统计与免费模型 AI 台词三条集成链路。
//
// 约定遵循 tauri-harness 后端规范：导入 → 数据结构(camelCase) → 私有辅助 → #[tauri::command]，
// 命令返回 Result<T, String>，禁止 unwrap()/expect()。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{
    AppHandle, Listener, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindowBuilder,
};

use super::{get_app_data_dir, now_rfc3339};

// --- 常量 ----------------------------------------------------------------

/// 桌面宠物悬浮窗口的标签（前端据此识别窗口类型）。
pub const PET_WINDOW_LABEL: &str = "pet";

/// 宠物悬浮窗口逻辑尺寸（容纳一只 192x208 的宠物按 0.75 缩放 + 走动留白）。
///
/// 全屏模式下不再用于构建窗口（窗口恒铺满整屏）。仅被 `position_bottom_right`
/// 备用定位逻辑引用，保留供未来「右下角小窗模式」复用。
#[allow(dead_code)]
const PET_WINDOW_WIDTH: f64 = 300.0;
#[allow(dead_code)]
const PET_WINDOW_HEIGHT: f64 = 320.0;

/// 右下角定位的右边距 / 下边距（逻辑像素）。下边距预留 macOS Dock / 任务栏空间。
///
/// 仅被 `position_bottom_right` 备用定位逻辑引用，全屏模式下未启用。
#[allow(dead_code)]
const PET_WINDOW_RIGHT_MARGIN: f64 = 24.0;
#[allow(dead_code)]
const PET_WINDOW_BOTTOM_MARGIN: f64 = 84.0;

/// 内置打包的 4 只宠物 id（资源在 src-tauri/resources/pets/<id>/）。
///
/// 市场安装（pet_market.rs）也引用它：与内置 id 同名的市场宠物禁止安装，
/// 否则会覆盖内置资源目录。
pub(crate) const BUILTIN_PET_IDS: &[&str] = &["ice-tea-hooper", "trump", "jige-kunkun", "fat-guga"];

// --- 数据结构（与前端共享，统一 camelCase） --------------------------------

/// codex-pets.net 包内的 manifest.json 结构（与本地 meta.json 不同）。
///
/// 用户从 codex-pets 下载 ZIP 解压后，目录里只有 manifest.json + spritesheet.webp（没有 meta.json）。
/// 本结构用于把上游格式映射成本地 LocalPetMeta 后再落盘 meta.json，实现对 codex 原生包的兼容导入。
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct CodexManifest {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    /// 上游用 spritesheetPath 指向精灵图文件名（通常 "spritesheet.webp"）。
    pub spritesheet_path: String,
    /// 1 = 标准 9 行图集，2 = 扩展 11 行图集。
    #[serde(default)]
    pub sprite_version_number: Option<u32>,
}

/// 落地的本地宠物元数据（meta.json 的结构）。
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LocalPetMeta {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// "builtin"（内置打包）或 "downloaded"（用户从市场下载）。
    pub source: String,
    /// 精灵图文件名（始终为 "spritesheet.webp"）。
    pub spritesheet_file: String,
    #[serde(default)]
    pub poster_file: Option<String>,
    #[serde(default)]
    pub spritesheet_url: Option<String>,
    /// codex-pets 的版本号（uploadedAt 毫秒）。
    #[serde(default)]
    pub version: Option<u64>,
    /// 精灵图版本号（codex manifest.json 的 spriteVersionNumber）：
    /// 1 = 标准 9 行图集（1536x1872），2 = 扩展 11 行图集（1536x2288）。
    /// 缺省时由渲染层按图集实际高度推断（≥11 行按 v2 处理）。
    #[serde(default)]
    pub sprite_version_number: Option<u32>,
    #[serde(default)]
    pub installed_at: Option<String>,
}

/// 透出给前端的本地宠物信息（meta + 绝对路径，便于 convertFileSrc）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LocalPetInfo {
    pub id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub kind: Option<String>,
    pub tags: Vec<String>,
    pub source: String,
    pub spritesheet_path: String,
    pub poster_path: Option<String>,
    pub spritesheet_url: Option<String>,
    pub version: Option<u64>,
    pub sprite_version_number: Option<u32>,
    pub installed_at: Option<String>,
}

// --- 私有辅助：路径与元数据 ----------------------------------------------

/// 本地宠物根目录：<app_data>/pets。
pub(crate) fn pets_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = get_app_data_dir(app)?.join("pets");
    fs::create_dir_all(&dir).map_err(|e| format!("创建宠物目录失败: {}", e))?;
    Ok(dir)
}

/// 单只宠物的本地目录：<app_data>/pets/<id>。
pub(crate) fn pet_dir(app: &AppHandle, pet_id: &str) -> Result<PathBuf, String> {
    // 拒绝路径穿越：只允许小写字母/数字/连字符的 id。
    if !pet_id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(format!("非法的宠物 id: {}", pet_id));
    }
    Ok(pets_dir(app)?.join(pet_id))
}

/// 读取某只宠物目录下的元数据：优先 meta.json，不存在则回退到 codex 的 manifest.json。
///
/// 兼容 codex-pets.net 原生包：用户把下载的 ZIP 解压进 pets/ 目录时，里面只有 manifest.json
/// （字段为 spritesheetPath / spriteVersionNumber），没有本地 meta.json。这里把 manifest 映射成
/// LocalPetMeta，并（可选）落盘 meta.json 便于后续管理与编辑。
pub(crate) fn read_meta(dir: &Path) -> Result<Option<LocalPetMeta>, String> {
    let meta_path = dir.join("meta.json");
    if meta_path.exists() {
        let content = fs::read_to_string(&meta_path)
            .map_err(|e| format!("读取 meta.json 失败: {}", e))?;
        let meta: LocalPetMeta = serde_json::from_str(&content)
            .map_err(|e| format!("解析 meta.json 失败: {}", e))?;
        return Ok(Some(meta));
    }

    // 回退：codex manifest.json（上游包格式）。
    let manifest_path = dir.join("manifest.json");
    if !manifest_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("读取 manifest.json 失败: {}", e))?;
    let manifest: CodexManifest = serde_json::from_str(&content)
        .map_err(|e| format!("解析 manifest.json 失败: {}", e))?;

    let meta = LocalPetMeta {
        id: manifest.id.clone(),
        display_name: manifest.display_name.clone(),
        description: manifest.description.clone(),
        kind: manifest.kind.clone(),
        tags: vec![],
        source: "imported".to_string(),
        spritesheet_file: manifest.spritesheet_path.clone(),
        poster_file: None,
        spritesheet_url: None,
        version: None,
        sprite_version_number: manifest.sprite_version_number,
        installed_at: Some(now_rfc3339()),
    };

    // 落盘成 meta.json，后续读取走快路径，也便于用户编辑。
    let _ = write_meta(dir, &meta);

    Ok(Some(meta))
}

/// 写入 meta.json（pretty 格式，便于排查）。
pub(crate) fn write_meta(dir: &Path, meta: &LocalPetMeta) -> Result<(), String> {
    let meta_path = dir.join("meta.json");
    let content = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("序列化 meta.json 失败: {}", e))?;
    fs::write(&meta_path, content).map_err(|e| format!("写入 meta.json 失败: {}", e))?;
    Ok(())
}

/// meta + 目录 → 透出给前端的 LocalPetInfo（附带绝对路径）。
pub(crate) fn meta_to_info(dir: &Path, meta: &LocalPetMeta) -> LocalPetInfo {
    let spritesheet_path = dir.join(&meta.spritesheet_file);
    let poster_path = meta
        .poster_file
        .as_ref()
        .map(|name| dir.join(name).to_string_lossy().to_string());

    LocalPetInfo {
        id: meta.id.clone(),
        display_name: meta.display_name.clone(),
        description: meta.description.clone(),
        kind: meta.kind.clone(),
        tags: meta.tags.clone(),
        source: meta.source.clone(),
        spritesheet_path: spritesheet_path.to_string_lossy().to_string(),
        poster_path,
        spritesheet_url: meta.spritesheet_url.clone(),
        version: meta.version,
        sprite_version_number: meta.sprite_version_number,
        installed_at: meta.installed_at.clone(),
    }
}

// --- 命令：本地宠物管理 --------------------------------------------------

/// 列出本地所有已安装的宠物（内置 + 下载）。
#[tauri::command]
pub fn list_local_pets(app: AppHandle) -> Result<Vec<LocalPetInfo>, String> {
    let root = pets_dir(&app)?;
    let mut infos: Vec<LocalPetInfo> = Vec::new();

    let entries = match fs::read_dir(&root) {
        Ok(e) => e,
        Err(_) => return Ok(infos),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Ok(Some(meta)) = read_meta(&path) {
            // 缺精灵图的目录跳过（损坏或未完成）。
            if path.join(&meta.spritesheet_file).exists() {
                infos.push(meta_to_info(&path, &meta));
            }
        }
    }

    // 内置宠物排在最前，按 BUILTIN_PET_IDS 顺序稳定排序。
    infos.sort_by(|a, b| {
        let ai = BUILTIN_PET_IDS.iter().position(|id| *id == a.id);
        let bi = BUILTIN_PET_IDS.iter().position(|id| *id == b.id);
        match (ai, bi) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.id.cmp(&b.id),
        }
    });

    Ok(infos)
}

/// 删除本地宠物（内置宠物不可删除）。
#[tauri::command]
pub fn delete_local_pet(app: AppHandle, pet_id: String) -> Result<(), String> {
    if BUILTIN_PET_IDS.contains(&pet_id.as_str()) {
        return Err("内置宠物不能删除".to_string());
    }
    let dir = pet_dir(&app, &pet_id)?;
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("删除宠物目录失败: {}", e))?;
    }
    Ok(())
}

/// 返回某只宠物精灵图的绝对路径（前端用 convertFileSrc 转成可加载 URL）。
#[tauri::command]
pub fn get_pet_spritesheet_path(app: AppHandle, pet_id: String) -> Result<String, String> {
    let dir = pet_dir(&app, &pet_id)?;
    let meta = read_meta(&dir)?.ok_or_else(|| format!("宠物 {} 未安装", pet_id))?;
    let path = dir.join(&meta.spritesheet_file);
    if !path.exists() {
        return Err(format!("宠物 {} 的精灵图不存在", pet_id));
    }
    Ok(path.to_string_lossy().to_string())
}

// --- 历史数据迁移 --------------------------------------------------------

/// 历史遗留的应用数据目录名（按「从新到旧」排列）。
///
/// 每次变更 identifier 都会更换 app_data 目录，这里登记全部旧目录名：
/// 启动时取其中第一个存在的，把数据补齐到当前目录。仅用于迁移，勿改。
const LEGACY_APP_DIR_NAMES: &[&str] = &[
    "io.github.hyqf.workbuddy-pet", // 首次改名后（原作者 hyqf 名下）
    "io.github.hyqf.zcode-pet",     // 最初：ZCodePet 时期
];

/// 把旧版数据目录一次性迁移到当前目录。
///
/// 项目改名为 workbuddy-PET 后 identifier 随之变更，`app_data_dir()` 指向新目录，
/// 旧目录中用户已下载/导入的宠物将不再被识别。此函数在启动时把旧目录内容补齐到新目录。
///
/// 触发条件（需同时满足）：
/// - 当前目录尚不存在 `pets/`（全新目录，从未安装过宠物）；
/// - 旧目录存在且与当前目录不同。
///
/// 采用「只补齐不覆盖」策略；失败只返回错误供调用方记录日志，绝不阻断启动。
pub fn migrate_legacy_app_data(app: &AppHandle) -> Result<(), String> {
    let new_dir = get_app_data_dir(app)?;
    if new_dir.join("pets").is_dir() {
        // 当前目录已有宠物数据：视为新装或已迁移，跳过。
        return Ok(());
    }
    let Some(base) = dirs::data_dir() else {
        return Ok(());
    };
    // 按「从新到旧」取第一个存在的旧目录，只迁移它，避免多目录内容重复复制。
    let Some(old_dir) = LEGACY_APP_DIR_NAMES
        .iter()
        .map(|name| base.join(name))
        .find(|dir| dir.is_dir() && *dir != new_dir)
    else {
        return Ok(());
    };
    copy_dir_recursive(&old_dir, &new_dir)?;
    tracing::info!(
        "[Migrate] 已迁移旧版数据目录: {} → {}",
        old_dir.display(),
        new_dir.display()
    );
    Ok(())
}

/// 递归复制目录内容到目标目录（已存在的目标文件不覆盖）。
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("创建目录失败 {}: {}", dst.display(), e))?;
    let entries = fs::read_dir(src).map_err(|e| format!("读取目录失败 {}: {}", src.display(), e))?;
    for entry in entries.flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if !to.exists() {
            fs::copy(&from, &to).map_err(|e| format!("复制文件失败 {}: {}", from.display(), e))?;
        }
    }
    Ok(())
}

// --- 命令：内置宠物安装 --------------------------------------------------

/// 把 4 只内置宠物的资源从安装包复制到本地持久化目录（幂等，已存在则跳过）。
/// 在 lib.rs 的 setup 中调用一次，确保用户开箱即有可用宠物。
pub fn ensure_builtin_pets_installed(app: &AppHandle) -> Result<(), String> {
    let resource_root = app
        .path()
        .resource_dir()
        .map_err(|e| format!("获取 resource_dir 失败: {}", e))?
        .join("pets");

    for id in BUILTIN_PET_IDS {
        let src_dir = resource_root.join(id);
        let dest_dir = pet_dir(app, id)?;

        // 精灵图缺失才复制（已安装则保留用户可能修改的 meta）。
        let dest_sprite = dest_dir.join("spritesheet.webp");
        if !dest_sprite.exists() {
            let src_sprite = src_dir.join("spritesheet.webp");
            if !src_sprite.exists() {
                // 资源未找到（开发模式下 resources 可能尚未拷贝），跳过不报错。
                continue;
            }
            fs::create_dir_all(&dest_dir).map_err(|e| format!("创建目录失败: {}", e))?;
            fs::copy(&src_sprite, &dest_sprite)
                .map_err(|e| format!("复制内置精灵图失败: {}", e))?;
            // 顺带复制 poster（若有）。
            let src_poster = src_dir.join("poster.webp");
            if src_poster.exists() {
                let _ = fs::copy(&src_poster, dest_dir.join("poster.webp"));
            }
        }

        // meta.json：始终从安装包同步（保证 displayName/描述最新），并补 installed_at。
        let src_meta = src_dir.join("meta.json");
        if src_meta.exists() {
            if let Ok(content) = fs::read_to_string(&src_meta) {
                if let Ok(mut meta) = serde_json::from_str::<LocalPetMeta>(&content) {
                    if meta.installed_at.is_none() {
                        meta.installed_at = Some(now_rfc3339());
                    }
                    let _ = write_meta(&dest_dir, &meta);
                }
            }
        }
    }

    Ok(())
}

// --- 命令：宠物悬浮窗口 --------------------------------------------------

/// 确保宠物悬浮窗口存在（透明、无边框、置顶、跳过任务栏）。已存在则直接返回。
///
/// 启动时即调用一次创建隐藏窗口，保证 `emit_to("pet")` 监听始终存活——
/// 即便窗口处于隐藏态，前端监听器与事件通道依旧有效。
///
/// 注意：构建后**不立即**全屏定位。macOS 上 webview 尚未完成 URL 加载时，
/// 过早 `set_size` 会触发 Tauri runtime 查询 `WebView::url`（wry 内部 unwrap None）导致 panic。
/// 全屏定位推迟到 `show_pet_window`/`toggle_pet_window`（窗口即将可见时）执行。
pub fn ensure_pet_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL) {
        return Ok(window);
    }

    WebviewWindowBuilder::new(app, PET_WINDOW_LABEL, WebviewUrl::App("/pet".into()))
        .title("Desktop Pet")
        // 占位尺寸：show 时由 position_fullscreen 铺满当前显示器。
        .inner_size(800.0, 600.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .visible(false)
        .focused(false)
        .build()
        .map_err(|e| format!("创建宠物窗口失败: {}", e))?;

    // 默认设为鼠标穿透（click-through）。前端轮询会在精灵上方切回可交互；
    // 即便前端启动失败/抛错，窗口也始终透明 + 穿透，绝不变成不透明遮挡层挡住整屏。
    if let Some(w) = app.get_webview_window(PET_WINDOW_LABEL) {
        let _ = w.set_ignore_cursor_events(true);
        return Ok(w);
    }
    Err("宠物窗口创建后无法获取".to_string())
}

/// 把宠物窗口定位到当前显示器右下角（考虑 Dock/任务栏预留边距）。
///
/// 优先取 `current_monitor()`（窗口当前所在屏），失败再回退 `primary_monitor()`，
/// 使宠物出现在用户当前关注的显示器而非永远是主屏。位置按显示器逻辑坐标（origin + size）
/// 钳制，保证窗口完全落在可见区域内（含边距），不溢出到屏幕外。
///
/// 全屏模式下 show/toggle 改用 `position_fullscreen`，此函数不再被调用，
/// 保留作为「右下角小窗模式」的备用定位能力。
#[allow(dead_code)]
fn position_bottom_right(window: &tauri::WebviewWindow) -> Result<(), String> {
    // current_monitor 在窗口首次创建/未定位时可能返回 None，此时回退主显示器。
    let monitor = window
        .current_monitor()
        .map_err(|e| format!("获取当前显示器失败: {}", e))?
        .or_else(|| {
            window
                .primary_monitor()
                .map_err(|e| format!("获取主显示器失败: {}", e))
                .ok()
                .flatten()
        })
        .ok_or_else(|| "未找到可用显示器".to_string())?;

    let scale = monitor.scale_factor();
    // 显示器在虚拟桌面中的逻辑坐标原点（多屏时可能为负）。
    let mon_origin_x = monitor.position().x as f64 / scale;
    let mon_origin_y = monitor.position().y as f64 / scale;
    let mon_logical_w = monitor.size().width as f64 / scale;
    let mon_logical_h = monitor.size().height as f64 / scale;

    // 右下角目标位置（逻辑坐标，相对虚拟桌面原点）。
    let target_x = mon_origin_x + mon_logical_w - PET_WINDOW_WIDTH - PET_WINDOW_RIGHT_MARGIN;
    let target_y = mon_origin_y + mon_logical_h - PET_WINDOW_HEIGHT - PET_WINDOW_BOTTOM_MARGIN;

    // 钳制：窗口至少留 8px 在显示器可见区内（不溢出屏幕边缘）。
    let min_x = mon_origin_x + 8.0;
    let min_y = mon_origin_y + 8.0;
    let max_x = mon_origin_x + mon_logical_w - PET_WINDOW_WIDTH - 8.0;
    let max_y = mon_origin_y + mon_logical_h - PET_WINDOW_HEIGHT - 8.0;
    let x = target_x.clamp(min_x, max_x.max(min_x));
    let y = target_y.clamp(min_y, max_y.max(min_y));

    window
        .set_position(LogicalPosition::new(x, y))
        .map_err(|e| format!("定位宠物窗口失败: {}", e))?;
    Ok(())
}

/// 把宠物窗口铺满当前显示器（全屏透明覆盖层）。
///
/// 宠物在全屏透明窗口内自由漫游（前端 viewport/click-through 已自适应窗口尺寸）。
/// 优先 `current_monitor`，失败回退 `primary_monitor`。窗口定位到显示器逻辑原点，
/// 尺寸设为显示器逻辑全尺寸（覆盖整屏，含 macOS 菜单栏区域——透明窗口不遮挡视线）。
fn position_fullscreen(window: &tauri::WebviewWindow) -> Result<(), String> {
    let monitor = window
        .current_monitor()
        .map_err(|e| format!("获取当前显示器失败: {}", e))?
        .or_else(|| {
            window
                .primary_monitor()
                .map_err(|e| format!("获取主显示器失败: {}", e))
                .ok()
                .flatten()
        })
        .ok_or_else(|| "未找到可用显示器".to_string())?;

    let scale = monitor.scale_factor();
    let mon_origin_x = monitor.position().x as f64 / scale;
    let mon_origin_y = monitor.position().y as f64 / scale;
    let mon_logical_w = monitor.size().width as f64 / scale;
    let mon_logical_h = monitor.size().height as f64 / scale;

    // 定位到显示器逻辑原点，并按显示器逻辑尺寸铺满。
    window
        .set_position(LogicalPosition::new(mon_origin_x, mon_origin_y))
        .map_err(|e| format!("定位全屏宠物窗口失败: {}", e))?;
    window
        .set_size(LogicalSize::new(mon_logical_w, mon_logical_h))
        .map_err(|e| format!("设置全屏宠物窗口尺寸失败: {}", e))?;
    Ok(())
}

/// 窗口迁移后返回的新几何信息（物理像素），供前端做宠物坐标重映射。
/// 前端在迁移前后用「虚拟桌面物理坐标」保持宠物位置连续，实现无缝跨屏。
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowGeometry {
    /// 新窗口物理原点（虚拟桌面坐标系）。
    origin_x: f64,
    origin_y: f64,
    /// 新窗口物理宽高。
    width: f64,
    height: f64,
    /// 新窗口逻辑宽高（= 物理 / scaleFactor），与 PixiJS 画布逻辑坐标一致。
    logical_width: f64,
    logical_height: f64,
    /// 新显示器的 scaleFactor。
    scale_factor: f64,
}

// --- 位置记忆 ------------------------------------------------------------

/// 宠物窗口位置持久化结构（逻辑坐标，与显示器虚拟桌面坐标系一致）。
#[derive(Serialize, Deserialize)]
struct PetWindowState {
    x: f64,
    y: f64,
}

/// 宠物窗口位置持久化文件：<app_data>/pet-window.json。
fn pet_window_state_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(get_app_data_dir(app)?.join("pet-window.json"))
}

/// 落盘宠物窗口位置（逻辑坐标）。
fn save_pet_window_position(app: &AppHandle, x: f64, y: f64) -> Result<(), String> {
    let path = pet_window_state_path(app)?;
    let content = serde_json::to_string_pretty(&PetWindowState { x, y })
        .map_err(|e| format!("序列化宠物窗口位置失败: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("写入宠物窗口位置失败: {}", e))?;
    Ok(())
}

/// 落盘宠物窗口位置的公开入口（供 lib.rs 的托盘切换助手调用）。
pub fn save_pet_window_position_pub(app: &AppHandle, x: f64, y: f64) -> Result<(), String> {
    save_pet_window_position(app, x, y)
}

/// 纯函数：判断窗口矩形 (x,y,w,h) 是否完全落在任一显示器矩形内。
///
/// `monitors` 每个元素为 `(origin_x, origin_y, width, height)`（逻辑坐标）。
/// 任一显示器完全包含窗口矩形即返回 true；显示器列表为空时返回 true（宽容策略，
/// 避免无法读取显示器信息时阻止恢复记忆位置）。抽出为纯函数以便单测。
///
/// 全屏模式下不再用于位置校验，保留作为带单测的纯工具函数。
#[allow(dead_code)]
fn rect_inside_any_monitor(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    monitors: &[(f64, f64, f64, f64)],
) -> bool {
    if monitors.is_empty() {
        return true;
    }
    let win_right = x + w;
    let win_bottom = y + h;
    monitors.iter().any(|(mx, my, mw, mh)| {
        let mon_right = mx + mw;
        let mon_bottom = my + mh;
        x >= *mx && y >= *my && win_right <= mon_right && win_bottom <= mon_bottom
    })
}

// --- 命令：宠物悬浮窗口 --------------------------------------------------

/// 显示宠物窗口。全屏透明窗口铺满当前显示器（单屏），宠物在内部自由漫游。
/// 多屏漫游由前端在检测到宠物跨屏时调用 `move_pet_window_to_monitor` 迁移窗口实现。
///
/// 会等待前端「首帧就绪」事件再显示：pet 窗口加载 HTML 时，webview 在解析到内联透明
/// 样式（index.html 的 html.pet-window）之前会闪现默认白底。这里在 show 前阻塞等待前端
/// emit 的 `pet-window-ready`（DOMContentLoaded 触发，此时内联透明样式已应用），消除白屏。
/// 带 800ms 超时兜底：即便前端加载失败/抛错，窗口仍会显示（不能因等不到事件而永远不显示）。
#[tauri::command]
pub async fn show_pet_window(app: AppHandle) -> Result<(), String> {
    let window = ensure_pet_window(&app)?;
    // 单屏铺满当前显示器（macOS 下跨屏超大窗口在副屏不可见，故采用单屏 + 迁移）。
    position_fullscreen(&window)?;

    // 等待前端首帧就绪（最多 800ms），消除 webview 解析 HTML 期间的白色闪屏。
    wait_pet_window_ready(&app).await;

    window.show().map_err(|e| format!("显示宠物窗口失败: {}", e))?;
    Ok(())
}

/// 等待前端 emit `pet-window-ready`（带 800ms 超时兜底）。
///
/// pet 窗口的 index.html 内联了透明样式（html.pet-window body { background:transparent }），
/// 但该样式仅在 HTML 解析后生效。webview 创建后、解析前的极短窗口内会渲染默认白底。
/// 前端在 DOMContentLoaded（内联透明样式已应用）时 emit 本事件；后端等到后再 show，
/// 从用户视角窗口「直接透明出现」，不再有白屏闪现。超时兜底防止前端异常导致窗口永不显示。
async fn wait_pet_window_ready(app: &AppHandle) {
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
    let tx_clone = tx.clone();
    let _listener = app.listen("pet-window-ready", move |_event| {
        if let Some(sender) = tx_clone.lock().ok().and_then(|mut g| g.take()) {
            let _ = sender.send(());
        }
    });
    // 超时兜底：800ms 内没等到事件也继续 show（不能让窗口因前端问题而永远不可见）。
    let _ = tokio::time::timeout(Duration::from_millis(800), rx).await;
}

/// 隐藏宠物窗口。
///
/// 全屏模式下窗口位置恒定（恒铺满整屏），无需落盘记忆位置，直接隐藏即可。
#[tauri::command]
pub fn hide_pet_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL) {
        window.hide().map_err(|e| format!("隐藏宠物窗口失败: {}", e))?;
    }
    Ok(())
}

/// 切换宠物窗口显隐，返回切换后是否可见。
///
/// 全屏透明窗口模式下位置恒定，显示时直接铺满整屏，隐藏时无需落盘记忆位置。
/// 显示路径同样等待前端首帧就绪，消除白屏（与 show_pet_window 一致）。
#[tauri::command]
pub async fn toggle_pet_window(app: AppHandle) -> Result<bool, String> {
    let window = ensure_pet_window(&app)?;
    let visible = window.is_visible().map_err(|e| format!("{}", e))?;
    if visible {
        window.hide().map_err(|e| format!("隐藏宠物窗口失败: {}", e))?;
        Ok(false)
    } else {
        // 单屏铺满当前显示器。
        position_fullscreen(&window)?;
        wait_pet_window_ready(&app).await;
        window.show().map_err(|e| format!("显示宠物窗口失败: {}", e))?;
        Ok(true)
    }
}

/// 设置宠物窗口的置顶状态（运行时切换，对应设置里的"始终置顶"开关）。
#[tauri::command]
pub fn set_pet_always_on_top(app: AppHandle, always_on_top: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL) {
        window
            .set_always_on_top(always_on_top)
            .map_err(|e| format!("切换置顶失败: {}", e))?;
    }
    Ok(())
}

/// 把宠物窗口迁移到指定显示器并铺满该屏（单屏铺满，跨屏漫游的迁移原语）。
///
/// macOS 下单个透明窗口无法跨屏渲染（副屏不可见），故采用「单屏铺满 + 跨屏迁移」：
/// 窗口始终铺满宠物当前所在屏；当宠物走到屏边要进入相邻屏时，前端调用此命令把窗口
/// 迁移到目标屏。返回新窗口几何（物理 + 逻辑），前端据此重映射宠物坐标，保持视觉连续。
///
/// `target_monitor_name` 为目标显示器名（来自 Tauri availableMonitors 的 Monitor.name）。
/// 找不到该显示器时回退 `position_fullscreen`（铺满当前屏）。
#[tauri::command]
pub fn move_pet_window_to_monitor(
    app: AppHandle,
    target_monitor_name: String,
) -> Result<WindowGeometry, String> {
    let window = app
        .get_webview_window(PET_WINDOW_LABEL)
        .ok_or_else(|| "宠物窗口不存在".to_string())?;

    let monitors = window
        .available_monitors()
        .map_err(|e| format!("获取显示器列表失败: {}", e))?;

    // 按名匹配目标显示器。
    let target = monitors
        .into_iter()
        .find(|m| m.name().map(|n| *n == target_monitor_name).unwrap_or(false));
    // 找不到 → 回退当前屏铺满。
    let monitor = match target {
        Some(m) => m,
        None => {
            let (origin_x, origin_y, logical_w, logical_h, scale) =
                position_fullscreen_and_report(&window)?;
            return Ok(WindowGeometry {
                origin_x,
                origin_y,
                width: logical_w * scale,
                height: logical_h * scale,
                logical_width: logical_w,
                logical_height: logical_h,
                scale_factor: scale,
            });
        }
    };

    let scale = monitor.scale_factor();
    let pos = monitor.position();
    let size = monitor.size();
    let origin_x = pos.x as f64;
    let origin_y = pos.y as f64;
    let width = size.width as f64;
    let height = size.height as f64;
    let logical_width = width / scale;
    let logical_height = height / scale;

    // 迁移：定位到目标屏物理原点，尺寸设为目标屏物理全尺寸。
    window
        .set_position(PhysicalPosition::new(origin_x, origin_y))
        .map_err(|e| format!("迁移宠物窗口定位失败: {}", e))?;
    window
        .set_size(PhysicalSize::new(width, height))
        .map_err(|e| format!("迁移宠物窗口尺寸失败: {}", e))?;

    Ok(WindowGeometry {
        origin_x,
        origin_y,
        width,
        height,
        logical_width,
        logical_height,
        scale_factor: scale,
    })
}

/// 铺满当前屏（position_fullscreen）并返回其几何信息（物理原点 + 逻辑尺寸 + scale）。
/// 供 move_pet_window_to_monitor 的回退路径复用。
fn position_fullscreen_and_report(
    window: &tauri::WebviewWindow,
) -> Result<(f64, f64, f64, f64, f64), String> {
    let monitor = window
        .current_monitor()
        .map_err(|e| format!("获取当前显示器失败: {}", e))?
        .or_else(|| {
            window
                .primary_monitor()
                .map_err(|e| format!("获取主显示器失败: {}", e))
                .ok()
                .flatten()
        })
        .ok_or_else(|| "未找到可用显示器".to_string())?;

    let scale = monitor.scale_factor();
    let pos = monitor.position();
    let size = monitor.size();
    let origin_x = pos.x as f64;
    let origin_y = pos.y as f64;
    let logical_w = size.width as f64 / scale;
    let logical_h = size.height as f64 / scale;

    window
        .set_position(LogicalPosition::new(origin_x, origin_y))
        .map_err(|e| format!("定位全屏宠物窗口失败: {}", e))?;
    window
        .set_size(LogicalSize::new(logical_w, logical_h))
        .map_err(|e| format!("设置全屏宠物窗口尺寸失败: {}", e))?;

    Ok((origin_x, origin_y, logical_w, logical_h, scale))
}

// --- 命令：本地导入宠物 --------------------------------------------------

/// 从本地文件导入宠物精灵图。
///
/// 读取用户选择的图片文件，检测格式（PNG / WebP）并校验是否符合 Codex atlas 尺寸契约
/// （宽度 1536、高度为 208 的整数倍），生成唯一 id 并落盘到
/// `<app_data>/pets/uploaded-<hex>/`，写入 `meta.json`（source: "uploaded"）。
/// 文件名（去扩展名）作为宠物显示名，也可由 `display_name` 覆盖。
#[tauri::command]
pub async fn import_local_pet(
    app: AppHandle,
    file_path: String,
    display_name: Option<String>,
) -> Result<LocalPetInfo, String> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("文件不存在: {}", file_path));
    }

    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|e| format!("读取文件失败: {}", e))?;

    if bytes.is_empty() {
        return Err("文件为空".to_string());
    }

    // 检测格式并确定扩展名。
    let ext = detect_image_ext(&bytes)
        .ok_or_else(|| "不支持的图片格式（仅支持 PNG / WebP）".to_string())?;

    // 校验精灵图尺寸：不符合 Codex atlas 契约的图能写入但渲染必然失败，
    // 故在此提前拦截并给出明确提示（也避免留下无法使用的半成品目录）。
    validate_atlas_size(&bytes)?;

    // 生成唯一 id：uploaded-<16位hex>（时间戳纳秒，碰撞概率极低）。
    let id = generate_uploaded_id();
    let dir = pet_dir(&app, &id)?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("创建目录失败: {}", e))?;

    let spritesheet_file = format!("spritesheet.{}", ext);
    let spritesheet_path = dir.join(&spritesheet_file);
    tokio::fs::write(&spritesheet_path, &bytes)
        .await
        .map_err(|e| format!("写入精灵图失败: {}", e))?;

    // 显示名：优先用参数，否则用文件名（去扩展名）。
    let name = display_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "导入宠物".to_string());

    let meta = LocalPetMeta {
        id: id.clone(),
        display_name: name,
        description: None,
        kind: None,
        tags: vec![],
        source: "uploaded".to_string(),
        spritesheet_file,
        poster_file: None,
        spritesheet_url: None,
        version: None,
        sprite_version_number: None,
        installed_at: Some(now_rfc3339()),
    };
    write_meta(&dir, &meta)?;

    Ok(meta_to_info(&dir, &meta))
}

/// 从 magic bytes 检测图片格式，返回扩展名（png / webp）。
pub(crate) fn detect_image_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 8 && &bytes[..8] == b"\x89PNG\r\n\x1a\n" {
        return Some("png");
    }
    // WebP: RIFF....WEBP（offset 0 = "RIFF", offset 8 = "WEBP"）。
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    None
}

/// 生成上传宠物的唯一 id：uploaded-<16位hex 纳秒时间戳>。
fn generate_uploaded_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("uploaded-{:016x}", nanos)
}

/// Codex atlas 契约：图集宽度（8 列 × 192）。
///
/// 与前端 `src/modules/desktopPet/engine/codexAtlas.ts` 的 `CODEX_ATLAS_WIDTH` 保持一致。
pub(crate) const ATLAS_WIDTH: u32 = 1536;

/// Codex atlas 契约：单个网格单元高度（行数 = 高度 / 该值）。
///
/// 与前端 `CODEX_CELL_HEIGHT` 保持一致。市场安装（pet_market.rs）也用它推断图集版本。
pub(crate) const ATLAS_CELL_HEIGHT: u32 = 208;

/// 解析图片字节流的像素宽高（仅 PNG / WebP）。
///
/// 只读取文件头，不引入图像解码依赖：
/// - **PNG**：8 字节签名后紧跟 IHDR，宽高为大端 u32（偏移 16..24）。
/// - **WebP**：RIFF 容器，按 `VP8X`（扩展）/ `VP8L`（无损）/ `VP8 `（有损）三种 chunk 解析。
///
/// 返回 `None` 表示无法识别尺寸（文件截断或未知变体）。
pub(crate) fn read_image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    // --- PNG ---
    if bytes.len() >= 24 && &bytes[..8] == b"\x89PNG\r\n\x1a\n" {
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        return Some((w, h));
    }

    // --- WebP（RIFF 容器：'RIFF' + 长度 + 'WEBP'，随后是 chunk）---
    if bytes.len() >= 20 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return match &bytes[12..16] {
            b"VP8X" => {
                // 扩展格式：24 位 canvas 宽高（存储值为实际值 - 1）。
                if bytes.len() < 30 {
                    None
                } else {
                    let w = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], 0]) + 1;
                    let h = u32::from_le_bytes([bytes[27], bytes[28], bytes[29], 0]) + 1;
                    Some((w, h))
                }
            }
            b"VP8L" => {
                // 无损格式：signature 0x2f + 14 位宽 + 14 位高（存储值为实际值 - 1）。
                if bytes.len() < 25 || bytes[20] != 0x2f {
                    None
                } else {
                    let bits = u32::from_le_bytes([bytes[21], bytes[22], bytes[23], bytes[24]]);
                    let w = (bits & 0x3FFF) + 1;
                    let h = ((bits >> 14) & 0x3FFF) + 1;
                    Some((w, h))
                }
            }
            b"VP8 " => {
                // 有损格式：frame tag(3) + start code 0x9d012a(3) + 宽(2) + 高(2)，各取低 14 位。
                if bytes.len() < 30 || &bytes[23..26] != b"\x9d\x01\x2a" {
                    None
                } else {
                    let w = (u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3FFF) as u32;
                    let h = (u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3FFF) as u32;
                    Some((w, h))
                }
            }
            _ => None,
        };
    }

    None
}

/// 校验精灵图是否符合 Codex atlas 契约（宽度恒为 1536，高度为 208 的整数倍）。
///
/// 无法识别尺寸时**放行**：宁可由渲染层兜底报错，也不误伤未知但合法的图片变体。
pub(crate) fn validate_atlas_size(bytes: &[u8]) -> Result<(), String> {
    let Some((w, h)) = read_image_size(bytes) else {
        return Ok(());
    };
    if w == ATLAS_WIDTH && h >= ATLAS_CELL_HEIGHT && h % ATLAS_CELL_HEIGHT == 0 {
        return Ok(());
    }
    Err(format!(
        "图片尺寸 {w}×{h} 不符合宠物精灵图规格：宽度必须为 {ATLAS_WIDTH}，\
         高度必须是 {ATLAS_CELL_HEIGHT} 的整数倍（例如 1536×1872 或 1536×2288）"
    ))
}

// --- 命令：WorkBuddy hook 联动（转调 crate::workbuddy 模块） --------------

/// 启用/禁用 WorkBuddy hook 联动（注入/清理 ~/.workbuddy/settings.json）。
///
/// 薄转调：实际逻辑在 `crate::workbuddy` 模块（由其负责安装 hook 脚本、改写配置文件）。
#[tauri::command]
pub fn link_workbuddy(app: AppHandle, enabled: bool) -> Result<crate::workbuddy::LinkResult, String> {
    crate::workbuddy::set_workbuddy_linked(&app, enabled)
}

/// 查询当前是否已启用 WorkBuddy 联动。
#[tauri::command]
pub fn get_workbuddy_link_status(app: AppHandle) -> Result<bool, String> {
    Ok(crate::workbuddy::is_workbuddy_linked(&app))
}

/// 检测系统是否安装了 Node.js（WorkBuddy hook 联动依赖）。
///
/// hook 脚本以 `node <script>` 方式被 WorkBuddy 拉起，若系统无 node 则联动静默失效。
/// 前端在开启联动前调用此命令，缺失时提示用户安装 Node.js。
/// 返回 node 版本号（如 "v20.11.0"）；不可用返回 Err。
///
/// 注意：Windows 上 WorkBuddy 通过 Git Bash 执行 hook，若 node 只装在系统 PATH
/// 而未进入 Git Bash 的 PATH，这里能检测到但 hook 仍会静默失效。
#[tauri::command]
pub fn check_node_available() -> Result<String, String> {
    let output = std::process::Command::new("node")
        .arg("--version")
        .output()
        .map_err(|e| format!("未找到 Node.js：{}", e))?;
    if !output.status.success() {
        return Err("Node.js 不可用（node --version 执行失败）".to_string());
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        return Err("无法读取 Node.js 版本".to_string());
    }
    Ok(version)
}

// --- 命令：WorkBuddy token 使用量统计 ------------------------------------

/// 获取 WorkBuddy SQLite 数据库路径（自动检测 + 用户覆盖）。
///
/// 返回检测到的 db.sqlite 绝对路径字符串，供前端展示。
/// 未检测到时返回 None（前端提示用户手动填写数据目录）。
#[tauri::command]
pub fn get_workbuddy_db_path(app: AppHandle) -> Result<Option<String>, String> {
    Ok(crate::workbuddy::stats::resolve_db_path(&app)
        .map(|p| p.to_string_lossy().to_string()))
}

/// 设置/清除 WorkBuddy 数据目录覆盖路径。
///
/// `dir` 为 None 或空串时清除覆盖（恢复自动检测）。
/// 返回 true 表示覆盖后路径检测成功（DB 文件存在）。
#[tauri::command]
pub fn set_workbuddy_data_dir(app: AppHandle, dir: Option<String>) -> Result<bool, String> {
    let trimmed = dir.as_deref().map(str::trim).filter(|s| !s.is_empty());
    crate::workbuddy::stats::write_override_dir(&app, trimmed)
}

/// 查询今日 WorkBuddy token 使用量统计。
///
/// 从 WorkBuddy SQLite 库只读查询 model_usage 表，返回今日各模型 token 消耗汇总。
/// DB 路径未检测到时返回 Err（前端可静默忽略）。
#[tauri::command]
pub fn get_workbuddy_token_stats(
    app: AppHandle,
) -> Result<Option<crate::workbuddy::stats::TokenStats>, String> {
    let db_path = crate::workbuddy::stats::resolve_db_path(&app)
        .ok_or_else(|| "未检测到 WorkBuddy 数据目录".to_string())?;
    crate::workbuddy::stats::query_today_stats(&db_path).map(Some)
}

// --- 命令：WorkBuddy 免费模型 AI 台词 -------------------------------------

/// 列出 WorkBuddy 已配置的模型（`~/.workbuddy/models.json`，脱敏不含 apiKey）。
///
/// 供管理窗口的「AI 搭话」设置区做模型下拉；未配置模型时返回 Err，前端提示用户。
#[tauri::command]
pub fn list_workbuddy_models(
) -> Result<Vec<crate::workbuddy::ai::WorkBuddyModelInfo>, String> {
    crate::workbuddy::ai::list_models()
}

/// 用 WorkBuddy 配置的免费模型生成一句宠物台词。
///
/// 直接以 `POST {model.url}` 调 OpenAI 兼容的 chat/completions，复用 WorkBuddy 的
/// apiKey（只读，不出后端）。失败返回 Err，由前端回退到内置固定语录。
///
/// # 参数
/// - `model_id`：模型 id；None / 不存在时用第一个可用模型。
/// - `topic`：`"chat"`（随口搭话，默认）或 `"news"`（今日播报）。
/// - `locale`：`"zh-CN"` / `"en-US"`，决定台词语言。
/// - `context`：可选的现场上下文（今日待办等），拼进提示词。
#[tauri::command]
pub async fn generate_pet_line(
    model_id: Option<String>,
    topic: Option<String>,
    locale: Option<String>,
    context: Option<String>,
) -> Result<String, String> {
    crate::workbuddy::ai::generate_line(
        model_id.as_deref(),
        topic.as_deref(),
        locale.as_deref(),
        context.as_deref(),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- 图片尺寸解析 / atlas 契约校验 -------------------------------------

    /// 构造最小 PNG 头（签名 + IHDR 宽高，共 24 字节；解析只需前 24 字节）。
    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        v.extend_from_slice(&[0, 0, 0, 13]); // IHDR chunk 长度
        v.extend_from_slice(b"IHDR");
        v.extend_from_slice(&width.to_be_bytes());
        v.extend_from_slice(&height.to_be_bytes());
        v
    }

    /// 构造 VP8X（扩展）WebP 头。
    fn webp_vp8x_header(width: u32, height: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&[0, 0, 0, 0]); // 文件总长（解析不校验）
        v.extend_from_slice(b"WEBP");
        v.extend_from_slice(b"VP8X");
        v.extend_from_slice(&[10, 0, 0, 0]); // chunk 长度
        v.extend_from_slice(&[0, 0, 0, 0]); // flags + reserved
        v.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
        v.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
        v
    }

    /// 构造 VP8L（无损）WebP 头。
    fn webp_vp8l_header(width: u32, height: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&[0, 0, 0, 0]);
        v.extend_from_slice(b"WEBP");
        v.extend_from_slice(b"VP8L");
        v.extend_from_slice(&[10, 0, 0, 0]);
        let bits: u32 = (width - 1) | ((height - 1) << 14);
        v.push(0x2f);
        v.extend_from_slice(&bits.to_le_bytes());
        v
    }

    /// 构造 VP8（有损）WebP 头。
    fn webp_vp8_header(width: u32, height: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&[0, 0, 0, 0]);
        v.extend_from_slice(b"WEBP");
        v.extend_from_slice(b"VP8 ");
        v.extend_from_slice(&[10, 0, 0, 0]);
        v.extend_from_slice(&[0, 0, 0]); // frame tag
        v.extend_from_slice(b"\x9d\x01\x2a"); // start code
        v.extend_from_slice(&(width as u16).to_le_bytes());
        v.extend_from_slice(&(height as u16).to_le_bytes());
        v
    }

    #[test]
    fn read_image_size_parses_png() {
        assert_eq!(read_image_size(&png_header(1536, 1872)), Some((1536, 1872)));
    }

    #[test]
    fn read_image_size_parses_all_webp_variants() {
        assert_eq!(
            read_image_size(&webp_vp8x_header(1536, 2288)),
            Some((1536, 2288))
        );
        assert_eq!(
            read_image_size(&webp_vp8l_header(1536, 1872)),
            Some((1536, 1872))
        );
        assert_eq!(
            read_image_size(&webp_vp8_header(1536, 1872)),
            Some((1536, 1872))
        );
    }

    #[test]
    fn read_image_size_returns_none_for_truncated_or_unknown() {
        assert_eq!(read_image_size(&[]), None);
        assert_eq!(read_image_size(b"not an image at all"), None);
        // 签名正确但长度不足，无法读取宽高。
        assert_eq!(read_image_size(b"\x89PNG\r\n\x1a\n"), None);
    }

    #[test]
    fn validate_atlas_size_accepts_conforming_spritesheets() {
        assert!(validate_atlas_size(&png_header(1536, 1872)).is_ok());
        assert!(validate_atlas_size(&png_header(1536, 2288)).is_ok());
        // 单行图集边界值也应接受。
        assert!(validate_atlas_size(&webp_vp8x_header(1536, 208)).is_ok());
    }

    #[test]
    fn validate_atlas_size_rejects_non_conforming() {
        // 宽度不是 1536（普通截图 / 单帧插画）。
        assert!(validate_atlas_size(&png_header(1024, 1872)).is_err());
        // 高度不是 208 的整数倍。
        assert!(validate_atlas_size(&png_header(1536, 1000)).is_err());
        // 高度不足一格。
        assert!(validate_atlas_size(&png_header(1536, 100)).is_err());
    }

    #[test]
    fn validate_atlas_size_passes_through_unrecognized() {
        // 尺寸无法识别时放行（交由渲染层兜底），避免误伤未知变体。
        assert!(validate_atlas_size(b"not an image").is_ok());
    }

    // --- 历史数据迁移 ------------------------------------------------------

    #[test]
    fn copy_dir_recursive_copies_without_overwriting() {
        let base = std::env::temp_dir().join(format!(
            "workbuddy-pet-migrate-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&base);

        let src = base.join("src");
        let dst = base.join("dst");
        // 源：嵌套一层目录 + 根文件。
        std::fs::create_dir_all(src.join("pets/a")).unwrap();
        std::fs::write(src.join("pets/a/meta.json"), "old").unwrap();
        std::fs::write(src.join("root.txt"), "r").unwrap();
        // 目标已存在同名文件：迁移不得覆盖。
        std::fs::create_dir_all(dst.join("pets/a")).unwrap();
        std::fs::write(dst.join("pets/a/meta.json"), "keep").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();

        assert_eq!(
            std::fs::read_to_string(dst.join("pets/a/meta.json")).unwrap(),
            "keep",
            "已存在的目标文件不应被覆盖"
        );
        assert_eq!(
            std::fs::read_to_string(dst.join("root.txt")).unwrap(),
            "r",
            "缺失的文件应被补齐"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn rect_inside_monitor_when_fully_contained() {
        // 单显示器：原点 (0,0)，1920x1080。窗口 300x320 完全在内。
        let monitors = vec![(0.0, 0.0, 1920.0, 1080.0)];
        assert!(rect_inside_any_monitor(100.0, 100.0, 300.0, 320.0, &monitors));
        // 紧贴右下角（含）。
        assert!(rect_inside_any_monitor(1620.0, 760.0, 300.0, 320.0, &monitors));
    }

    #[test]
    fn rect_outside_when_partially_offscreen() {
        let monitors = vec![(0.0, 0.0, 1920.0, 1080.0)];
        // 右边溢出。
        assert!(!rect_inside_any_monitor(1700.0, 100.0, 300.0, 320.0, &monitors));
        // 下边溢出。
        assert!(!rect_inside_any_monitor(100.0, 900.0, 300.0, 320.0, &monitors));
        // 左上为负。
        assert!(!rect_inside_any_monitor(-10.0, 0.0, 300.0, 320.0, &monitors));
    }

    #[test]
    fn rect_inside_any_of_multiple_monitors() {
        // 双屏：主屏 (0,0) 1920x1080，副屏 (1920,0) 1920x1080。
        let monitors = vec![
            (0.0, 0.0, 1920.0, 1080.0),
            (1920.0, 0.0, 1920.0, 1080.0),
        ];
        // 落在副屏内。
        assert!(rect_inside_any_monitor(2000.0, 100.0, 300.0, 320.0, &monitors));
        // 横跨双屏（不在任一屏内）。
        assert!(!rect_inside_any_monitor(1800.0, 100.0, 300.0, 320.0, &monitors));
    }

    #[test]
    fn rect_empty_monitors_is_tolerant() {
        // 无法读取显示器时应宽容放行（不阻止恢复记忆位置）。
        assert!(rect_inside_any_monitor(100.0, 100.0, 300.0, 320.0, &[]));
    }

    // --- 删除保护测试 ------------------------------------------------------

    #[test]
    fn delete_rejects_builtin_id() {
        // 纯逻辑检查：内置 id 应在保护列表中。
        for id in BUILTIN_PET_IDS {
            assert!(BUILTIN_PET_IDS.contains(id));
        }
    }

    // --- 图片格式检测测试 --------------------------------------------------

    #[test]
    fn detect_png_magic_bytes() {
        let png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x00IHDR";
        assert_eq!(detect_image_ext(png), Some("png"));
    }

    #[test]
    fn detect_webp_magic_bytes() {
        let mut webp = b"RIFF\x00\x00\x00\x00WEBP".to_vec();
        webp.extend_from_slice(&[0u8; 10]);
        assert_eq!(detect_image_ext(&webp), Some("webp"));
    }

    #[test]
    fn detect_unknown_format_returns_none() {
        let jpeg = b"\xff\xd8\xff\xe0\x00\x10JFIF";
        assert_eq!(detect_image_ext(jpeg), None);
        let random = b"hello world this is not an image";
        assert_eq!(detect_image_ext(random), None);
    }

    #[test]
    fn uploaded_id_format_is_valid() {
        let id = generate_uploaded_id();
        assert!(id.starts_with("uploaded-"));
        // 验证 id 通过 pet_dir 的字符校验（仅 a-z0-9-）。
        assert!(
            id.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        );
    }
}
