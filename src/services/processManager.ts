/**
 * 进程管理（系统进程枚举 / 端口占用 / 结束进程）的 IPC 薄封装。
 *
 * 实现全部在 Rust 侧 `commands/process.rs`：进程枚举走 sysinfo，
 * 端口占用走各平台原生命令，结束进程走 taskkill / kill。
 * 前端只通过这里 invoke，便于统一类型与错误边界。
 */
import { invoke } from '@tauri-apps/api/core'
import type { KillOutcome, ProcessSnapshot } from '@/types/process'

/**
 * 枚举当前系统进程。
 *
 * @param includePorts 是否读取端口占用。端口枚举需调用外部命令（Windows `netstat`
 *   约 100~300ms，macOS `lsof` 更慢），关闭后刷新更快，但端口列与端口排序会失去数据。
 */
export function listProcesses(includePorts = true): Promise<ProcessSnapshot> {
  return invoke<ProcessSnapshot>('list_processes', { includePorts })
}

/** 结束单个进程（`force = true` 为强杀）。 */
export function killProcess(pid: number, force = false): Promise<KillOutcome> {
  return invoke<KillOutcome>('kill_process', { pid, force })
}

/** 批量结束进程（按端口 / 名称筛选后一次清理多个占用者）。 */
export function killProcesses(pids: number[], force = false): Promise<KillOutcome[]> {
  return invoke<KillOutcome[]>('kill_processes', { pids, force })
}
