/**
 * 进程管理面板的数据类型（与 Rust 侧 `commands/process.rs` 的 DTO 一一对应）。
 */

/** 单个进程信息。 */
export interface ProcessInfo {
  /** 进程 ID。 */
  pid: number
  /** 进程名（如 chrome.exe）。 */
  name: string
  /** 可执行文件完整路径（内核线程等可能为 null）。 */
  exe: string | null
  /** 完整命令行（可能为空串）。 */
  cmd: string
  /** 物理内存占用（字节）。 */
  memory: number
  /** CPU 占用（百分比；多核满载可超过 100）。 */
  cpu: number
  /** 该进程占用的端口（升序去重；仅 TCP 监听 / UDP 绑定）。 */
  ports: number[]
  /** 是否为当前应用自身进程（禁止结束）。 */
  isSelf: boolean
}

/** 一次进程枚举的结果。 */
export interface ProcessSnapshot {
  /** 进程列表（未排序，排序在前端做，切换排序不重复 IPC）。 */
  processes: ProcessInfo[]
  /** 系统进程总数。 */
  total: number
  /** 端口信息是否读取成功（false 时端口列不可用）。 */
  portsAvailable: boolean
  /** 本次枚举涉及的去重端口总数。 */
  portCount: number
  /** 平台标识：windows / macos / linux / unknown。 */
  platform: string
  /** 本次枚举耗时（毫秒）。 */
  elapsedMs: number
}

/** 结束进程的结果（批量结束返回数组）。 */
export interface KillOutcome {
  pid: number
  name: string
  ok: boolean
  /** 失败原因（成功时为 null）。 */
  error: string | null
}

/** 列表排序维度。 */
export type ProcessSortKey = 'memory' | 'cpu' | 'name' | 'ports'

/**
 * 搜索框支持的命令式关键词。
 *
 * 输入这些词会切换到「关键词模式」，套用对应的默认排序与筛选：
 *   - netstat            → 按端口数排序 + 只看占用端口的进程（端口视角）；
 *   - kill / taskkill /
 *     tasklist / ps /
 *     process            → 按内存占用排序。
 */
export const PROCESS_KEYWORDS = [
  'kill',
  'taskkill',
  'tasklist',
  'netstat',
  'ps',
  'process'
] as const

export type ProcessKeyword = (typeof PROCESS_KEYWORDS)[number]

/** 每个关键词的默认排序与筛选规则。 */
export const PROCESS_KEYWORD_RULES: Record<
  ProcessKeyword,
  { sort: ProcessSortKey; onlyWithPorts: boolean }
> = {
  netstat: { sort: 'ports', onlyWithPorts: true },
  kill: { sort: 'memory', onlyWithPorts: false },
  taskkill: { sort: 'memory', onlyWithPorts: false },
  tasklist: { sort: 'memory', onlyWithPorts: false },
  ps: { sort: 'memory', onlyWithPorts: false },
  process: { sort: 'memory', onlyWithPorts: false }
}

/** 各排序维度的默认方向（true = 升序）。内存 / CPU / 端口数按「越大越靠前」，名称按字典序。 */
export const PROCESS_SORT_DEFAULT_ASC: Record<ProcessSortKey, boolean> = {
  memory: false,
  cpu: false,
  ports: false,
  name: true
}
