//! 注入 / 清理 `~/.workbuddy/settings.json` 的 hooks 配置。
//!
//! 联动开启时，把本应用安装的纯转发脚本（`workbuddy-pet-hook.mjs`）注册到 WorkBuddy
//! 的 7 个 hook 事件上；关闭时仅移除我们注入的条目，保留用户其他 hook。
//!
//! 去重标识：脚本路径（`command` 字符串中含该路径），保证反复开关不会累加条目。
//! 核心逻辑抽成 [`set_workbuddy_linked_at`] / [`is_workbuddy_linked_at`]，便于用临时文件单测。
//!
//! # WorkBuddy（CodeBuddy Code 引擎）hooks 格式
//!
//! ```json
//! {
//!   "hooks": {
//!     "PreToolUse": [
//!       {
//!         "matcher": "*",
//!         "hooks": [
//!           { "type": "command", "command": "node \"/abs/path/hook.mjs\"", "timeout": 4 }
//!         ]
//!       }
//!     ]
//!   }
//! }
//! ```
//!
//! 与 ZCode 的关键差异：
//! - 顶层 `hooks` **直接**是「事件名 → 数组」，没有 `enabled` / `events` 包裹层；
//! - 条目 `type` 为 `"command"`（而非 `"process"`），参数**拼进 `command` 字符串**（无 `args` 数组）；
//! - 超时字段为 `timeout`，单位**秒**（而非 `timeoutMs` 毫秒）；
//! - Windows 上 hook 由 **Git Bash** 执行，故命令中的路径统一转成正斜杠并用双引号包裹。

use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// hook 脚本文件名（写入 app_data 目录，由 WorkBuddy 以 `node` 调用）。
const HOOK_SCRIPT_NAME: &str = "workbuddy-pet-hook.mjs";

/// WorkBuddy 用户级配置相对 home 的路径。
///
/// Windows 对应 `%USERPROFILE%\.workbuddy\settings.json`，
/// macOS / Linux 对应 `$HOME/.workbuddy/settings.json`。
const WORKBUDDY_CONFIG_REL: &str = ".workbuddy/settings.json";

/// WorkBuddy（CodeBuddy Code 引擎）支持的 hook 事件名。
///
/// 引擎完整支持 27+ 种事件，这里只注册驱动宠物反馈所需的 7 个。
const WORKBUDDY_HOOK_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PostToolUseFailure",
    "Stop",
];

/// hook 条目超时（秒）。WorkBuddy 一次性子进程不应长时间阻塞（默认 60s）。
const HOOK_TIMEOUT_SECS: u64 = 4;

/// 调用 hook 的命令（系统 node）。
const HOOK_COMMAND: &str = "node";

/// hook 条目的 matcher（匹配所有工具/输入）。
const HOOK_MATCHER: &str = "*";

/// 内联的 hook 转发脚本（纯 Node 内置模块，无 npm 依赖，兼容 Node 18+）。
///
/// 采用内联常量直接 `fs::write`，不依赖 `tauri.conf.json` 的 resources 注册，
/// 安装/刷新更稳健。
const HOOK_SCRIPT: &str = r##"#!/usr/bin/env node
// WorkBuddy Pet Hook 转发脚本（纯 Node 内置模块，无 npm 依赖，兼容 Node 18+）。
//
// 作用：WorkBuddy 的 hook 是一次性子进程。WorkBuddy 把一行 JSON 写到本进程 stdin。
// 本脚本不做任何解析，只把 stdin 原文 POST 到桌面宠物应用启动的本地 HTTP 服务，
// 由 Rust 后端（workbuddy::hook_server）转发给 pet 窗口做动画反馈。
//
// 设计原则：绝不阻断 WorkBuddy。任何错误（服务未启动、端口文件缺失、网络超时）
// 都静默 exit(0)，不影响 WorkBuddy 正常工作。

import http from 'node:http';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

// 应用 app_data 目录名（由 tauri.conf.json 的 identifier 决定）。
const APP_DIR_NAME = 'io.github.xiaoheqvq.workbuddy-pet';
const PORT_FILE = 'workbuddy-pet.port';
const TIMEOUT_MS = 3000;

// 拼接 home 目录下路径（~ 无法被 fs 直接识别）。
function home(...segments) {
  const h = os.homedir();
  return h ? path.join(h, ...segments) : null;
}

// 各平台 app_data 候选目录（按优先级，逐一尝试读取端口文件）。
function candidateDataDirs() {
  const dirs = [];
  const platform = process.platform;
  if (platform === 'darwin') {
    const p = home('Library', 'Application Support', APP_DIR_NAME);
    if (p) dirs.push(p);
  } else if (platform === 'win32') {
    const appdata = process.env.APPDATA;
    if (appdata) dirs.push(path.join(appdata, APP_DIR_NAME));
    const p = home('AppData', 'Roaming', APP_DIR_NAME);
    if (p) dirs.push(p);
  } else {
    // Linux / 其他 *nix：优先 XDG_DATA_HOME，回退 ~/.local/share
    const xdg = process.env.XDG_DATA_HOME;
    if (xdg) dirs.push(path.join(xdg, APP_DIR_NAME));
    const p = home('.local', 'share', APP_DIR_NAME);
    if (p) dirs.push(p);
  }
  return dirs;
}

// 读取端口文件，返回端口号字符串；找不到返回 null。
function readPort() {
  for (const dir of candidateDataDirs()) {
    const file = path.join(dir, PORT_FILE);
    try {
      const port = fs.readFileSync(file, 'utf8').trim();
      if (port) return port;
    } catch {
      // 文件不存在或不可读，继续尝试下一个候选目录。
    }
  }
  return null;
}

// 读取 stdin 全部内容为 Buffer（二进制安全）。
function readStdin() {
  return new Promise((resolve) => {
    const chunks = [];
    process.stdin.on('data', (chunk) => { chunks.push(chunk); });
    process.stdin.on('end', () => resolve(Buffer.concat(chunks)));
    process.stdin.on('error', () => resolve(Buffer.concat(chunks)));
  });
}

async function main() {
  const port = readPort();
  if (!port) {
    // 桌面宠物服务未启动，静默退出。
    process.exit(0);
  }

  const body = await readStdin();

  const options = {
    hostname: '127.0.0.1',
    port: Number(port),
    path: '/hook',
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Content-Length': body.length,
    },
  };

  const req = http.request(options, () => {
    // 收到响应即结束，不关心响应体。
    process.exit(0);
  });

  req.on('error', () => {
    process.exit(0);
  });

  // 超时保护：避免 WorkBuddy 因 hook 长时间挂起。
  const timer = setTimeout(() => {
    req.destroy();
    process.exit(0);
  }, TIMEOUT_MS);
  req.on('close', () => clearTimeout(timer));

  req.end(body);
}

main().catch(() => process.exit(0));
"##;

/// 联动操作结果（返回给前端，统一 camelCase）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkResult {
    /// 操作是否成功。
    pub ok: bool,
    /// 当前是否已联动（`enabled = true` 时为 true）。
    pub linked: bool,
    /// hook 脚本落地路径。
    pub script_path: String,
    /// WorkBuddy 配置文件路径。
    pub config_path: String,
}

/// hook 脚本落地路径（`<app_data>/workbuddy-pet-hook.mjs`）。
///
/// # 参数
/// - `app`: Tauri 应用句柄
///
/// # 返回
/// 成功返回脚本路径，失败返回错误消息字符串。
pub fn hook_script_path(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取 app_data_dir 失败: {}", e))?;
    Ok(data_dir.join(HOOK_SCRIPT_NAME))
}

/// 幂等安装（刷新）hook 转发脚本到 `<app_data>/workbuddy-pet-hook.mjs`。
///
/// 采用内联常量直接写入，不依赖打包资源注册。已存在则覆盖为最新版本。
///
/// # 参数
/// - `app`: Tauri 应用句柄
///
/// # 返回
/// 成功返回 `Ok(())`，失败返回错误消息字符串。
pub fn ensure_hook_script_installed(app: &AppHandle) -> Result<(), String> {
    let script_path = hook_script_path(app)?;
    if let Some(parent) = script_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 hook 脚本目录失败: {}", e))?;
    }
    fs::write(&script_path, HOOK_SCRIPT).map_err(|e| format!("写入 hook 脚本失败: {}", e))?;
    tracing::info!("[WorkBuddyLink] hook 脚本已写入 {}", script_path.display());
    Ok(())
}

/// WorkBuddy 用户级配置文件路径（`~/.workbuddy/settings.json`）。
///
/// # 返回
/// 成功返回配置路径；无法定位 home 目录时返回错误消息字符串。
pub fn workbuddy_config_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法定位用户 home 目录".to_string())?;
    Ok(home.join(WORKBUDDY_CONFIG_REL))
}

/// 把脚本路径归一化为 shell / JSON 友好的 POSIX 形式。
///
/// Windows 上 WorkBuddy 通过 **Git Bash** 执行 hook，反斜杠路径会被当作转义符，
/// 故统一转成正斜杠。
fn to_posix(script_path: &str) -> String {
    script_path.replace('\\', "/")
}

/// 构造 hook 的 `command` 字符串：`node "<绝对路径>"`。
///
/// WorkBuddy 的 `command` 是 shell 命令，参数直接拼在字符串里（没有 `args` 数组），
/// 因此路径必须用双引号包裹以容忍空格。
fn build_hook_command(script_path: &str) -> String {
    let posix = to_posix(script_path);
    let escaped = posix.replace('"', "\\\"");
    format!("{HOOK_COMMAND} \"{escaped}\"")
}

/// 构造一条 matcher 容器（含 hooks 数组），符合 WorkBuddy 官方嵌套格式。
///
/// 官方结构：每个事件数组元素是 `{ matcher, hooks: [{ type, command, timeout }] }`。
fn build_hook_entry(script_path: &str) -> Value {
    json!({
        "matcher": HOOK_MATCHER,
        "hooks": [
            {
                "type": "command",
                "command": build_hook_command(script_path),
                "timeout": HOOK_TIMEOUT_SECS
            }
        ]
    })
}

/// 判断某条 matcher 容器是否由本应用注入。
///
/// WorkBuddy 条目没有 `args` 数组，故以 `command` 字符串是否包含脚本路径为标识；
/// 归一化反斜杠后比较，跨平台稳定。同时兼容旧扁平格式（entry 直接带 `command`）。
fn is_our_entry(entry: &Value, script_path: &str) -> bool {
    let needle = to_posix(script_path);
    // 嵌套格式：entry.hooks[].command
    if let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
        if hooks.iter().any(|h| {
            h.get("command")
                .and_then(|c| c.as_str())
                .map(|c| to_posix(c).contains(needle.as_str()))
                .unwrap_or(false)
        }) {
            return true;
        }
    }
    // 旧扁平格式兼容（清理历史残留）：entry.command
    entry
        .get("command")
        .and_then(|c| c.as_str())
        .map(|c| to_posix(c).contains(needle.as_str()))
        .unwrap_or(false)
}

/// 开关联动的核心逻辑（作用于任意指定配置文件，便于测试）。
///
/// - `enabled = true`：对 7 个事件移除旧的同路径条目后追加新条目。
/// - `enabled = false`：对 7 个事件移除所有同路径条目，保留用户其他 hook。
///
/// WorkBuddy 的 `hooks` 直接是「事件名 → 数组」，不额外包裹 `enabled` / `events`，
/// 以免污染引擎无法识别的字段。
///
/// # 参数
/// - `config_path`: WorkBuddy 配置文件路径
/// - `script_path`: hook 脚本路径（去重标识）
/// - `enabled`: 是否开启联动
///
/// # 返回
/// 成功返回 [`LinkResult`]，失败返回错误消息字符串。
pub fn set_workbuddy_linked_at(
    config_path: &Path,
    script_path: &str,
    enabled: bool,
) -> Result<LinkResult, String> {
    // 读现有配置：文件不存在视为空对象；存在但读取/解析失败则中止，
    // 避免把用户既有配置（可能含其他 hooks / 不可识别字段）覆盖丢失。
    let raw = match fs::read_to_string(config_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("读取 WorkBuddy 配置失败: {}", e)),
    };
    let mut config: Value = if raw.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&raw).map_err(|e| {
            format!(
                "解析 WorkBuddy 配置失败（为避免破坏已有配置已中止本次写入）: {}",
                e
            )
        })?
    };

    // 确保 config.hooks 存在且为对象。
    if !config.get("hooks").map(|v| v.is_object()).unwrap_or(false) {
        config["hooks"] = json!({});
    }
    let hooks = config
        .get_mut("hooks")
        .expect("hooks 已在上一步确保存在");

    if enabled {
        let entry = build_hook_entry(script_path);
        for ev in WORKBUDDY_HOOK_EVENTS {
            // 该事件数组不存在或非数组则新建空数组。
            if !hooks.get(*ev).map(|v| v.is_array()).unwrap_or(false) {
                hooks[*ev] = json!([]);
            }
            if let Some(arr) = hooks.get_mut(*ev).and_then(|v| v.as_array_mut()) {
                // 去重：移除旧的同路径条目，再追加新条目。
                arr.retain(|e| !is_our_entry(e, script_path));
                arr.push(entry.clone());
            }
        }
    } else {
        // 关闭：仅移除我们注入的条目，保留用户其他 hook。
        for ev in WORKBUDDY_HOOK_EVENTS {
            if let Some(arr) = hooks.get_mut(*ev).and_then(|v| v.as_array_mut()) {
                arr.retain(|e| !is_our_entry(e, script_path));
            }
        }
    }

    // 写回（先确保父目录存在）。
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
    }
    let pretty =
        serde_json::to_string_pretty(&config).map_err(|e| format!("序列化配置失败: {}", e))?;
    fs::write(config_path, pretty).map_err(|e| format!("写入配置失败: {}", e))?;

    Ok(LinkResult {
        ok: true,
        linked: enabled,
        script_path: script_path.to_string(),
        config_path: config_path.to_string_lossy().to_string(),
    })
}

/// 是否已联动的核心逻辑（作用于任意指定配置文件，便于测试）。
///
/// 任一 7 事件中存在 `command` 含脚本路径的条目即视为已联动。
fn is_workbuddy_linked_at(config_path: &Path, script_path: &str) -> bool {
    let Ok(raw) = fs::read_to_string(config_path) else {
        return false;
    };
    let Ok(config): Result<Value, _> = serde_json::from_str(&raw) else {
        return false;
    };
    let Some(hooks) = config.get("hooks") else {
        return false;
    };
    for ev in WORKBUDDY_HOOK_EVENTS {
        if let Some(arr) = hooks.get(*ev).and_then(|v| v.as_array()) {
            if arr.iter().any(|e| is_our_entry(e, script_path)) {
                return true;
            }
        }
    }
    false
}

/// 开关 WorkBuddy 联动（委托给 [`set_workbuddy_linked_at`]，自动定位脚本与配置路径）。
///
/// # 参数
/// - `app`: Tauri 应用句柄
/// - `enabled`: 是否开启联动
///
/// # 返回
/// 成功返回 [`LinkResult`]，失败返回错误消息字符串。
pub fn set_workbuddy_linked(app: &AppHandle, enabled: bool) -> Result<LinkResult, String> {
    ensure_hook_script_installed(app)?;
    let script_path = hook_script_path(app)?;
    let config_path = workbuddy_config_path()?;
    let script_str = script_path.to_string_lossy().to_string();
    set_workbuddy_linked_at(&config_path, &script_str, enabled)
}

/// 是否已联动（委托给 [`is_workbuddy_linked_at`]）。
///
/// # 参数
/// - `app`: Tauri 应用句柄
///
/// # 返回
/// 已注入任一 hook 条目返回 true；配置缺失/无条目返回 false。
pub fn is_workbuddy_linked(app: &AppHandle) -> bool {
    let Ok(config_path) = workbuddy_config_path() else {
        return false;
    };
    let Ok(script_path) = hook_script_path(app) else {
        return false;
    };
    let script_str = script_path.to_string_lossy().to_string();
    is_workbuddy_linked_at(&config_path, &script_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 进程内自增计数器，保证并行/串行测试的临时文件名唯一。
    fn unique_counter() -> usize {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static C: AtomicUsize = AtomicUsize::new(0);
        C.fetch_add(1, Ordering::SeqCst)
    }

    /// 生成唯一的临时配置文件路径（清理可能残留的同名文件）。
    fn unique_temp_config() -> PathBuf {
        let mut p = std::env::temp_dir();
        let name = format!(
            "workbuddy-pet-link-test-{}-{}.json",
            std::process::id(),
            unique_counter()
        );
        p.push(name);
        let _ = std::fs::remove_file(&p);
        p
    }

    /// 统计配置中所有事件里属于本应用注入的条目总数。
    fn count_our_entries(config_path: &Path, script_path: &str) -> usize {
        let raw = std::fs::read_to_string(config_path).unwrap_or_default();
        let config: Value = serde_json::from_str(&raw).unwrap_or(json!({}));
        let Some(hooks) = config.get("hooks") else {
            return 0;
        };
        let mut count = 0;
        for ev in WORKBUDDY_HOOK_EVENTS {
            if let Some(arr) = hooks.get(*ev).and_then(|v| v.as_array()) {
                count += arr.iter().filter(|e| is_our_entry(e, script_path)).count();
            }
        }
        count
    }

    #[test]
    fn test_workbuddy_config_path_under_home() {
        // 仅校验路径拼接结果（只读，无副作用）。
        let p = workbuddy_config_path().unwrap();
        let s = p.to_string_lossy().replace('\\', "/");
        assert!(
            s.ends_with(".workbuddy/settings.json"),
            "配置路径应以 .workbuddy/settings.json 结尾，实际: {s}"
        );
    }

    #[test]
    fn test_build_hook_command_posix_quoted() {
        // Windows 反斜杠路径应被转成正斜杠并加引号（Git Bash 友好）。
        let cmd = build_hook_command(r"C:\Users\me\AppData\Roaming\app\hook.mjs");
        assert_eq!(
            cmd,
            r#"node "C:/Users/me/AppData/Roaming/app/hook.mjs""#
        );
    }

    #[test]
    fn test_link_idempotent_enable_disable_enable() {
        let config_path = unique_temp_config();
        let script_path = "/tmp/fake-workbuddy-pet-hook.mjs";

        // 初始：未联动，配置文件不存在也算未联动。
        assert!(!is_workbuddy_linked_at(&config_path, script_path));

        // 第一次 enabled：7 个事件各 1 条 = 7。
        let r = set_workbuddy_linked_at(&config_path, script_path, true).unwrap();
        assert!(r.linked);
        assert_eq!(count_our_entries(&config_path, script_path), 7);
        assert!(is_workbuddy_linked_at(&config_path, script_path));

        // 第二次 enabled：去重，仍为 7（不累加）。
        set_workbuddy_linked_at(&config_path, script_path, true).unwrap();
        assert_eq!(count_our_entries(&config_path, script_path), 7);

        // 第三次 enabled：仍为 7。
        set_workbuddy_linked_at(&config_path, script_path, true).unwrap();
        assert_eq!(count_our_entries(&config_path, script_path), 7);

        // disabled：移除全部，0 条。
        let r = set_workbuddy_linked_at(&config_path, script_path, false).unwrap();
        assert!(!r.linked);
        assert_eq!(count_our_entries(&config_path, script_path), 0);
        assert!(!is_workbuddy_linked_at(&config_path, script_path));

        // 再次 enabled：恢复 7。
        set_workbuddy_linked_at(&config_path, script_path, true).unwrap();
        assert_eq!(count_our_entries(&config_path, script_path), 7);

        let _ = std::fs::remove_file(&config_path);
    }

    #[test]
    fn test_link_preserves_other_entries_and_unknown_keys() {
        // 预置含其他 hook 条目 + 引擎其他字段的配置，验证本应用操作不破坏它们。
        let config_path = unique_temp_config();
        let script_path = "/tmp/fake-workbuddy-pet-hook.mjs";
        let initial = json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            { "type": "command", "command": "python3 other.py", "timeout": 30 }
                        ]
                    }
                ]
            },
            "model": "wb-default"
        });
        std::fs::write(&config_path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();

        // 联动：保留 other.py 并追加我们的条目 → PreToolUse 应有 2 条。
        set_workbuddy_linked_at(&config_path, script_path, true).unwrap();
        let raw = std::fs::read_to_string(&config_path).unwrap();
        let config: Value = serde_json::from_str(&raw).unwrap();
        let pre = config
            .get("hooks")
            .and_then(|h| h.get("PreToolUse"))
            .and_then(|v| v.as_array())
            .unwrap();
        assert_eq!(pre.len(), 2);
        assert!(pre.iter().any(|e| is_our_entry(e, script_path)));
        assert!(pre.iter().any(|e| {
            e.get("hooks")
                .and_then(|h| h.as_array())
                .and_then(|a| a.first())
                .and_then(|h| h.get("command"))
                .and_then(|v| v.as_str())
                == Some("python3 other.py")
        }));
        // 引擎其他字段保持不动。
        assert_eq!(
            config.get("model").and_then(|v| v.as_str()),
            Some("wb-default")
        );

        // disabled：仅移除我们的，保留 other.py → PreToolUse 应剩 1 条。
        set_workbuddy_linked_at(&config_path, script_path, false).unwrap();
        let raw = std::fs::read_to_string(&config_path).unwrap();
        let config: Value = serde_json::from_str(&raw).unwrap();
        let pre = config
            .get("hooks")
            .and_then(|h| h.get("PreToolUse"))
            .and_then(|v| v.as_array())
            .unwrap();
        assert_eq!(pre.len(), 1);

        let _ = std::fs::remove_file(&config_path);
    }

    #[test]
    fn test_link_all_seven_events_present() {
        // 校验 enabled 后恰好 7 个事件键都存在且各含 1 条。
        let config_path = unique_temp_config();
        let script_path = "/tmp/fake-workbuddy-pet-hook.mjs";
        set_workbuddy_linked_at(&config_path, script_path, true).unwrap();
        let raw = std::fs::read_to_string(&config_path).unwrap();
        let config: Value = serde_json::from_str(&raw).unwrap();
        let hooks = config.get("hooks").expect("hooks 应存在");
        for ev in WORKBUDDY_HOOK_EVENTS {
            let arr = hooks
                .get(*ev)
                .and_then(|v| v.as_array())
                .unwrap_or_else(|| panic!("事件 {ev} 应存在数组"));
            assert_eq!(
                arr.iter().filter(|e| is_our_entry(e, script_path)).count(),
                1,
                "事件 {ev} 应恰好含 1 条本应用条目"
            );
        }
        let _ = std::fs::remove_file(&config_path);
    }

    #[test]
    fn test_link_does_not_add_enabled_or_events_wrapper() {
        // WorkBuddy 格式不应出现 ZCode 特有的 enabled / events 包裹层。
        let config_path = unique_temp_config();
        let script_path = "/tmp/fake-workbuddy-pet-hook.mjs";
        set_workbuddy_linked_at(&config_path, script_path, true).unwrap();
        let raw = std::fs::read_to_string(&config_path).unwrap();
        let config: Value = serde_json::from_str(&raw).unwrap();
        let hooks = config.get("hooks").unwrap();
        assert!(hooks.get("enabled").is_none(), "不应写入 hooks.enabled");
        assert!(hooks.get("events").is_none(), "不应写入 hooks.events");
        assert!(hooks.get("PreToolUse").is_some(), "事件应直接挂在 hooks 下");
        let _ = std::fs::remove_file(&config_path);
    }

    #[test]
    fn test_link_aborts_on_invalid_json() {
        // 已存在但内容非法时，应中止写入而非覆盖（保护用户配置）。
        let config_path = unique_temp_config();
        std::fs::write(&config_path, "{ not valid json").unwrap();
        let r = set_workbuddy_linked_at(&config_path, "/tmp/hook.mjs", true);
        assert!(r.is_err(), "非法 JSON 应返回错误");
        // 原内容保持不变。
        assert_eq!(
            std::fs::read_to_string(&config_path).unwrap(),
            "{ not valid json"
        );
        let _ = std::fs::remove_file(&config_path);
    }

    #[test]
    fn test_is_our_entry_identity() {
        let ours = json!({
            "matcher": "*",
            "hooks": [{ "type": "command", "command": r#"node "/x/hook.mjs""#, "timeout": 4 }]
        });
        let others = json!({
            "matcher": "*",
            "hooks": [{ "type": "command", "command": r#"node "/y/other.mjs""#, "timeout": 4 }]
        });
        let noargs = json!({ "type": "prompt", "prompt": "evaluate" });
        assert!(is_our_entry(&ours, "/x/hook.mjs"));
        assert!(!is_our_entry(&ours, "/other/hook.mjs"));
        assert!(!is_our_entry(&others, "/x/hook.mjs"));
        assert!(!is_our_entry(&noargs, "/x/hook.mjs"));

        // 跨平台归一化：写入时 command 用正斜杠（build_hook_command 转过），
        // 读取时 script_path 是 Windows 反斜杠形式 → 仍应识别为同一条目。
        let ours_win = json!({
            "matcher": "*",
            "hooks": [
                { "type": "command", "command": r#"node "C:/Users/me/app/hook.mjs""#, "timeout": 4 }
            ]
        });
        assert!(is_our_entry(&ours_win, r"C:\Users\me\app\hook.mjs"));
        assert!(!is_our_entry(&ours_win, r"C:\Users\me\other\hook.mjs"));
    }
}
