// 进程管理：枚举进程（含内存 / CPU / 占用端口）与结束进程。
//
// 设计要点（遵循 tauri-harness 后端规范：commands 层是薄入口，重活下沉到私有辅助）：
// - 进程枚举走 sysinfo（跨平台，一次拿到 PID / 名称 / 内存 / CPU）；`System` 实例常驻
//   managed state —— CPU 占用是「两次采样的时间差」，必须复用同一实例才能读到非零值。
// - 端口映射走各平台原生命令（Windows `netstat -ano` / macOS·Linux `lsof -nP -i`，
//   Linux 再兜底 `netstat -tunap`）；读取失败只降级为空表，不阻断进程列表本身，
//   由前端按 `portsAvailable` 提示「端口信息不可用」。
// - 结束进程走 `taskkill`（Windows，带 /T 连带子进程树）/ `kill`（Unix）。
//   本应用是 GUI 子系统，Windows 下所有外部命令都必须加 CREATE_NO_WINDOW，
//   否则每刷新一次都会闪出黑色控制台窗口。

use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use sysinfo::{Pid, ProcessesToUpdate, System, MINIMUM_CPU_UPDATE_INTERVAL};

/// 单个进程信息（序列化给前端，camelCase）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    /// 进程 ID。
    pub pid: u32,
    /// 进程名（如 chrome.exe）；内核线程等无名进程退化为 `PID <n>`。
    pub name: String,
    /// 可执行文件完整路径（可能为空，如内核线程）。
    pub exe: Option<String>,
    /// 完整命令行（以空格拼接，可能为空）。
    pub cmd: String,
    /// 物理内存占用（字节）。
    pub memory: u64,
    /// CPU 占用（百分比；多核满载可超过 100）。
    pub cpu: f32,
    /// 该进程占用的端口（升序去重；仅 TCP 监听 / UDP 绑定）。
    pub ports: Vec<u16>,
    /// 是否为当前应用自身进程（前端禁止直接结束）。
    pub is_self: bool,
}

/// 一次进程枚举的完整结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSnapshot {
    /// 进程列表（未排序，排序交由前端，便于切换排序不重复 IPC）。
    pub processes: Vec<ProcessInfo>,
    /// 系统进程总数。
    pub total: usize,
    /// 端口信息是否读取成功。
    pub ports_available: bool,
    /// 本次枚举涉及的端口总数（去重后）。
    pub port_count: usize,
    /// 平台标识（windows / macos / linux / unknown）。
    pub platform: String,
    /// 本次枚举耗时（毫秒）。
    pub elapsed_ms: u64,
}

/// 结束进程的结果（批量结束返回数组）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KillOutcome {
    pub pid: u32,
    pub name: String,
    pub ok: bool,
    /// 失败原因（成功时为 None）。
    pub error: Option<String>,
}

/// 进程枚举的常驻状态。
///
/// `System` 必须跨调用复用：sysinfo 的进程 CPU 占用由两次采样的 CPU 时间差算出，
/// 每次新建实例的话首屏永远是 0。`warmed` 标记是否已做过「预热采样」——
/// 首次调用会额外等待一个采样间隔再采一次，保证用户第一次打开面板就有 CPU 数据。
pub struct ProcessState {
    system: Arc<Mutex<System>>,
    warmed: AtomicBool,
}

impl Default for ProcessState {
    fn default() -> Self {
        Self {
            system: Arc::new(Mutex::new(System::new())),
            warmed: AtomicBool::new(false),
        }
    }
}

// ============================ 命令 ============================

/// 枚举当前系统进程。
///
/// - `include_ports`：是否读取端口占用。端口枚举是外部命令调用（Windows `netstat`
///   约 100~300ms，macOS `lsof` 更慢），关闭后刷新显著更快，但端口列与「端口数排序」
///   会失去数据。
#[tauri::command]
pub async fn list_processes(
    state: tauri::State<'_, ProcessState>,
    include_ports: Option<bool>,
) -> Result<ProcessSnapshot, String> {
    let system = state.system.clone();
    // 首次调用需要预热采样（详见 ProcessState 注释）。
    let needs_warmup = !state.warmed.swap(true, Ordering::SeqCst);
    let include_ports = include_ports.unwrap_or(true);

    tauri::async_runtime::spawn_blocking(move || {
        collect_snapshot(&system, needs_warmup, include_ports)
    })
    .await
    .map_err(|e| format!("进程枚举任务执行失败: {}", e))?
}

/// 结束单个进程。`force = true` 为强杀（不做优雅退出请求）。
#[tauri::command]
pub async fn kill_process(
    state: tauri::State<'_, ProcessState>,
    pid: u32,
    force: Option<bool>,
) -> Result<KillOutcome, String> {
    let name = lookup_name(&state, pid);
    let force = force.unwrap_or(false);

    tauri::async_runtime::spawn_blocking(move || outcome(pid, name, terminate(pid, force)))
        .await
        .map_err(|e| format!("结束进程任务执行失败: {}", e))
}

/// 批量结束进程（端口搜索后一次清理多个占用者时使用）。
#[tauri::command]
pub async fn kill_processes(
    state: tauri::State<'_, ProcessState>,
    pids: Vec<u32>,
    force: Option<bool>,
) -> Result<Vec<KillOutcome>, String> {
    let force = force.unwrap_or(false);
    let targets = with_names(&state, pids);

    tauri::async_runtime::spawn_blocking(move || {
        targets
            .into_iter()
            .map(|(pid, name)| outcome(pid, name, terminate(pid, force)))
            .collect::<Vec<KillOutcome>>()
    })
    .await
    .map_err(|e| format!("结束进程任务执行失败: {}", e))
}

// ============================ 私有辅助 ============================

/// 采集一次进程快照。
///
/// 端口枚举刻意放在加锁之前：`netstat` / `lsof` 是外部进程调用（较慢），与 sysinfo 的
/// 采样互不依赖，还能顺带把两次 CPU 采样间隔撑开。
fn collect_snapshot(
    system: &Arc<Mutex<System>>,
    needs_warmup: bool,
    include_ports: bool,
) -> Result<ProcessSnapshot, String> {
    let started = Instant::now();

    let (port_map, ports_available) = if include_ports {
        match collect_ports() {
            Some(map) => (map, true),
            None => (HashMap::new(), false),
        }
    } else {
        (HashMap::new(), false)
    };

    let mut sys = system
        .lock()
        .map_err(|_| "进程状态锁已损坏（内部错误）".to_string())?;

    sys.refresh_processes(ProcessesToUpdate::All, true);
    if needs_warmup {
        // 首次采样没有基线，CPU 恒为 0；补一次间隔采样拿到真实值。
        std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL + Duration::from_millis(50));
        sys.refresh_processes(ProcessesToUpdate::All, true);
    }

    let self_pid = std::process::id();
    let mut processes: Vec<ProcessInfo> = Vec::with_capacity(sys.processes().len());
    let mut seen_ports: HashSet<u16> = HashSet::new();

    for (pid, process) in sys.processes() {
        let pid_u32 = pid.as_u32();
        let raw_name = process.name().to_string_lossy().trim().to_string();
        let cmd = process
            .cmd()
            .iter()
            .map(|part| part.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        let ports = port_map.get(&pid_u32).cloned().unwrap_or_default();
        seen_ports.extend(ports.iter().copied());

        processes.push(ProcessInfo {
            pid: pid_u32,
            name: if raw_name.is_empty() {
                format!("PID {}", pid_u32)
            } else {
                raw_name
            },
            exe: process
                .exe()
                .map(|path| path.to_string_lossy().to_string()),
            cmd,
            memory: process.memory(),
            cpu: process.cpu_usage(),
            ports,
            is_self: pid_u32 == self_pid,
        });
    }

    let total = processes.len();
    Ok(ProcessSnapshot {
        processes,
        total,
        ports_available,
        port_count: seen_ports.len(),
        platform: platform_tag().to_string(),
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

/// 按 PID 读取进程名（用于结束进程后的回执文案；查不到则返回空串）。
fn lookup_name(state: &tauri::State<'_, ProcessState>, pid: u32) -> String {
    let Ok(sys) = state.system.lock() else {
        return String::new();
    };
    sys.processes()
        .get(&Pid::from_u32(pid))
        .map(|process| process.name().to_string_lossy().trim().to_string())
        .unwrap_or_default()
}

/// 批量把 PID 补上进程名（一次性持锁，避免逐个查询反复加锁）。
fn with_names(state: &tauri::State<'_, ProcessState>, pids: Vec<u32>) -> Vec<(u32, String)> {
    let mut names: HashMap<u32, String> = HashMap::new();
    if let Ok(sys) = state.system.lock() {
        for pid in &pids {
            if let Some(process) = sys.processes().get(&Pid::from_u32(*pid)) {
                names.insert(*pid, process.name().to_string_lossy().trim().to_string());
            }
        }
    }
    pids.into_iter()
        .map(|pid| {
            let name = names.get(&pid).cloned().unwrap_or_default();
            (pid, name)
        })
        .collect()
}

/// 把「结束结果」整理成回执 DTO。
fn outcome(pid: u32, name: String, result: Result<(), String>) -> KillOutcome {
    KillOutcome {
        pid,
        name: if name.is_empty() {
            format!("PID {}", pid)
        } else {
            name
        },
        ok: result.is_ok(),
        error: result.err(),
    }
}

/// 平台标识（前端据此切换文案，如 Windows 的 taskkill 提示）。
fn platform_tag() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    }
}

// ============================ 结束进程 ============================

/// 结束进程（平台实现分离，避免 cfg 分支把函数体切碎）。
///
/// Windows：`taskkill /PID <pid> /T`，`/T` 连带结束子进程树；`/F` 为强杀。
/// 系统保留进程（PID 0~4，Idle / System / 内核线程）与自身进程一律拒绝。
#[cfg(target_os = "windows")]
fn terminate(pid: u32, force: bool) -> Result<(), String> {
    if pid <= 4 {
        return Err("系统保留进程不可结束".to_string());
    }
    if pid == std::process::id() {
        return Err("不能结束本应用自身进程".to_string());
    }

    let mut cmd = hidden_command("taskkill");
    cmd.arg("/PID").arg(pid.to_string()).arg("/T");
    if force {
        cmd.arg("/F");
    }

    let output = cmd
        .output()
        .map_err(|e| format!("调用 taskkill 失败: {}", e))?;
    if output.status.success() {
        return Ok(());
    }

    // taskkill 的 stderr 是系统本地化编码（中文 Windows 为 GBK），
    // 直接 from_utf8_lossy 会输出乱码，故按退出码给出可读文案。
    Err(match output.status.code() {
        Some(1) => "结束进程被拒绝（权限不足或进程受保护）".to_string(),
        Some(128) => "进程不存在或已退出".to_string(),
        Some(code) => format!("结束进程失败（taskkill 退出码 {}）", code),
        None => "结束进程失败（taskkill 异常终止）".to_string(),
    })
}

/// 结束进程（Unix）：`kill -TERM`，强杀时 `-KILL`。
#[cfg(unix)]
fn terminate(pid: u32, force: bool) -> Result<(), String> {
    if pid <= 4 {
        // PID 1 是 init / launchd，结束它会直接拖垮系统。
        return Err("系统保留进程不可结束".to_string());
    }
    if pid == std::process::id() {
        return Err("不能结束本应用自身进程".to_string());
    }

    let signal = if force { "-KILL" } else { "-TERM" };
    let output = Command::new("kill")
        .arg(signal)
        .arg(pid.to_string())
        .output()
        .map_err(|e| format!("调用 kill 失败: {}", e))?;
    if output.status.success() {
        return Ok(());
    }

    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if detail.is_empty() {
        "结束进程失败（权限不足或进程不存在）".to_string()
    } else {
        format!("结束进程失败: {}", detail)
    })
}

/// 构造不弹控制台窗口的子进程命令（仅 Windows 需要）。
///
/// 本应用是 GUI 子系统（无控制台），直接 spawn `netstat` / `taskkill`
/// 会为每个子进程闪出一个黑色控制台窗，故统一加 CREATE_NO_WINDOW。
#[cfg(target_os = "windows")]
fn hidden_command(program: &str) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut cmd = Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

// ============================ 端口枚举 ============================

/// PID → 占用端口（升序去重）。
type PortMap = HashMap<u32, Vec<u16>>;

/// 读取本机端口占用；平台无可用手段时返回 None（前端提示端口信息不可用）。
fn collect_ports() -> Option<PortMap> {
    let mut map = raw_ports()?;
    for ports in map.values_mut() {
        ports.sort_unstable();
        ports.dedup();
    }
    Some(map)
}

#[cfg(target_os = "windows")]
fn raw_ports() -> Option<PortMap> {
    ports_from_netstat_ano()
}

#[cfg(target_os = "macos")]
fn raw_ports() -> Option<PortMap> {
    ports_from_lsof()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn raw_ports() -> Option<PortMap> {
    // Linux：优先 lsof（信息最全），未安装时兜底 net-tools 的 netstat。
    ports_from_lsof().or_else(ports_from_netstat_unix)
}

#[cfg(not(any(windows, unix)))]
fn raw_ports() -> Option<PortMap> {
    None
}

/// 把端口记入映射（同 PID 同端口只记一次，排序在 collect_ports 统一做）。
fn push_port(map: &mut PortMap, pid: u32, port: u16) {
    let entry = map.entry(pid).or_default();
    if !entry.contains(&port) {
        entry.push(port);
    }
}

/// 从地址串里取端口号，兼容 IPv4 / IPv6 / lsof 的点分隔写法：
/// `0.0.0.0:135`、`[::]:135`、`*:3000`、`127.0.0.1.5000`；`*:*` 之类无端口返回 None。
fn parse_port(address: &str) -> Option<u16> {
    let raw = address.rsplit([':', '.']).next()?;
    match raw.parse::<u16>() {
        Ok(0) | Err(_) => None,
        Ok(port) => Some(port),
    }
}

/// Windows：解析 `netstat -ano`。
///
/// ```text
///   TCP    0.0.0.0:135      0.0.0.0:0      LISTENING    1234
///   UDP    0.0.0.0:500      *:*                          1234
/// ```
/// 表头行首个 token 不是 TCP/UDP，会被自然跳过（中英文系统表头不同，不做匹配）。
/// TCP 只统计监听态（连接态的临时端口对「谁占了端口」没有意义）；UDP 无状态列，全部计入。
#[cfg(target_os = "windows")]
fn ports_from_netstat_ano() -> Option<PortMap> {
    let output = hidden_command("netstat").arg("-ano").output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut map = PortMap::new();

    for line in text.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        if toks.len() < 4 {
            continue;
        }
        let is_tcp = toks[0].eq_ignore_ascii_case("TCP");
        let is_udp = toks[0].eq_ignore_ascii_case("UDP");
        if !is_tcp && !is_udp {
            continue;
        }
        // `-o` 保证末列恒为 PID；解析失败说明不是数据行。
        let Ok(pid) = toks[toks.len() - 1].parse::<u32>() else {
            continue;
        };
        if is_tcp && !toks[3].eq_ignore_ascii_case("LISTENING") {
            continue;
        }
        if let Some(port) = parse_port(toks[1]) {
            push_port(&mut map, pid, port);
        }
    }

    Some(map)
}

/// macOS / Linux：解析 `lsof -nP -i`。
///
/// ```text
/// COMMAND   PID  USER  FD  TYPE  DEVICE  SIZE/OFF NODE NAME
/// node    12345  bob   23u IPv4  0x1234      0t0  TCP *:3000 (LISTEN)
/// ```
/// `-n` 不做 DNS 反查、`-P` 不做端口名反查，保证输出里是纯数字端口。
/// TCP 只收 `(LISTEN)` 行；UDP 无状态标记，全部计入。
#[cfg(unix)]
fn ports_from_lsof() -> Option<PortMap> {
    let output = Command::new("lsof").args(["-nP", "-i"]).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    // lsof 未安装时 output() 已失败返回；stdout 为空且退出码非 0 视为不可用。
    if !output.status.success() && text.trim().is_empty() {
        return None;
    }

    let mut map = PortMap::new();
    for line in text.lines().skip(1) {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let Some(pid) = toks.get(1).and_then(|v| v.parse::<u32>().ok()) else {
            continue;
        };
        // TCP/UDP 标记之后紧跟本地地址（NAME）。
        let Some(proto_idx) = toks.iter().position(|t| *t == "TCP" || *t == "UDP") else {
            continue;
        };
        if toks[proto_idx] == "TCP" && !line.contains("(LISTEN)") {
            continue;
        }
        let Some(name) = toks.get(proto_idx + 1) else {
            continue;
        };
        // 防御：连接态形如 `127.0.0.1:5432->1.2.3.4:5`，只取本地段。
        let local = name.split("->").next().unwrap_or(name);
        if let Some(port) = parse_port(local) {
            push_port(&mut map, pid, port);
        }
    }

    Some(map)
}

/// Linux 兜底：解析 `netstat -tunap`（net-tools 未安装时返回 None）。
///
/// ```text
/// Proto Recv-Q Send-Q Local Address  Foreign Address  State   PID/Program name
/// tcp        0      0 0.0.0.0:22     0.0.0.0:*        LISTEN  1234/sshd
/// udp        0      0 0.0.0.0:68     0.0.0.0:*                1234/dhclient
/// ```
#[cfg(all(unix, not(target_os = "macos")))]
fn ports_from_netstat_unix() -> Option<PortMap> {
    let output = Command::new("netstat").args(["-tunap"]).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut map = PortMap::new();

    for line in text.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        if toks.len() < 6 {
            continue;
        }
        let proto = toks[0].to_ascii_lowercase();
        let is_tcp = proto.starts_with("tcp");
        let is_udp = proto.starts_with("udp");
        if !is_tcp && !is_udp {
            continue;
        }
        // tcp 行第 6 列是状态；udp 行没有状态列，该位置已是 PID。
        if is_tcp && !toks[5].eq_ignore_ascii_case("LISTEN") && !toks[5].eq_ignore_ascii_case("LISTENING") {
            continue;
        }
        // 末列形如 `1234/sshd`；无权限时是 `-`，解析失败即跳过。
        let Some(pid) = toks
            .last()
            .and_then(|v| v.split('/').next())
            .and_then(|v| v.parse::<u32>().ok())
        else {
            continue;
        };
        if let Some(port) = parse_port(toks[3]) {
            push_port(&mut map, pid, port);
        }
    }

    Some(map)
}
