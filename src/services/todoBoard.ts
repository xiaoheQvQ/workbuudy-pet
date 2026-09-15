/**
 * 待办「固定到桌面」窗口（todo-board）的 IPC 薄封装。
 *
 * 窗口由 Rust 侧 todo_board.rs 管理：创建 / 定位右下角 / 显隐切换。
 * 前端只通过这里 invoke，便于统一错误处理与类型。
 */
import { invoke } from '@tauri-apps/api/core'

/** 显示桌面待办窗口（定位到当前显示器右下角）。 */
export function showTodoBoardWindow(): Promise<void> {
  return invoke('show_todo_board_window')
}

/** 隐藏桌面待办窗口（取消固定）。 */
export function hideTodoBoardWindow(): Promise<void> {
  return invoke('hide_todo_board_window')
}

/** 切换桌面待办窗口显隐，返回切换后是否可见。 */
export function toggleTodoBoardWindow(): Promise<boolean> {
  return invoke('toggle_todo_board_window')
}

/** 查询桌面待办窗口当前是否可见。 */
export function isTodoBoardVisible(): Promise<boolean> {
  return invoke('is_todo_board_visible')
}
