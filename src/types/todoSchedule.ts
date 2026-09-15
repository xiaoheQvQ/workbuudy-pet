/**
 * 待办事项与日历日程的类型与数值配置。
 *
 * 数据语义：
 *   - 任务挂在某个日期（YYYY-MM-DD）上；可设置多个提醒时间（精确到 HH:mm）。
 *   - repeatDaily = true 表示「每日任务 / 每日待办」：每天都会出现在日历与列表里，
 *     完成状态按日期记录在 completedDates（而非布尔值），跨天自动重置为待办。
 *   - color 为颜色标记（预设色板），用于日历格子与列表条目的色点/色条。
 */

/** 任务状态：待办 / 已完成。 */
export type TodoStatus = 'pending' | 'done'

/** 颜色标记 key（对应 TODO_COLORS 色板与 i18n ui.todo.color.*）。 */
export type TodoColorKey =
  | 'red'
  | 'orange'
  | 'amber'
  | 'green'
  | 'teal'
  | 'blue'
  | 'purple'
  | 'pink'
  | 'gray'

/** 单条提醒定义。time 为 HH:mm（24 小时制，精确到时分）。 */
export interface TodoReminder {
  id: string
  /** HH:mm，如 "09:30"。 */
  time: string
  /** 是否启用（保留配置但可临时关闭单条提醒）。 */
  enabled: boolean
}

/** 单条待办 / 日程任务。 */
export interface TodoTask {
  id: string
  /** 标题。 */
  title: string
  /** 备注（可选）。 */
  note: string
  /** 所属日期 YYYY-MM-DD。repeatDaily 时该值表示首次日期，展示时按日重复展开。 */
  date: string
  /** 颜色标记。 */
  color: TodoColorKey
  /** 状态。 */
  status: TodoStatus
  /** 是否为每日任务 / 每日待办（每天重复出现）。 */
  repeatDaily: boolean
  /** 每日任务已完成的具体日期（YYYY-MM-DD 列表），非每日任务忽略。 */
  completedDates: string[]
  /** 提醒列表（支持多次提醒，精确到时分）。 */
  reminders: TodoReminder[]
  createdAt: number
  updatedAt: number
}

/** 色板定义：hex 用于 UI，labelKey 引用 ui.todo.color.* 文案。 */
export interface TodoColorDef {
  key: TodoColorKey
  hex: string
  labelKey: string
}

/** 预设色板（任务颜色标记）。 */
export const TODO_COLORS: readonly TodoColorDef[] = [
  { key: 'red', hex: '#ef4444', labelKey: 'ui.todo.color.red' },
  { key: 'orange', hex: '#f97316', labelKey: 'ui.todo.color.orange' },
  { key: 'amber', hex: '#eab308', labelKey: 'ui.todo.color.amber' },
  { key: 'green', hex: '#22c55e', labelKey: 'ui.todo.color.green' },
  { key: 'teal', hex: '#14b8a6', labelKey: 'ui.todo.color.teal' },
  { key: 'blue', hex: '#3b82f6', labelKey: 'ui.todo.color.blue' },
  { key: 'purple', hex: '#8b5cf6', labelKey: 'ui.todo.color.purple' },
  { key: 'pink', hex: '#ec4899', labelKey: 'ui.todo.color.pink' },
  { key: 'gray', hex: '#9ca3af', labelKey: 'ui.todo.color.gray' }
]

/** 默认颜色（新建任务时）。 */
export const TODO_DEFAULT_COLOR: TodoColorKey = 'blue'

/** 按颜色 key 取 hex（未知 key 回退默认色）。 */
export function todoColorHex(key: TodoColorKey): string {
  return TODO_COLORS.find((c) => c.key === key)?.hex ?? '#3b82f6'
}

/** 宠物形象上方日程展示范围：关闭 / 近七日 / 近半月。 */
export type PetScheduleMode = 'off' | '7d' | '15d'

/** 稍后提醒的延迟时长（ms，10 分钟）。 */
export const SNOOZE_DELAY_MS = 10 * 60 * 1000

// --- 日期工具（YYYY-MM-DD 本地日期，避免时区/UTC 偏移问题） -----------------

/** 本地日期 → YYYY-MM-DD 字符串。 */
export function toDateStr(d: Date): string {
  const m = `${d.getMonth() + 1}`.padStart(2, '0')
  const day = `${d.getDate()}`.padStart(2, '0')
  return `${d.getFullYear()}-${m}-${day}`
}

/** 今天的 YYYY-MM-DD。 */
export function todayStr(): string {
  return toDateStr(new Date())
}

/** 字符串 → 当天 00:00 的 Date（本地时区）。 */
export function fromDateStr(s: string): Date {
  const [y, m, d] = s.split('-').map((n) => parseInt(n, 10))
  return new Date(y, (m ?? 1) - 1, d ?? 1)
}

/** 日期加减天数，返回 YYYY-MM-DD。 */
export function addDays(dateStr: string, days: number): string {
  const d = fromDateStr(dateStr)
  d.setDate(d.getDate() + days)
  return toDateStr(d)
}

/** HH:mm → 当天该时刻的时间戳（本地时区）。 */
export function timeOnDate(dateStr: string, time: string): number {
  const [h, min] = time.split(':').map((n) => parseInt(n, 10))
  const d = fromDateStr(dateStr)
  d.setHours(h ?? 0, min ?? 0, 0, 0)
  return d.getTime()
}

/**
 * 判断任务在指定日期是否「已完成」。
 * 每日任务看 completedDates 是否包含该日；普通任务看 status。
 */
export function isTaskDoneOnDate(task: TodoTask, dateStr: string): boolean {
  if (task.repeatDaily) return task.completedDates.includes(dateStr)
  return task.status === 'done'
}

/** 判断任务在指定日期是否「待办」（出现且未完成）。 */
export function isTaskPendingOnDate(task: TodoTask, dateStr: string): boolean {
  if (task.repeatDaily) return dateStr >= task.date && !task.completedDates.includes(dateStr)
  return task.date === dateStr && task.status === 'pending'
}

/** 判断任务是否出现在指定日期（每日任务自 date 起每天出现）。 */
export function taskOccursOn(task: TodoTask, dateStr: string): boolean {
  if (task.repeatDaily) return dateStr >= task.date
  return task.date === dateStr
}
