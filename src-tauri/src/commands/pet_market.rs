// Petdex 在线宠物市场命令模块。
//
// 作用：把 petdex.dev 的公开宠物图库接进本应用，支持浏览 / 搜索 / 排序 / 一键安装到本地。
//
// 数据来源（与官网 petdex.dev 图库完全同源）：
// - GET https://petdex.dev/api/pets/search
//     官网画廊使用的搜索接口（见上游 src/lib/pet-search.ts）：服务端完成过滤、
//     排序与分页，返回 total / facets / nextCursor。
//     排序键与官网一致（官网画廊默认 installed，搜索时自动切 curated）：
//       installed — 按安装量降序（官网默认）
//       curated   — 精选：featured 置顶 + 相关性/稳定洗牌（搜索时用）
//       popular   — 按点赞数降序
//       recent    — 按审核通过时间降序
//       alpha     — 按显示名字母序
//     单条记录含 slug / displayName / description / spritesheetPath（绝对 URL）/
//     kind / tags / vibes / submittedBy / spriteVersionNumber / dexNumber /
//     metrics{installCount, likeCount} 等。
// - GET /api/manifest/v2（回退 /api/manifest）
//     预生成的全量静态清单，仅作安装时的兜底数据源（search 按 slug 精确查找
//     失败时使用），也保留其解析与单测。
//
// 图集规格与本地渲染契约完全一致（8 列 × 192，行高 208）：
//   v1 = 1536×1872（9 行），v2 = 1536×2288（11 行）。直接复用 desktop_pet 的校验函数。
//
// 安全约束：
// - 资源 URL 必须为 https 且主机属于 petdex.dev（含子域），防止上游数据被污染后指向任意地址（SSRF）。
// - slug 规范化并限制为 [a-z0-9-]，避免路径穿越；与内置宠物同名的 slug 拒绝安装。
// - 下载体积设上限，流式累加并在超限时中止，避免磁盘/内存被写爆。
//
// 约定遵循 tauri-harness 后端规范：数据结构(camelCase) → 私有辅助 → #[tauri::command]，
// 命令返回 Result<T, String>，禁止 unwrap()/expect()。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::AppHandle;

use super::desktop_pet::{
    detect_image_ext, meta_to_info, pet_dir, pets_dir, read_image_size, read_meta,
    validate_atlas_size, write_meta, LocalPetInfo, LocalPetMeta, ATLAS_CELL_HEIGHT, ATLAS_WIDTH,
    BUILTIN_PET_IDS,
};
use super::now_rfc3339;

// --- 常量 ----------------------------------------------------------------

/// 官网画廊搜索接口（排序 / 过滤 / 分页都在服务端完成）。
const SEARCH_URL: &str = "https://petdex.dev/api/pets/search";

/// 全量静态清单（仅作安装时的兜底数据源）。
const MANIFEST_URL_COMPACT: &str = "https://petdex.dev/api/manifest/v2";
const MANIFEST_URL_LEGACY: &str = "https://petdex.dev/api/manifest";

/// 受信任的资源主机后缀（assets.petdex.dev 等）。
const TRUSTED_HOST_SUFFIX: &str = ".petdex.dev";
/// 受信任的根域。
const TRUSTED_ROOT_HOST: &str = "petdex.dev";

/// 单次 HTTP 请求超时。
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);
/// 精灵图体积上限（正常 WebP 图集 1~3MB，8MB 足够宽松）。
const MAX_SPRITE_BYTES: usize = 8 * 1024 * 1024;
/// 默认每页条数（与官网画廊 PAGE_SIZE 一致）。
const DEFAULT_PAGE_SIZE: u32 = 24;
/// 每页条数上限（与官网 search 的 MAX_LIMIT 一致）。
const MAX_PAGE_SIZE: u32 = 60;
/// 安装时按 slug 精确查找的最大结果数（search 接口的 limit 上限）。
const RESOLVE_LIMIT: u32 = 60;

/// 官网支持的排序键（与上游 SortKey 一致），默认 installed。
const SORT_KEYS: [&str; 5] = ["curated", "popular", "installed", "alpha", "recent"];

// --- 数据结构（与前端共享，统一 camelCase） --------------------------------

/// 市场中的一只宠物（搜索结果条目）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MarketPet {
    pub slug: String,
    pub display_name: String,
    /// 分类：character / creature / object（可能为空）。
    pub kind: String,
    /// 投稿者昵称（可能为 null）。
    pub submitted_by: Option<String>,
    /// 精灵图地址（远程 https，前端 PetThumb 可直接用作背景图）。
    pub spritesheet_url: String,
    /// 图集版本：1 = 9 行，2 = 11 行。
    pub sprite_version_number: u32,
    /// 描述（上游可能为空）。
    pub description: Option<String>,
    /// 标签（官网 tags + vibes 合并，用于本地 meta 与详情展示）。
    pub tags: Vec<String>,
    /// 图鉴编号（按审核通过顺序，官网卡片上的 No.xxx）。
    pub dex_number: Option<u32>,
    /// 官网精选标记。
    pub featured: bool,
    /// 安装量（官网「最多安装」排序依据，可能缺失）。
    pub install_count: Option<u64>,
    /// 点赞数（可能缺失）。
    pub like_count: Option<u64>,
}

/// 市场分页结果。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MarketPage {
    /// 当前页条目。
    pub items: Vec<MarketPet>,
    /// 过滤后的总条数（分页器用）。
    pub total: usize,
    /// 当前页码（从 1 开始）。
    pub page: u32,
    /// 每页条数（已钳制）。
    pub page_size: u32,
    /// 全部可选分类及数量（供筛选下拉，不随关键词变化；按数量降序）。
    pub kinds: Vec<MarketKindCount>,
}

/// 分类计数（官网 facets.kinds）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MarketKindCount {
    pub kind: String,
    pub count: usize,
}

// --- 上游 DTO（search 接口响应，字段可能缺失，宽容解析） --------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchResponse {
    #[serde(default)]
    pets: Vec<SearchPetDto>,
    #[serde(default)]
    total: Option<usize>,
    #[serde(default)]
    facets: Option<SearchFacetsDto>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SearchPetDto {
    slug: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    spritesheet_path: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    vibes: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    submitted_by: Option<SearchAuthorDto>,
    #[serde(default)]
    sprite_version_number: Option<u32>,
    #[serde(default)]
    dex_number: Option<u32>,
    #[serde(default)]
    featured: bool,
    #[serde(default)]
    metrics: Option<SearchMetricsDto>,
}

#[derive(Deserialize, Clone)]
struct SearchAuthorDto {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SearchMetricsDto {
    #[serde(default)]
    install_count: Option<u64>,
    #[serde(default)]
    like_count: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchFacetsDto {
    #[serde(default)]
    kinds: HashMap<String, usize>,
}

// --- 上游 DTO（manifest 清单，安装兜底用） ----------------------------------

/// 上游紧凑清单（v2）。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompactManifest {
    v: u32,
    asset_base: String,
    fields: Vec<String>,
    pets: Vec<Vec<serde_json::Value>>,
}

/// 上游旧版清单中的单条宠物。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyPet {
    slug: String,
    display_name: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    submitted_by: Option<String>,
    spritesheet_url: String,
    #[serde(default)]
    sprite_version_number: Option<u32>,
}

/// 上游旧版清单。
#[derive(Deserialize)]
struct LegacyManifest {
    pets: Vec<LegacyPet>,
}

// --- 私有辅助：缓存与 HTTP 客户端 ------------------------------------------

/// manifest 内存缓存槽（安装兜底用，首次访问时初始化）。
fn manifest_slot() -> &'static Mutex<Option<CachedManifest>> {
    static SLOT: OnceLock<Mutex<Option<CachedManifest>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// manifest 内存缓存条目。
struct CachedManifest {
    fetched_at: Instant,
    pets: Vec<MarketPet>,
}

/// 进程内共享的 HTTP 客户端（连接复用 + 统一超时）。
///
/// 用 `OnceLock<Result<..>>` 缓存构建结果：TLS 后端初始化失败时把错误也缓存下来，
/// 避免每次调用都重试构建（同时满足「不使用 unwrap/expect」的约束）。
fn http_client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(HTTP_TIMEOUT)
                .user_agent(concat!("workbuddy-pet/", env!("CARGO_PKG_VERSION")))
                .build()
                .map_err(|e| format!("初始化 HTTP 客户端失败: {}", e))
        })
        .as_ref()
        .map_err(|e| e.clone())
}

// --- 私有辅助：URL / slug / 排序键 校验 ------------------------------------

/// 校验一个已解析的 URL 是否可信（https + petdex 域）。
fn ensure_trusted_url(url: reqwest::Url) -> Result<String, String> {
    if url.scheme() != "https" {
        return Err(format!("不安全的资源地址协议: {}", url.scheme()));
    }
    let host = url
        .host_str()
        .ok_or_else(|| "资源地址缺少主机名".to_string())?;
    let host_lower = host.to_ascii_lowercase();
    if host_lower != TRUSTED_ROOT_HOST && !host_lower.ends_with(TRUSTED_HOST_SUFFIX) {
        return Err(format!("不受信任的资源主机: {}", host));
    }
    Ok(url.to_string())
}

/// 解析清单中的资源地址：绝对地址直接校验；相对地址相对 `asset_base` 拼接后再校验。
fn resolve_asset_url(raw: &str, asset_base: &str) -> Result<String, String> {
    if let Ok(url) = reqwest::Url::parse(raw) {
        return ensure_trusted_url(url);
    }
    let base = reqwest::Url::parse(asset_base).map_err(|e| format!("清单 assetBase 非法: {}", e))?;
    let joined = base
        .join(raw)
        .map_err(|e| format!("清单资源地址无法解析: {}", e))?;
    ensure_trusted_url(joined)
}

/// 直接校验绝对资源地址。
fn ensure_trusted_asset_url(raw: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(raw).map_err(|e| format!("资源地址非法: {}", e))?;
    ensure_trusted_url(url)
}

/// 规范化市场 slug：转小写并校验字符集（与本地宠物目录命名约束一致）。
///
/// 上游 slug 均为 `[a-z0-9-]`，但内容由第三方投稿生成，这里做防御性收敛：
/// 规范化后仍不符合约束（含点、斜杠等非法字符）直接拒绝，杜绝路径穿越。
fn normalize_slug(raw: &str) -> Result<String, String> {
    let slug = raw.trim().to_ascii_lowercase();
    if slug.is_empty() || slug.len() > 64 {
        return Err(format!("非法的宠物标识: {}", raw));
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(format!("非法的宠物标识: {}", raw));
    }
    Ok(slug)
}

/// 归一化排序键：仅接受官网支持的 5 种，其余（含空）回退官网默认 installed。
fn normalize_sort(raw: Option<&str>) -> &'static str {
    let key = raw.unwrap_or_default().trim().to_ascii_lowercase();
    match SORT_KEYS.iter().find(|k| **k == key) {
        Some(k) => k,
        None => "installed",
    }
}

// --- 私有辅助：search 结果转换 --------------------------------------------

/// tags + vibes 合并去重（写入本地 meta.tags，详情弹窗也直接展示）。
fn merge_tags(tags: Vec<String>, vibes: Vec<String>) -> Vec<String> {
    let mut merged: Vec<String> = Vec::with_capacity(tags.len() + vibes.len());
    for value in tags.into_iter().chain(vibes) {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() && !merged.contains(&trimmed) {
            merged.push(trimmed);
        }
    }
    merged
}

/// search DTO → 前端 MarketPet。精灵图地址不可信/缺失的条目返回 None（跳过）。
fn search_pet_to_market(pet: &SearchPetDto) -> Option<MarketPet> {
    let spritesheet_raw = pet.spritesheet_path.as_deref()?;
    let spritesheet_url = ensure_trusted_asset_url(spritesheet_raw).ok()?;
    let submitted_by = pet
        .submitted_by
        .as_ref()
        .and_then(|author| author.name.clone())
        .filter(|name| !name.is_empty());
    Some(MarketPet {
        slug: pet.slug.clone(),
        display_name: pet
            .display_name
            .clone()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| pet.slug.clone()),
        kind: pet.kind.clone().unwrap_or_default(),
        submitted_by,
        spritesheet_url,
        sprite_version_number: pet.sprite_version_number.unwrap_or(1),
        description: pet.description.clone().filter(|d| !d.trim().is_empty()),
        tags: merge_tags(pet.tags.clone(), pet.vibes.clone()),
        dex_number: pet.dex_number,
        featured: pet.featured,
        install_count: pet.metrics.as_ref().and_then(|m| m.install_count),
        like_count: pet.metrics.as_ref().and_then(|m| m.like_count),
    })
}

/// 分类 facets → 按数量降序的分类列表（与官网筛选 chip 的排列一致）。
fn kinds_from_facets(facets: &SearchFacetsDto) -> Vec<MarketKindCount> {
    let mut kinds: Vec<MarketKindCount> = facets
        .kinds
        .iter()
        .filter(|(kind, _)| !kind.is_empty())
        .map(|(kind, count)| MarketKindCount {
            kind: kind.clone(),
            count: *count,
        })
        .collect();
    kinds.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.kind.cmp(&b.kind)));
    kinds
}

// --- 私有辅助：HTTP 拉取 ---------------------------------------------------

/// 拉取并解析一份 JSON（带超时与状态码检查）。
async fn fetch_json(request: reqwest::RequestBuilder) -> Result<serde_json::Value, String> {
    let response = request
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| format!("请求 petdex 失败: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("petdex 返回 HTTP {}", response.status().as_u16()));
    }
    response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("解析 petdex 响应失败: {}", e))
}

/// 调官网搜索接口拉一页结果。
///
/// 返回 (条目, 总数, 分类 facets)。条目里不可信/缺精灵图地址的会被跳过。
#[allow(clippy::too_many_arguments)]
async fn fetch_search_page(
    client: &reqwest::Client,
    query: Option<&str>,
    kind: Option<&str>,
    sort: &str,
    cursor: u32,
    limit: u32,
) -> Result<(Vec<MarketPet>, Option<usize>, Vec<MarketKindCount>), String> {
    let mut request = client.get(SEARCH_URL).query(&[
        ("sort", sort.to_string()),
        ("cursor", cursor.to_string()),
        ("limit", limit.to_string()),
        ("includeMeta", "1".to_string()),
    ]);
    if let Some(q) = query {
        request = request.query(&[("q", q)]);
    }
    if let Some(k) = kind {
        request = request.query(&[("kinds", k)]);
    }

    let value = fetch_json(request).await?;
    let data: SearchResponse =
        serde_json::from_value(value).map_err(|e| format!("解析搜索结果失败: {}", e))?;

    let items: Vec<MarketPet> = data
        .pets
        .iter()
        .filter_map(search_pet_to_market)
        .collect();
    let kinds = data
        .facets
        .as_ref()
        .map(kinds_from_facets)
        .unwrap_or_default();
    Ok((items, data.total, kinds))
}

// --- 私有辅助：manifest 兜底（安装用） --------------------------------------

/// 从紧凑清单原始 JSON 解析宠物列表（仅取安装所需字段）。
fn parse_compact_manifest(value: &serde_json::Value) -> Result<Vec<MarketPet>, String> {
    let raw: CompactManifest =
        serde_json::from_value(value.clone()).map_err(|e| format!("解析紧凑清单失败: {}", e))?;
    if raw.v != 2 {
        return Err(format!("未知的清单版本: {}", raw.v));
    }
    // 按 fields 声明的字段名定位列索引（而非硬编码下标），上游调整列顺序也不会解析错位。
    let column = |name: &str| -> Result<usize, String> {
        raw.fields
            .iter()
            .position(|f| f == name)
            .ok_or_else(|| format!("清单缺少字段: {}", name))
    };
    let i_slug = column("slug")?;
    let i_name = column("displayName")?;
    let i_kind = column("kind")?;
    let i_author = column("submittedBy")?;
    let i_sprite = column("spritesheet")?;
    let i_version = column("spriteVersionNumber")?;

    let mut pets = Vec::with_capacity(raw.pets.len());
    for (index, row) in raw.pets.iter().enumerate() {
        let slug = row
            .get(i_slug)
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("清单第 {} 条缺少 slug", index))?;
        let sprite_raw = row
            .get(i_sprite)
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("清单第 {} 条缺少 spritesheet", index))?;
        // 单条地址不可信时跳过该条，而不是让整份清单失败（一条脏数据不该毁掉兜底数据源）。
        let Ok(spritesheet_url) = resolve_asset_url(sprite_raw, &raw.asset_base) else {
            continue;
        };
        pets.push(MarketPet {
            slug: slug.to_string(),
            display_name: row
                .get(i_name)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(slug)
                .to_string(),
            kind: row
                .get(i_kind)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            submitted_by: row
                .get(i_author)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            spritesheet_url,
            sprite_version_number: row
                .get(i_version)
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .filter(|v| *v == 1 || *v == 2)
                .unwrap_or(1),
            description: None,
            tags: vec![],
            dex_number: None,
            featured: false,
            install_count: None,
            like_count: None,
        });
    }
    Ok(pets)
}

/// 从旧版清单原始 JSON 解析宠物列表（仅取安装所需字段）。
fn parse_legacy_manifest(value: &serde_json::Value) -> Result<Vec<MarketPet>, String> {
    let raw: LegacyManifest =
        serde_json::from_value(value.clone()).map_err(|e| format!("解析清单失败: {}", e))?;
    let mut pets = Vec::with_capacity(raw.pets.len());
    for pet in raw.pets {
        let Ok(spritesheet_url) = ensure_trusted_asset_url(&pet.spritesheet_url) else {
            continue;
        };
        pets.push(MarketPet {
            slug: pet.slug,
            display_name: pet.display_name,
            kind: pet.kind.unwrap_or_default(),
            submitted_by: pet.submitted_by,
            spritesheet_url,
            sprite_version_number: pet.sprite_version_number.unwrap_or(1),
            description: None,
            tags: vec![],
            dex_number: None,
            featured: false,
            install_count: None,
            like_count: None,
        });
    }
    Ok(pets)
}

/// 从远端拉取清单：优先紧凑版，失败回退旧版。
async fn fetch_manifest_remote() -> Result<Vec<MarketPet>, String> {
    let client = http_client()?;
    let compact_error: String = match fetch_json(client.get(MANIFEST_URL_COMPACT)).await {
        Ok(value) => match parse_compact_manifest(&value) {
            Ok(pets) if !pets.is_empty() => return Ok(pets),
            Ok(_) => "紧凑清单为空".to_string(),
            Err(e) => e,
        },
        Err(e) => e,
    };

    match fetch_json(client.get(MANIFEST_URL_LEGACY)).await {
        Ok(value) => {
            parse_legacy_manifest(&value).map_err(|e| format!("{}（紧凑清单: {}）", e, compact_error))
        }
        Err(e) => Err(format!(
            "拉取宠物清单失败: {}（紧凑清单: {}）",
            e, compact_error
        )),
    }
}

/// 取 manifest（带内存缓存，安装兜底专用，浏览不走这里）。
async fn load_manifest() -> Result<Vec<MarketPet>, String> {
    {
        // 短锁：只读取，不跨 await 持锁。
        if let Ok(guard) = manifest_slot().lock() {
            if let Some(cache) = guard.as_ref() {
                if cache.fetched_at.elapsed() < Duration::from_secs(10 * 60) {
                    return Ok(cache.pets.clone());
                }
            }
        }
    }

    let pets = fetch_manifest_remote().await?;
    if let Ok(mut guard) = manifest_slot().lock() {
        *guard = Some(CachedManifest {
            fetched_at: Instant::now(),
            pets: pets.clone(),
        });
    }
    Ok(pets)
}

/// 按 slug 精确查找一只市场宠物：优先官网搜索接口（数据更全），
/// 找不到（如 slug 的子串匹配结果超过一页上限）时回退全量 manifest。
async fn find_market_pet(slug: &str) -> Result<MarketPet, String> {
    let client = http_client()?;
    let search = fetch_search_page(client, Some(slug), None, "alpha", 0, RESOLVE_LIMIT).await;
    if let Ok((items, ..)) = &search {
        if let Some(hit) = items.iter().find(|pet| pet.slug.eq_ignore_ascii_case(slug)) {
            return Ok(hit.clone());
        }
    }
    // 回退：全量 manifest。
    let pets = load_manifest().await?;
    pets.into_iter()
        .find(|pet| pet.slug.eq_ignore_ascii_case(slug))
        .ok_or_else(|| format!("市场中没有找到宠物「{}」", slug))
}

/// 流式下载资源到内存，超过 `max_bytes` 立即中止（避免超大响应写爆内存/磁盘）。
async fn download_bytes(
    client: &reqwest::Client,
    url: &str,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载资源失败: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("下载资源失败: HTTP {}", response.status().as_u16()));
    }
    if let Some(length) = response.content_length() {
        if length as usize > max_bytes {
            return Err(format!("资源体积超出上限（{} 字节）", length));
        }
    }

    let mut buffer: Vec<u8> = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("读取资源数据失败: {}", e))?
    {
        if buffer.len() + chunk.len() > max_bytes {
            return Err("资源体积超出上限，已中止下载".to_string());
        }
        buffer.extend_from_slice(&chunk);
    }
    if buffer.is_empty() {
        return Err("下载到的资源为空".to_string());
    }
    Ok(buffer)
}

// --- 命令：市场浏览 --------------------------------------------------------

/// 拉取市场宠物（支持关键词 / 分类过滤 + 官网同款排序 + 分页）。
///
/// 直接转发到官网画廊使用的 `/api/pets/search`：排序、过滤、分页全部在服务端完成，
/// 本地只做字段映射与安全校验。`cursor = (page-1) * pageSize` 与官网的 offset
/// 游标分页兼容。
#[tauri::command]
pub async fn fetch_market_pets(
    query: Option<String>,
    kind: Option<String>,
    sort: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<MarketPage, String> {
    let sort_key = normalize_sort(sort.as_deref());
    let size = page_size.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE);
    let page_number = page.unwrap_or(1).max(1);
    let cursor = (page_number - 1).saturating_mul(size);

    let keyword = query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let kind_filter = kind
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let client = http_client()?;
    let (items, total, kinds) = fetch_search_page(
        client,
        keyword.as_deref(),
        kind_filter.as_deref(),
        sort_key,
        cursor,
        size,
    )
    .await?;

    Ok(MarketPage {
        items,
        total: total.unwrap_or_default(),
        page: page_number,
        page_size: size,
        kinds,
    })
}

// --- 命令：市场安装 --------------------------------------------------------

/// 按精灵图真实像素尺寸推断图集版本：1 = 9 行（1536×1872），2 = 11 行（1536×2288）。
///
/// 不能直接采信上游标注的 `spriteVersionNumber`：实测 curated/boba 标注为 1，
/// 实际图集却是 1536×2288 的 11 行 v2。版本号写错会让本地渲染与缩略图取帧错位，
/// 故以图片真实尺寸为准（兼容 2x 等干净缩放），上游值仅在尺寸无法识别时兜底。
fn infer_sprite_version(bytes: &[u8], fallback: u32) -> u32 {
    let Some((width, height)) = read_image_size(bytes) else {
        return fallback;
    };
    if width == 0 || height == 0 {
        return fallback;
    }
    // 行数 = 图高 / 单元格高，而单元格高随缩放比（宽 / 1536）等比放大，
    // 故 rows = height * 1536 / (width * 208)；四舍五入消除缩放取整误差。
    let rows = ((height as f64) * (ATLAS_WIDTH as f64)
        / ((width as f64) * (ATLAS_CELL_HEIGHT as f64)))
        .round() as u32;
    if rows >= 11 {
        2
    } else {
        1
    }
}

/// 安装一只市场宠物到本地。
///
/// 流程：按 slug 精确查找 → 校验资源地址 → 下载精灵图 → 校验图集尺寸 → 落盘
/// `<app_data>/pets/<slug>/{spritesheet.<ext>,meta.json}`（source: "downloaded"）。
///
/// 幂等：本地已存在同 slug 的有效宠物时直接返回，不重复下载。
/// 原子性：内容先下载到内存并校验通过后再落盘；写 meta 失败则清理目录，不留半成品。
#[tauri::command]
pub async fn install_market_pet(app: AppHandle, slug: String) -> Result<LocalPetInfo, String> {
    let slug = normalize_slug(&slug)?;
    if BUILTIN_PET_IDS.contains(&slug.as_str()) {
        return Err(format!("「{}」是内置宠物标识，不能从市场覆盖安装", slug));
    }

    let dir = pet_dir(&app, &slug)?;
    // 已安装（目录存在且能读出 meta）：直接返回，避免重复下载。
    if dir.is_dir() {
        if let Ok(Some(meta)) = read_meta(&dir) {
            if dir.join(&meta.spritesheet_file).exists() {
                return Ok(meta_to_info(&dir, &meta));
            }
        }
    }

    let pet = find_market_pet(&slug).await?;
    let spritesheet_url = ensure_trusted_asset_url(&pet.spritesheet_url)?;
    let client = http_client()?;
    let sprite_bytes = download_bytes(client, &spritesheet_url, MAX_SPRITE_BYTES).await?;

    // 图集格式与尺寸校验：与本地导入同一套契约，避免装进来一只渲染必然失败的宠物。
    let ext = detect_image_ext(&sprite_bytes)
        .ok_or_else(|| "市场精灵图格式不支持（仅支持 PNG / WebP）".to_string())?;
    validate_atlas_size(&sprite_bytes)?;

    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("创建宠物目录失败: {}", e))?;

    // 统一落盘为 spritesheet.<ext>（上游文件名可能是 sprite.webp / sprite-v2.webp 等，
    // 以本地规范为准并让 meta.spritesheet_file 指向真实文件）。
    let spritesheet_file = format!("spritesheet.{}", ext);
    if let Err(e) = tokio::fs::write(dir.join(&spritesheet_file), &sprite_bytes).await {
        let _ = tokio::fs::remove_dir_all(&dir).await;
        return Err(format!("写入精灵图失败: {}", e));
    }

    let meta = LocalPetMeta {
        id: slug.clone(),
        display_name: pet.display_name,
        description: pet.description,
        kind: Some(pet.kind).filter(|kind| !kind.is_empty()),
        tags: pet.tags,
        source: "downloaded".to_string(),
        spritesheet_file,
        poster_file: None,
        spritesheet_url: Some(spritesheet_url),
        version: None,
        // 以图片真实尺寸为准推断版本（上游标注可能错误），详见 infer_sprite_version。
        sprite_version_number: Some(infer_sprite_version(
            &sprite_bytes,
            pet.sprite_version_number,
        )),
        installed_at: Some(now_rfc3339()),
    };

    if let Err(e) = write_meta(&dir, &meta) {
        // 写元数据失败：清理已落盘内容，不留无法管理的半成品目录。
        let _ = tokio::fs::remove_dir_all(&dir).await;
        return Err(e);
    }

    Ok(meta_to_info(&dir, &meta))
}

/// 本地宠物根目录（供前端展示安装位置 / 诊断用）。
#[tauri::command]
pub fn market_pets_dir(app: AppHandle) -> Result<String, String> {
    Ok(pets_dir(&app)?.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- slug 规范化 --------------------------------------------------------

    #[test]
    fn normalize_slug_accepts_valid_ids() {
        assert_eq!(normalize_slug("boba").unwrap(), "boba");
        assert_eq!(normalize_slug("GabirU").unwrap(), "gabiru");
        assert_eq!(
            normalize_slug("  aurelion-sol-2  ").unwrap(),
            "aurelion-sol-2"
        );
    }

    #[test]
    fn normalize_slug_rejects_path_traversal_and_symbols() {
        assert!(normalize_slug("").is_err());
        assert!(normalize_slug("../etc/passwd").is_err());
        assert!(normalize_slug("a/b").is_err());
        assert!(normalize_slug("a.b").is_err());
        assert!(normalize_slug("有中文").is_err());
        assert!(normalize_slug(&"a".repeat(65)).is_err());
    }

    // --- 排序键 ------------------------------------------------------------

    #[test]
    fn normalize_sort_accepts_official_keys_and_defaults_to_installed() {
        assert_eq!(normalize_sort(Some("curated")), "curated");
        assert_eq!(normalize_sort(Some("POPULAR")), "popular");
        assert_eq!(normalize_sort(Some(" installed ")), "installed");
        assert_eq!(normalize_sort(Some("alpha")), "alpha");
        assert_eq!(normalize_sort(Some("recent")), "recent");
        // 未知 / 空 / 缺省 → 官网默认 installed。
        assert_eq!(normalize_sort(Some("hot")), "installed");
        assert_eq!(normalize_sort(Some("")), "installed");
        assert_eq!(normalize_sort(None), "installed");
    }

    // --- 资源地址可信校验 ---------------------------------------------------

    #[test]
    fn trusted_url_accepts_petdex_hosts() {
        assert!(ensure_trusted_asset_url("https://assets.petdex.dev/pets/a/sprite.webp").is_ok());
        assert!(ensure_trusted_asset_url("https://petdex.dev/api/manifest").is_ok());
    }

    #[test]
    fn trusted_url_rejects_other_hosts_and_schemes() {
        assert!(ensure_trusted_asset_url("https://evil.example.com/a.webp").is_err());
        // 相似但不同域，不得放行（后缀匹配必须带点号）。
        assert!(ensure_trusted_asset_url("https://notpetdex.dev/a.webp").is_err());
        assert!(ensure_trusted_asset_url("https://petdex.dev.evil.com/a.webp").is_err());
        assert!(ensure_trusted_asset_url("http://assets.petdex.dev/a.webp").is_err());
        assert!(ensure_trusted_asset_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn resolve_asset_url_joins_relative_paths_against_asset_base() {
        let url =
            resolve_asset_url("pets/boba/sprite.webp", "https://assets.petdex.dev/").unwrap();
        assert_eq!(url, "https://assets.petdex.dev/pets/boba/sprite.webp");
    }

    // --- search 结果转换 ---------------------------------------------------

    /// 官网 search 接口的真实返回样例（节选自 live 探测）。
    fn search_value() -> serde_json::Value {
        serde_json::json!({
            "pets": [
                {
                    "slug": "boba",
                    "displayName": "Boba",
                    "description": "A tiny otter sipping bubble tea.",
                    "spritesheetPath": "https://assets.petdex.dev/curated/boba/sprite-v2.webp",
                    "zipUrl": "https://assets.petdex.dev/curated/boba/v2/boba.zip",
                    "featured": true,
                    "kind": "creature",
                    "vibes": ["cozy", "playful"],
                    "tags": ["otter", "drink", "cozy"],
                    "submittedBy": { "name": "railly" },
                    "spriteVersionNumber": 1,
                    "dexNumber": 69,
                    "metrics": { "installCount": 10322, "likeCount": 21 }
                },
                {
                    "slug": "bad",
                    "displayName": "Bad",
                    "spritesheetPath": "https://evil.example.com/sprite.webp"
                }
            ],
            "total": 4867,
            "facets": { "kinds": { "character": 2432, "creature": 1905, "object": 530 } }
        })
    }

    #[test]
    fn search_pet_to_market_maps_fields_and_merges_tags() {
        let data: SearchResponse = serde_json::from_value(search_value()).unwrap();
        assert_eq!(data.pets.len(), 2);
        let pet = search_pet_to_market(&data.pets[0]).unwrap();
        assert_eq!(pet.slug, "boba");
        assert_eq!(pet.display_name, "Boba");
        assert_eq!(pet.submitted_by.as_deref(), Some("railly"));
        assert_eq!(pet.dex_number, Some(69));
        assert!(pet.featured);
        assert_eq!(pet.install_count, Some(10322));
        // tags + vibes 合并去重（"cozy" 在两边都出现，只保留一份）。
        assert_eq!(pet.tags, vec!["otter", "drink", "cozy", "playful"]);
        // 不可信主机的条目被跳过。
        assert!(search_pet_to_market(&data.pets[1]).is_none());
    }

    #[test]
    fn kinds_from_facets_sorts_by_count_desc() {
        let data: SearchResponse = serde_json::from_value(search_value()).unwrap();
        let kinds = kinds_from_facets(data.facets.as_ref().unwrap());
        assert_eq!(
            kinds.iter().map(|k| k.kind.as_str()).collect::<Vec<_>>(),
            vec!["character", "creature", "object"]
        );
    }

    #[test]
    fn search_response_tolerates_missing_optional_fields() {
        let value = serde_json::json!({
            "pets": [
                {
                    "slug": "x",
                    "displayName": "X",
                    "spritesheetPath": "https://assets.petdex.dev/pets/x/sprite.webp"
                }
            ]
        });
        let data: SearchResponse = serde_json::from_value(value).unwrap();
        let pet = search_pet_to_market(&data.pets[0]).unwrap();
        assert!(pet.description.is_none());
        assert!(pet.tags.is_empty());
        assert_eq!(pet.sprite_version_number, 1);
        assert_eq!(pet.install_count, None);
        // 缺精灵图地址的条目无法展示/安装，应被跳过。
        let no_sprite: SearchResponse =
            serde_json::from_value(serde_json::json!({ "pets": [ { "slug": "y" } ] })).unwrap();
        assert!(search_pet_to_market(&no_sprite.pets[0]).is_none());
    }

    // --- manifest 兜底解析 -------------------------------------------------

    fn manifest_value() -> serde_json::Value {
        serde_json::json!({
            "v": 2,
            "assetBase": "https://assets.petdex.dev/",
            "fields": [
                "slug", "displayName", "kind", "submittedBy",
                "spritesheet", "petJson", "zip", "spriteVersionNumber"
            ],
            "total": 1,
            "pets": [
                [
                    "homelander", "Homelander", "character", "Serhat",
                    "pets/homelander-dbbb/sprite.webp",
                    "pets/homelander-dbbb/petjson.json",
                    null, 1
                ]
            ]
        })
    }

    #[test]
    fn parse_compact_manifest_maps_fields_by_name() {
        let pets = parse_compact_manifest(&manifest_value()).unwrap();
        assert_eq!(pets.len(), 1);
        assert_eq!(pets[0].slug, "homelander");
        assert_eq!(pets[0].display_name, "Homelander");
        assert_eq!(pets[0].kind, "character");
        assert_eq!(pets[0].submitted_by.as_deref(), Some("Serhat"));
        assert_eq!(
            pets[0].spritesheet_url,
            "https://assets.petdex.dev/pets/homelander-dbbb/sprite.webp"
        );
        assert_eq!(pets[0].sprite_version_number, 1);
    }

    #[test]
    fn parse_compact_manifest_rejects_unknown_version_and_missing_fields() {
        let mut bad_version = manifest_value();
        bad_version["v"] = serde_json::json!(3);
        assert!(parse_compact_manifest(&bad_version).is_err());

        let mut missing = manifest_value();
        missing["fields"] = serde_json::json!(["slug", "displayName"]);
        assert!(parse_compact_manifest(&missing).is_err());
    }

    #[test]
    fn parse_legacy_manifest_filters_untrusted_hosts() {
        let value = serde_json::json!({
            "pets": [
                {
                    "slug": "boba",
                    "displayName": "Boba",
                    "kind": "character",
                    "submittedBy": "railly",
                    "spritesheetUrl": "https://assets.petdex.dev/pets/boba-hash/sprite.webp",
                    "petJsonUrl": "https://assets.petdex.dev/pets/boba-hash/petjson.json",
                    "zipUrl": null
                },
                {
                    "slug": "bad",
                    "displayName": "Bad",
                    "kind": "object",
                    "submittedBy": null,
                    "spritesheetUrl": "https://evil.example.com/sprite.webp",
                    "petJsonUrl": "https://assets.petdex.dev/pets/bad/petjson.json",
                    "zipUrl": null
                }
            ]
        });
        let pets = parse_legacy_manifest(&value).unwrap();
        assert_eq!(pets.len(), 1);
        assert_eq!(pets[0].slug, "boba");
        // 缺省版本号按 v1 处理。
        assert_eq!(pets[0].sprite_version_number, 1);
    }

    // --- 图集版本推断 ------------------------------------------------------

    /// 构造最小 PNG 头（签名 + IHDR 宽高，共 24 字节；尺寸解析只需前 24 字节）。
    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        v.extend_from_slice(&[0, 0, 0, 13]);
        v.extend_from_slice(b"IHDR");
        v.extend_from_slice(&width.to_be_bytes());
        v.extend_from_slice(&height.to_be_bytes());
        v
    }

    #[test]
    fn infer_sprite_version_prefers_real_image_size_over_manifest() {
        // 清单标注 1，但实际是 11 行图（curated/boba 的真实情况）→ 必须判为 v2。
        assert_eq!(infer_sprite_version(&png_header(1536, 2288), 1), 2);
        // 标准 v1：清单即便标 2 也要纠正回 v1。
        assert_eq!(infer_sprite_version(&png_header(1536, 1872), 1), 1);
        assert_eq!(infer_sprite_version(&png_header(1536, 1872), 2), 1);
        // 2x 干净缩放：v1 = 3072×3744，v2 = 3072×4576。
        assert_eq!(infer_sprite_version(&png_header(3072, 3744), 2), 1);
        assert_eq!(infer_sprite_version(&png_header(3072, 4576), 1), 2);
        // 尺寸无法识别 → 沿用上游标注。
        assert_eq!(infer_sprite_version(b"not an image", 2), 2);
    }

    // --- tags 合并 ---------------------------------------------------------

    #[test]
    fn merge_tags_dedupes_and_trims() {
        assert_eq!(
            merge_tags(
                vec!["otter".into(), " cozy ".into()],
                vec!["cozy".into(), "playful".into(), "  ".into()]
            ),
            vec!["otter", "cozy", "playful"]
        );
        assert!(merge_tags(vec![], vec![]).is_empty());
    }

    // --- 联网冒烟测试（默认忽略），手动运行： -------------------------------
    /// `cargo test pet_market::tests::live_probe -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_probe_search_and_sprite() {
        let client = http_client().expect("client");
        let (items, total, kinds) =
            fetch_search_page(client, None, None, "installed", 0, 3)
                .await
                .expect("search fetch");
        println!("LIVE total = {total:?}");
        println!(
            "LIVE kinds = {:?}",
            kinds.iter().map(|k| (k.kind.clone(), k.count)).collect::<Vec<_>>()
        );
        let sample = &items[0];
        println!(
            "LIVE top = {} installs={:?} dex={:?}",
            sample.slug, sample.install_count, sample.dex_number
        );
        let bytes = download_bytes(client, &sample.spritesheet_url, MAX_SPRITE_BYTES)
            .await
            .expect("download sprite");
        println!(
            "LIVE sprite bytes={} size={:?} inferred_ver={}",
            bytes.len(),
            read_image_size(&bytes),
            infer_sprite_version(&bytes, sample.sprite_version_number)
        );
        validate_atlas_size(&bytes).expect("atlas size");
    }
}
