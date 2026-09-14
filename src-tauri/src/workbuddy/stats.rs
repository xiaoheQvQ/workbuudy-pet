//! WorkBuddy 使用统计：从本地 SQLite 库读取 token 使用量。
//!
//! 数据源：WorkBuddy 桌面版（底层 CodeBuddy Code 引擎）本地的 SQLite 库。
//! 引擎的存储目录布局随版本演进，因此这里采用「候选根目录 + 多候选相对路径 + 受限深度兜底搜索」
//! 的探测策略，并保留用户手动覆盖入口。只读打开（`SQLITE_OPEN_READ_ONLY`），
//! 避免与 WorkBuddy 主进程产生锁冲突。
//!
//! 路径决策链：
//! ```text
//! 0. 用户覆盖（<app_data>/workbuddy-data-dir.json）→ 最高优先级
//! 1. WORKBUDDY_STORAGE_DIR / WORKBUDDY_HOME 环境变量
//! 2. CODEBUDDY_STORAGE_DIR / CODEBUDDY_HOME 环境变量（同源引擎回退）
//! 3. ~/.workbuddy、~/.codebuddy、~/.codebuddy-beta（默认候选根目录）
//! ```
//!
//! 注意：统计查询仍假设引擎使用 `model_usage` 表（与 Claude Code / CodeBuddy 系一致）。
//! 若目标版本表结构不同，查询会返回错误并被前端静默忽略（统计是非关键增强功能）。

use std::path::{Path, PathBuf};

use chrono::{Datelike, TimeZone};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use tauri::{AppHandle, Manager};

/// 用户覆盖的数据目录路径存储文件名（位于 app_data_dir 下）。
const DATA_DIR_OVERRIDE_FILE: &str = "workbuddy-data-dir.json";

/// 引擎存储库相对候选根目录的常见位置（按优先级）。
const DB_REL_CANDIDATES: &[&str] = &[
    "cli/db/db.sqlite",
    "db/db.sqlite",
    "db.sqlite",
    "storage/db.sqlite",
    "cli/storage/db.sqlite",
];

/// 兜底搜索的最大目录深度（相对候选根目录）。
const DB_SEARCH_MAX_DEPTH: usize = 3;

/// 环境变量候选（WorkBuddy 优先，CodeBuddy 同源引擎回退）。
const ENV_DIR_VARS: &[&str] = &[
    "WORKBUDDY_STORAGE_DIR",
    "WORKBUDDY_HOME",
    "CODEBUDDY_STORAGE_DIR",
    "CODEBUDDY_HOME",
];

/// home 下的候选根目录（按优先级）。
const HOME_ROOT_CANDIDATES: &[&str] = &[".workbuddy", ".codebuddy", ".codebuddy-beta"];

// ---------------------------------------------------------------------------
// DTO（serde camelCase，与前端 TypeScript 类型对齐）
// ---------------------------------------------------------------------------

/// 单个模型的 token 使用统计行。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTokenRow {
    /// 模型标识（如 "glm-5.2"）。
    pub model_id: String,
    /// 今日 API 调用次数。
    pub calls: u64,
    /// 今日该模型消耗的总 token 数。
    pub total_tokens: u64,
}

/// 今日 WorkBuddy token 使用量汇总。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenStats {
    /// 实际查询的数据库路径（供前端展示）。
    pub db_path: String,
    /// 今日输入 token 总量。
    pub today_input_tokens: u64,
    /// 今日输出 token 总量。
    pub today_output_tokens: u64,
    /// 今日计算总 token（input + output + reasoning + cache 等）。
    pub today_total_tokens: u64,
    /// 今日 API 调用总次数。
    pub today_calls: u64,
    /// 今日各模型 token 明细（按消耗降序）。
    pub active_models: Vec<ModelTokenRow>,
}

// ---------------------------------------------------------------------------
// 用户覆盖文件管理
// ---------------------------------------------------------------------------

/// 获取覆盖文件完整路径。
fn override_file_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join(DATA_DIR_OVERRIDE_FILE))
}

/// 读取用户覆盖的数据目录路径（非空字符串才视为有效覆盖）。
fn read_override_dir(app: &AppHandle) -> Option<String> {
    let path = override_file_path(app)?;
    let content = std::fs::read_to_string(&path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    v.get("dir")?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// 写入用户覆盖路径。`dir` 为 None 或空串时清除覆盖（恢复自动检测）。
///
/// 返回 `true` 表示写入后路径检测成功（DB 文件存在）。
pub fn write_override_dir(app: &AppHandle, dir: Option<&str>) -> Result<bool, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let file = data_dir.join(DATA_DIR_OVERRIDE_FILE);
    let v = serde_json::json!({ "dir": dir.unwrap_or("") });
    let json_str = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    std::fs::write(&file, json_str).map_err(|e| e.to_string())?;
    // 写入后立即验证路径是否可解析。
    Ok(resolve_db_path(app).is_some())
}

// ---------------------------------------------------------------------------
// 探测链
// ---------------------------------------------------------------------------

/// 在给定目录下探测 SQLite 库文件。
///
/// 先按 [`DB_REL_CANDIDATES`] 试固定相对路径，未命中则在受限深度内兜底搜索
/// `db.sqlite` / `*.sqlite`。
fn find_db_in(root: &Path) -> Option<PathBuf> {
    if !root.is_dir() {
        return None;
    }
    // 1. 固定候选相对路径。
    for rel in DB_REL_CANDIDATES {
        let p = root.join(*rel);
        if p.is_file() {
            return Some(p);
        }
    }
    // 2. 受限深度兜底搜索。
    search_sqlite_bounded(root, DB_SEARCH_MAX_DEPTH)
}

/// 广度优先兜底搜索：优先返回 `db.sqlite`，否则返回第一个 `*.sqlite`。
///
/// 跳过隐藏目录与明显的非数据目录（node_modules / skills / memory），避免无谓遍历。
fn search_sqlite_bounded(root: &Path, max_depth: usize) -> Option<PathBuf> {
    let mut queue: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    let mut fallback: Option<PathBuf> = None;

    while let Some((dir, depth)) = queue.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if depth < max_depth && !name.starts_with('.') && !is_skip_dir(&name) {
                    queue.push((path, depth + 1));
                }
            } else if name == "db.sqlite" {
                return Some(path);
            } else if name.ends_with(".sqlite") && fallback.is_none() {
                fallback = Some(path);
            }
        }
    }
    fallback
}

/// 兜底搜索时应跳过的目录名（体积大且与统计库无关）。
fn is_skip_dir(name: &str) -> bool {
    matches!(
        name,
        "node_modules" | "skills" | "memory" | "logs" | "cache" | "skills-market"
    )
}

/// 解析 WorkBuddy SQLite 数据库路径（用户覆盖 → 环境变量 → 默认候选根目录）。
///
/// 返回 `None` 表示所有路径都未找到有效 DB 文件。
pub fn resolve_db_path(app: &AppHandle) -> Option<PathBuf> {
    // 0. 用户覆盖（最高优先级）。
    if let Some(dir) = read_override_dir(app) {
        if let Some(p) = find_db_in(Path::new(&dir)) {
            return Some(p);
        }
    }
    resolve_db_path_auto()
}

/// 纯自动检测（不含用户覆盖）：环境变量 → home 候选根目录。
fn resolve_db_path_auto() -> Option<PathBuf> {
    // 1. 环境变量（WorkBuddy 优先，CodeBuddy 回退）。
    for var in ENV_DIR_VARS {
        if let Ok(dir) = std::env::var(*var) {
            let trimmed = dir.trim();
            if !trimmed.is_empty() {
                if let Some(p) = find_db_in(Path::new(trimmed)) {
                    return Some(p);
                }
            }
        }
    }

    // 2. home 下的候选根目录。
    let home = dirs::home_dir()?;
    for rel in HOME_ROOT_CANDIDATES {
        let root = home.join(*rel);
        if let Some(p) = find_db_in(&root) {
            return Some(p);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// 查询
// ---------------------------------------------------------------------------

/// 查询今日 WorkBuddy token 使用量。
///
/// 以只读模式打开 SQLite 库，查询 `model_usage` 表中今日（本地时区零点起）的记录。
/// WAL 模式下多读一写不冲突；设 1s busy_timeout 容错短暂锁竞争。
pub fn query_today_stats(db_path: &Path) -> Result<TokenStats, String> {
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("打开数据库失败: {e}"))?;

    // 容错：WorkBuddy 主进程正在写入时短暂等待。
    conn.busy_timeout(std::time::Duration::from_millis(1000))
        .map_err(|e| format!("设置超时失败: {e}"))?;

    // 本地时区今日零点的 UTC 毫秒时间戳（model_usage.started_at 为 UTC ms）。
    let now = chrono::Local::now();
    let today_start_ms = chrono::Local
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .map(|dt| dt.timestamp_millis())
        .unwrap_or_else(|| now.timestamp_millis());
    // 今日汇总。
    let (input, output, total, calls): (i64, i64, i64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(computed_total_tokens), 0),
                    COUNT(*)
             FROM model_usage
             WHERE started_at >= ?1",
            rusqlite::params![today_start_ms],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| format!("查询今日用量失败: {e}"))?;

    // 今日各模型明细（按消耗降序）。
    let mut stmt = conn
        .prepare(
            "SELECT model_id, COUNT(*), COALESCE(SUM(computed_total_tokens), 0)
             FROM model_usage
             WHERE started_at >= ?1
             GROUP BY model_id
             ORDER BY 3 DESC",
        )
        .map_err(|e| format!("准备模型查询失败: {e}"))?;

    let active_models: Vec<ModelTokenRow> = stmt
        .query_map(rusqlite::params![today_start_ms], |row| {
            Ok(ModelTokenRow {
                model_id: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                calls: row.get::<_, i64>(1)? as u64,
                total_tokens: row.get::<_, Option<i64>>(2)?.unwrap_or(0) as u64,
            })
        })
        .map_err(|e| format!("查询模型用量失败: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(TokenStats {
        db_path: db_path.to_string_lossy().to_string(),
        today_input_tokens: input.max(0) as u64,
        today_output_tokens: output.max(0) as u64,
        today_total_tokens: total.max(0) as u64,
        today_calls: calls.max(0) as u64,
        active_models,
    })
}
