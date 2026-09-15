//! WorkBuddy 免费模型驱动的 AI 台词生成。
//!
//! WorkBuddy 桌面版把用户在「模型管理」里配置的模型写在
//! `~/.workbuddy/models.json`（CodeBuddy 同源引擎回退 `~/.codebuddy/models.json`），
//! 每个条目是**标准 OpenAI 兼容**的 `{ id, name, vendor, url, apiKey, ... }`。
//!
//! 本模块复用这份配置：直接以 `POST {url}` 调 `/chat/completions` 生成一句宠物台词，
//! 无需用户在本应用里另行填写任何 API Key。
//!
//! 设计要点：
//! - **只复用 `vendor = "Custom"` 的条目**（见 [`is_custom_model`]）。WorkBuddy 官方 /
//!   原生渠道的模型带自己的 vendor 标识，通常附有客户端侧鉴权与用量约束，不属于本应用
//!   的使用范围，一律跳过。因此实际调用的是**用户自己配置的第三方接口**，
//!   与 WorkBuddy 官方额度无关。
//! - 用户未指定模型时，默认走 [`PREFERRED_MODEL_HOSTS`] 里的渠道（见
//!   [`pick_default_model`]），而不是简单取列表第一个。
//! - 只读读取 WorkBuddy 配置，绝不写入（避免影响 WorkBuddy 自身）。
//! - **apiKey 绝不出后端**：对外只暴露 [`WorkBuddyModelInfo`]（id / name / vendor）。
//! - 台词生成是「锦上添花」：任何失败都返回 Err，由前端回退到内置固定语录。
//! - 提示词构造与文本清洗都是纯函数，便于单测。

use std::path::PathBuf;
use std::time::Duration;

use chrono::{Datelike, Timelike};
use serde::{Deserialize, Serialize};

/// 模型配置文件的候选相对路径（home 下，按优先级）。
const MODELS_FILE_REL_CANDIDATES: &[&str] = &[".workbuddy/models.json", ".codebuddy/models.json"];

/// 单次生成请求的超时（秒）。宠物台词必须快速返回，超时即回退固定语录。
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// 生成的台词最大长度（字符数，按码点计）。超出截断并补省略号。
const MAX_LINE_CHARS: usize = 90;

/// 默认模型的优先渠道（按顺序匹配 `models.json` 里 url 的**主机名**）。
///
/// 用户在设置页没选过模型时（`aiModelId` 为空），优先用这些渠道的模型；
/// 都没命中才回退列表第一个。按主机名而非模型 id 匹配，用户改模型名 / 换模型也不会失效。
const PREFERRED_MODEL_HOSTS: &[&str] = &["chatapi.weixin.qq.com"];

// ---------------------------------------------------------------------------
// 数据结构
// ---------------------------------------------------------------------------

/// models.json 中的单个模型条目（宽松反序列化）。
///
/// `apiKey` 只在此结构内存在，绝不序列化透传给前端。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawModel {
    /// 模型标识（如 "glm-5.2"）。
    #[serde(default)]
    id: String,
    /// 展示名（缺省时回退 id）。
    #[serde(default)]
    name: Option<String>,
    /// OpenAI 兼容的 chat/completions 完整地址。
    #[serde(default)]
    url: String,
    /// API Key（`Authorization: Bearer <apiKey>`）。
    #[serde(default)]
    api_key: String,
    /// 供应商标识（展示用）。
    #[serde(default)]
    vendor: Option<String>,
}

/// 透出给前端的模型信息（**不含 apiKey**）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkBuddyModelInfo {
    /// 模型标识，前端用它指定要使用的模型。
    pub id: String,
    /// 展示名。
    pub name: String,
    /// 供应商（可选，仅展示）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    /// 是否为「用户未指定模型」时实际会使用的默认模型（前端用于下拉首项文案）。
    pub is_default: bool,
}

/// 台词话题。
///
/// - `chat`：随口搭话（俏皮短句，可结合时间 / 待办上下文）。
/// - `news`：今日播报（一条近期真实热点，明确禁止编造细节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    Chat,
    News,
}

impl Topic {
    /// 从（可能为 None 或未知的）字符串解析话题，未知一律按 `chat` 处理。
    fn parse(raw: Option<&str>) -> Self {
        match raw.map(str::trim) {
            Some("news") => Topic::News,
            _ => Topic::Chat,
        }
    }
}

/// 界面语言（决定台词用中文还是英文）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Zh,
    En,
}

impl Lang {
    /// 从 locale 字符串解析语言，未知一律按中文处理。
    fn parse(raw: Option<&str>) -> Self {
        match raw.map(str::trim) {
            Some(s) if s.to_ascii_lowercase().starts_with("en") => Lang::En,
            _ => Lang::Zh,
        }
    }
}

// ---------------------------------------------------------------------------
// 配置文件读取
// ---------------------------------------------------------------------------

/// 定位 WorkBuddy 模型配置文件（`~/.workbuddy/models.json`，回退 `~/.codebuddy`）。
///
/// 返回第一个存在的文件路径；都不存在返回 `None`。
fn resolve_models_file() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    MODELS_FILE_REL_CANDIDATES
        .iter()
        .map(|rel| home.join(rel))
        .find(|p| p.is_file())
}

/// 是否属于「自定义（Custom）」模型。
///
/// 本应用只复用用户自行配置的 Custom 接口。WorkBuddy 官方/原生渠道的模型会在
/// `vendor` 里标注自己的来源，这类条目一律跳过（它们通常还附带客户端侧的鉴权与
/// 用量约束，不适合由外部进程直连）。
///
/// `vendor` 缺失时按 Custom 处理：老版本配置可能没有该字段，而这类条目实际也是
/// 用户手填的地址与 Key。
fn is_custom_model(model: &RawModel) -> bool {
    match model.vendor.as_deref().map(str::trim) {
        Some(v) => v.eq_ignore_ascii_case("custom"),
        None => true,
    }
}

/// 读取并解析模型清单。
///
/// 依次过滤：
/// 1. 缺少 `id` / `url` / `apiKey` 的残缺条目；
/// 2. 非 Custom 渠道的条目（见 [`is_custom_model`]）。
///
/// 列表与默认回退共用本函数，保证「下拉里能选的」与「后端实际会调的」完全一致。
fn load_models() -> Result<Vec<RawModel>, String> {
    let path = resolve_models_file().ok_or_else(|| {
        "未找到 WorkBuddy 模型配置（~/.workbuddy/models.json），请先在 WorkBuddy 中添加模型".to_string()
    })?;

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("读取模型配置失败: {}", e))?;
    let models: Vec<RawModel> =
        serde_json::from_str(&content).map_err(|e| format!("解析模型配置失败: {}", e))?;

    Ok(models
        .into_iter()
        .filter(|m| !m.id.trim().is_empty() && !m.url.trim().is_empty() && !m.api_key.trim().is_empty())
        .filter(is_custom_model)
        .collect())
}

/// 列出可用的自定义模型（脱敏，供管理窗口下拉选择）。
///
/// 默认模型（{@link pick_default_model} 的结果）会带上 `isDefault = true`，
/// 前端据此在下拉首项展示「默认（xxx）」。
pub fn list_models() -> Result<Vec<WorkBuddyModelInfo>, String> {
    let models = load_models()?;
    let default_id = pick_default_model(&models).map(|m| m.id.clone());

    Ok(models
        .into_iter()
        .map(|m| WorkBuddyModelInfo {
            is_default: default_id.as_deref() == Some(m.id.as_str()),
            name: m.name.clone().unwrap_or_else(|| m.id.clone()),
            id: m.id,
            vendor: m.vendor,
        })
        .collect())
}

/// 从 URL 中取主机名（转小写）。无法解析时返回空串。
///
/// 手写而不引入 `url` crate：只服务于 [`PREFERRED_MODEL_HOSTS`] 的比对，
/// 输入恒为用户配置里的 http(s) 地址，够用且便于单测。
fn url_host(url: &str) -> String {
    // 去掉 scheme。
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    // 取 authority（到第一个 / ? # 为止）。
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    // 去掉 userinfo（user:pass@host）。
    let host_port = authority
        .rsplit_once('@')
        .map(|(_, h)| h)
        .unwrap_or(authority);
    // IPv6 字面量（[::1]:8080）不能按 ':' 切分。
    let host = match host_port.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or(""),
        None => host_port.split(':').next().unwrap_or(""),
    };
    host.to_ascii_lowercase()
}

/// 默认模型：优先命中 [`PREFERRED_MODEL_HOSTS`]，都没命中则回退列表第一个。
fn pick_default_model(models: &[RawModel]) -> Option<&RawModel> {
    PREFERRED_MODEL_HOSTS
        .iter()
        .find_map(|host| models.iter().find(|m| url_host(&m.url) == *host))
        .or_else(|| models.first())
}

/// 按 id 选模型；`model_id` 为 None 或不匹配时回退默认模型。
///
/// `models` 应来自 [`load_models`]（已过滤为 Custom）；传入 id 不在其中时回退默认项，
/// 保证历史设置里残留的非 Custom / 已删除模型 id 不会导致调用失败。
fn pick_model<'a>(models: &'a [RawModel], model_id: Option<&str>) -> Result<&'a RawModel, String> {
    if models.is_empty() {
        return Err("WorkBuddy 中没有可用的自定义模型".to_string());
    }
    if let Some(wanted) = model_id.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(found) = models.iter().find(|m| m.id == wanted) {
            return Ok(found);
        }
    }
    pick_default_model(models).ok_or_else(|| "WorkBuddy 中没有可用的自定义模型".to_string())
}

// ---------------------------------------------------------------------------
// 提示词与文本清洗（纯函数，可单测）
// ---------------------------------------------------------------------------

/// 构造 system 提示词（约束角色与输出格式）。
pub fn build_system_prompt(topic: Topic, lang: Lang) -> String {
    match (topic, lang) {
        (Topic::Chat, Lang::Zh) => "你是一只趴在程序员桌面上的小宠物，说话俏皮、口语化、偶尔自嘲，像个爱吐槽的搭子。\
             你只输出你要说的那一句话本身：不要解释、不要加引号、不要加「宠物：」之类的前缀、不要换行、不要用 markdown。"
            .to_string(),
        (Topic::News, Lang::Zh) => "你是一只趴在程序员桌面上的小宠物，负责给主人播报。\
             你只输出播报内容本身：不要解释、不要加引号、不要加序号或前缀、不要换行、不要用 markdown。"
            .to_string(),
        (Topic::Chat, Lang::En) => "You are a tiny pet living on a programmer's desktop. You speak playfully, casually and a little sarcastic. \
             Output ONLY the single sentence you want to say: no explanation, no quotes, no prefix like \"Pet:\", no line breaks, no markdown."
            .to_string(),
        (Topic::News, Lang::En) => "You are a tiny pet living on a programmer's desktop, giving your owner a short briefing. \
             Output ONLY the briefing itself: no explanation, no quotes, no numbering or prefix, no line breaks, no markdown."
            .to_string(),
    }
}

/// 构造 user 提示词。
///
/// `date_line` 为形如「2026-09-15（周二）下午 15:20」的时间描述；
/// `context` 为可选的现场上下文（今日待办 / token 用量等），由前端拼接后传入。
pub fn build_user_prompt(
    topic: Topic,
    lang: Lang,
    date_line: &str,
    context: Option<&str>,
) -> String {
    let ctx = context.map(str::trim).filter(|s| !s.is_empty());
    let ctx_line = match (lang, ctx) {
        (Lang::Zh, Some(c)) => format!("\n现场情况：{c}"),
        (Lang::En, Some(c)) => format!("\nCurrent context: {c}"),
        (_, None) => String::new(),
    };

    match (topic, lang) {
        (Topic::Chat, Lang::Zh) => format!(
            "现在是 {date_line}。{ctx_line}\n\
             请说一句 15~40 字的中文短句，像随口跟主人搭话一样。\
             不要说烂大街的问候语，可以吐槽工作、关心主人、聊点小情绪或有趣的碎碎念。"
        ),
        (Topic::News, Lang::Zh) => format!(
            "现在是 {date_line}。{ctx_line}\n\
             请说一条你确实知道的、近期真实发生的科技或互联网领域热点（一句话，20~60 字），\
             用宠物播报的口吻说出来。如果记不清具体时间和细节，就不要编造日期和数字，\
             宁可换一条你有把握的行业动态或有趣的事实。"
        ),
        (Topic::Chat, Lang::En) => format!(
            "It is now {date_line}.{ctx_line}\n\
             Say one sentence of 10~30 words, like casually chatting with your owner. \
             Avoid generic greetings; you may complain about work, show care, or share a random thought."
        ),
        (Topic::News, Lang::En) => format!(
            "It is now {date_line}.{ctx_line}\n\
             Give one recent real tech or internet headline you actually know (one sentence, 15~40 words), \
             spoken in the voice of a pet news anchor. If you are unsure about the exact date or details, do NOT invent them — \
             pick an industry development or fun fact you are confident about instead."
        ),
    }
}

/// 清洗模型输出：去掉 markdown 围栏、常见前缀标签、包裹引号与多余空白，并截断超长文本。
pub fn sanitize_line(raw: &str) -> String {
    let mut text = raw.replace("\r\n", "\n").replace('\r', "\n");

    // 去掉 ``` 围栏行。
    if text.contains("```") {
        text = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n");
    }

    // 取第一行非空内容（模型偶尔会先来一句「好的，」再换行说正题）。
    let mut line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string();

    line = strip_label(&line);
    line = line.trim().to_string();
    line = strip_wrapping_quotes(&line);

    // 折叠连续空白（含模型偶发插入的换行 / 制表符）。
    let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_chars(&collapsed, MAX_LINE_CHARS)
}

/// 去掉模型爱加的「宠物：」「回答：」「Assistant:」等前缀标签。
fn strip_label(line: &str) -> String {
    const LABELS: &[&str] = &[
        "宠物说：", "宠物：", "小宠物：", "桌宠：", "助手：", "回答：", "播报：",
        "Pet:", "Assistant:", "Answer:", "Briefing:", "Output:",
    ];
    // 允许最多两层标签（如「回答：宠物：」）。
    let mut out = line.trim().to_string();
    for _ in 0..2 {
        let matched = LABELS
            .iter()
            .find(|label| out.starts_with(**label))
            .map(|label| label.len());
        match matched {
            Some(len) => out = out[len..].trim_start().to_string(),
            None => break,
        }
    }
    out
}

/// 去掉整体包裹的成对引号（中英文引号 / 书名号）。
fn strip_wrapping_quotes(line: &str) -> String {
    const PAIRS: &[(char, char)] = &[
        ('"', '"'),
        ('\'', '\''),
        ('“', '”'),
        ('‘', '’'),
        ('「', '」'),
        ('『', '』'),
        ('《', '》'),
    ];
    let chars: Vec<char> = line.chars().collect();
    if chars.len() >= 2 {
        let (first, last) = (chars[0], chars[chars.len() - 1]);
        if PAIRS.iter().any(|(a, b)| *a == first && *b == last) {
            return chars[1..chars.len() - 1].iter().collect::<String>().trim().to_string();
        }
    }
    line.to_string()
}

/// 按码点截断到 `max` 个字符（超长补省略号）。
fn truncate_chars(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_string();
    }
    let mut out: String = chars[..max.saturating_sub(1)].iter().collect();
    out.push('…');
    out
}

/// 从 OpenAI 兼容响应体里取出正文。
///
/// 优先 `choices[0].message.content`；部分推理模型只回 `reasoning_content`（或 content 为空串），
/// 此时退而取 `reasoning_content`，避免整条请求白跑。
pub fn extract_content(body: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("解析响应失败: {}", e))?;

    let message = value
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("message"))
        .ok_or_else(|| "响应中没有 choices[0].message".to_string())?;

    for key in ["content", "reasoning_content"] {
        if let Some(text) = message.get(key).and_then(|v| v.as_str()) {
            if !text.trim().is_empty() {
                return Ok(text.to_string());
            }
        }
    }

    Err("模型没有返回任何文本".to_string())
}

// ---------------------------------------------------------------------------
// 生成
// ---------------------------------------------------------------------------

/// 当前时间的可读描述：`2026-09-15（周二）下午 15:20`。
fn now_description(lang: Lang) -> String {
    let now = chrono::Local::now();
    match lang {
        Lang::Zh => {
            let weekday = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"]
                [now.weekday().num_days_from_monday() as usize];
            let period = match now.hour() {
                0..=5 => "凌晨",
                6..=10 => "上午",
                11..=13 => "中午",
                14..=17 => "下午",
                _ => "晚上",
            };
            format!(
                "{}（{}）{} {:02}:{:02}",
                now.format("%Y-%m-%d"),
                weekday,
                period,
                now.hour(),
                now.minute()
            )
        }
        Lang::En => format!("{}", now.format("%Y-%m-%d %H:%M")),
    }
}

/// 调用 WorkBuddy 配置的模型生成一句宠物台词。
///
/// # 参数
/// - `model_id`：models.json 中的模型 id；为 None / 不存在时用第一个可用模型。
/// - `topic`：`"chat"`（随口搭话）或 `"news"`（今日播报），未知按 `chat`。
/// - `locale`：`"zh-CN"` / `"en-US"`，决定台词语言（以 `en` 开头即为英文）。
/// - `context`：可选的现场上下文（今日待办等），会拼进提示词。
///
/// # 返回
/// 清洗后的单行台词；任何失败返回错误消息字符串（前端据此回退固定语录）。
pub async fn generate_line(
    model_id: Option<&str>,
    topic: Option<&str>,
    locale: Option<&str>,
    context: Option<&str>,
) -> Result<String, String> {
    let topic = Topic::parse(topic);
    let lang = Lang::parse(locale);

    let models = load_models()?;
    let model = pick_model(&models, model_id)?;

    let system = build_system_prompt(topic, lang);
    let user = build_user_prompt(topic, lang, &now_description(lang), context);

    let payload = serde_json::json!({
        "model": model.id,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ],
        "temperature": 0.95,
        "max_tokens": 200,
        "stream": false
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let response = client
        .post(&model.url)
        .header("Authorization", format!("Bearer {}", model.api_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("请求模型失败: {}", e))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("读取模型响应失败: {}", e))?;

    if !status.is_success() {
        // 截断错误体，避免把超长 HTML 错误页塞进前端气泡。
        let snippet = truncate_chars(&body.split_whitespace().collect::<Vec<_>>().join(" "), 160);
        return Err(format!("模型返回 {}: {}", status.as_u16(), snippet));
    }

    let content = extract_content(&body)?;
    let line = sanitize_line(&content);
    if line.is_empty() {
        return Err("模型返回了空台词".to_string());
    }
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_topic_and_lang() {
        assert_eq!(Topic::parse(Some("news")), Topic::News);
        assert_eq!(Topic::parse(Some(" chat ")), Topic::Chat);
        assert_eq!(Topic::parse(None), Topic::Chat);
        assert_eq!(Topic::parse(Some("unknown")), Topic::Chat);

        assert_eq!(Lang::parse(Some("en-US")), Lang::En);
        assert_eq!(Lang::parse(Some("EN")), Lang::En);
        assert_eq!(Lang::parse(Some("zh-CN")), Lang::Zh);
        assert_eq!(Lang::parse(None), Lang::Zh);
    }

    #[test]
    fn user_prompt_includes_date_and_context() {
        let p = build_user_prompt(Topic::Chat, Lang::Zh, "2026-09-15 上午 09:30", Some("2 条待办"));
        assert!(p.contains("2026-09-15 上午 09:30"));
        assert!(p.contains("2 条待办"));

        // 无上下文时不应出现「现场情况」占位。
        let p2 = build_user_prompt(Topic::Chat, Lang::Zh, "2026-09-15", None);
        assert!(!p2.contains("现场情况"));

        // 空串 / 纯空白上下文同样忽略。
        let p3 = build_user_prompt(Topic::Chat, Lang::Zh, "2026-09-15", Some("   "));
        assert!(!p3.contains("现场情况"));
    }

    #[test]
    fn news_prompt_forbids_fabrication() {
        let zh = build_user_prompt(Topic::News, Lang::Zh, "2026-09-15", None);
        assert!(zh.contains("不要编造"));
        let en = build_user_prompt(Topic::News, Lang::En, "2026-09-15", None);
        assert!(en.contains("do NOT invent"));
    }

    #[test]
    fn sanitize_strips_label_prefix_and_quotes() {
        assert_eq!(sanitize_line("宠物：今天也要加油哦～"), "今天也要加油哦～");
        assert_eq!(sanitize_line("回答：宠物：嘿嘿"), "嘿嘿");
        assert_eq!(sanitize_line("「今天也要加油哦」"), "今天也要加油哦");
        assert_eq!(sanitize_line("\"Let's go!\""), "Let's go!");
        assert_eq!(sanitize_line("Pet: hello there"), "hello there");
    }

    #[test]
    fn sanitize_takes_first_non_empty_line_and_collapses_whitespace() {
        assert_eq!(sanitize_line("嗯…\n\n  那就写代码吧  "), "嗯…");
        assert_eq!(sanitize_line("哈哈   你   看"), "哈哈 你 看");
    }

    #[test]
    fn sanitize_strips_markdown_fence() {
        assert_eq!(sanitize_line("```text\n摸鱼时间到\n```"), "摸鱼时间到");
    }

    #[test]
    fn sanitize_truncates_long_text() {
        let long = "字".repeat(200);
        let out = sanitize_line(&long);
        assert_eq!(out.chars().count(), MAX_LINE_CHARS);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn sanitize_handles_empty_and_whitespace() {
        assert_eq!(sanitize_line(""), "");
        assert_eq!(sanitize_line("   \n  "), "");
    }

    #[test]
    fn extract_content_prefers_message_content() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"你好呀"}}]}"#;
        assert_eq!(extract_content(body).unwrap(), "你好呀");
    }

    #[test]
    fn extract_content_falls_back_to_reasoning_content() {
        // 推理模型偶发 content 为空：回退 reasoning_content 而非整条失败。
        let body = r#"{"choices":[{"message":{"content":"","reasoning_content":"fallback"}}]}"#;
        assert_eq!(extract_content(body).unwrap(), "fallback");
    }

    #[test]
    fn extract_content_errors_on_malformed_body() {
        assert!(extract_content("not json").is_err());
        assert!(extract_content(r#"{"choices":[]}"#).is_err());
        assert!(extract_content(r#"{"choices":[{"message":{"content":"  "}}]}"#).is_err());
    }

    /// 构造测试用模型条目（指定 url）。
    fn raw_with(id: &str, vendor: Option<&str>, url: &str) -> RawModel {
        RawModel {
            id: id.into(),
            name: None,
            url: url.into(),
            api_key: "k".into(),
            vendor: vendor.map(|v| v.to_string()),
        }
    }

    /// 构造测试用模型条目（url 用无关主机，只关心 id / vendor 的用例）。
    fn raw(id: &str, vendor: Option<&str>) -> RawModel {
        raw_with(id, vendor, &format!("https://x/{id}"))
    }

    #[test]
    fn pick_model_prefers_requested_then_falls_back() {
        let models = vec![raw("a", Some("Custom")), raw("b", Some("custom"))];
        assert_eq!(pick_model(&models, Some("b")).unwrap().id, "b");
        assert_eq!(pick_model(&models, Some("missing")).unwrap().id, "a");
        assert_eq!(pick_model(&models, None).unwrap().id, "a");
        assert!(pick_model(&[], None).is_err());
    }

    #[test]
    fn url_host_parses_common_forms() {
        assert_eq!(
            url_host("https://chatapi.weixin.qq.com/openai/v1/chat/completions"),
            "chatapi.weixin.qq.com"
        );
        // 带端口 / 查询串 / 哈希。
        assert_eq!(url_host("http://Example.COM:8080/v1?a=1#x"), "example.com");
        // 带 userinfo。
        assert_eq!(url_host("https://user:pw@api.host.cn/v1"), "api.host.cn");
        // 无 scheme。
        assert_eq!(url_host("api.host.cn/v1"), "api.host.cn");
        // IPv6 字面量（不能按 ':' 切）。
        assert_eq!(url_host("http://[::1]:8080/v1"), "::1");
        // 空串 / 垃圾输入不 panic。
        assert_eq!(url_host(""), "");
    }

    #[test]
    fn pick_default_model_prefers_configured_host() {
        // 微信渠道排在第二位（非首位）：默认应命中它而不是列表第一个。
        let models = vec![
            raw_with("first", Some("Custom"), "https://windhub.cc/v1/chat/completions"),
            raw_with(
                "weixin-model",
                Some("Custom"),
                "https://chatapi.weixin.qq.com/openai/v1/chat/completions",
            ),
        ];
        assert_eq!(pick_default_model(&models).unwrap().id, "weixin-model");
    }

    #[test]
    fn pick_default_model_falls_back_to_first_without_preferred_host() {
        let models = vec![
            raw_with("first", Some("Custom"), "https://windhub.cc/v1/chat/completions"),
            raw_with("second", Some("Custom"), "https://api.123nhh.com/chat/completions"),
        ];
        assert_eq!(pick_default_model(&models).unwrap().id, "first");
        assert!(pick_default_model(&[]).is_none());
    }

    #[test]
    fn pick_model_without_id_uses_preferred_default() {
        let models = vec![
            raw_with("first", Some("Custom"), "https://windhub.cc/v1/chat/completions"),
            raw_with(
                "weixin-model",
                Some("Custom"),
                "https://chatapi.weixin.qq.com/openai/v1/chat/completions",
            ),
        ];
        // 未指定 / 指定了不存在的 id → 都回退到首选渠道模型。
        assert_eq!(pick_model(&models, None).unwrap().id, "weixin-model");
        assert_eq!(pick_model(&models, Some("gone")).unwrap().id, "weixin-model");
        // 显式指定的 id 仍然优先。
        assert_eq!(pick_model(&models, Some("first")).unwrap().id, "first");
    }

    #[test]
    fn is_custom_model_accepts_custom_case_insensitively() {
        assert!(is_custom_model(&raw("a", Some("Custom"))));
        assert!(is_custom_model(&raw("b", Some("custom"))));
        assert!(is_custom_model(&raw("c", Some(" CUSTOM "))));
    }

    #[test]
    fn is_custom_model_rejects_official_channels() {
        // 官方 / 原生渠道：带自己的 vendor 标识，一律不调用。
        assert!(!is_custom_model(&raw("a", Some("WorkBuddy"))));
        assert!(!is_custom_model(&raw("b", Some("CodeBuddy"))));
        assert!(!is_custom_model(&raw("c", Some("Tencent"))));
        assert!(!is_custom_model(&raw("d", Some("OpenAI"))));
    }

    #[test]
    fn is_custom_model_treats_missing_vendor_as_custom() {
        // 老版本配置可能没有 vendor 字段，这类条目同样是用户手填的地址与 Key。
        assert!(is_custom_model(&raw("a", None)));
    }


    #[test]
    fn truncate_chars_is_lossless_when_short() {
        assert_eq!(truncate_chars("abc", 5), "abc");
        assert_eq!(truncate_chars("abcde", 5), "abcde");
        assert_eq!(truncate_chars("abcdef", 5), "abcd…");
    }
}
