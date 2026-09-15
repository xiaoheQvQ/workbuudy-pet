/**
 * 待办事项与日历日程 store。
 *
 * 与 petCare 同款模式：
 *   - Setup Store + localStorage 持久化（key: workbuddy-pet-todo）
 *   - persist + watch 自动落盘 + storage 事件跨窗口同步
 *     （main / pet / todo-board 三个窗口各自一份 Pinia，storage 事件是唯一跨窗通知）
 *   - 提醒结算采用「懒结算」：定时器周期扫描，把到点且未触发的提醒一次性结算出来，
 *     触发记录持久化（key: workbuddy-pet-todo-fired），重启后不重复提醒。
 *
 * 每日任务：repeatDaily = true 的任务按日展开，完成状态记录在 completedDates，
 * 跨天自动回到待办（无需定时重置）。
 *
 * 提醒触发条件：当前时刻 ≥ 任务日期+提醒时刻，且在宽限期内（避免打开应用时
 * 把很久以前的提醒全部爆出来）；触发后写入 fired 表不再重复。
 */
import { defineStore } from 'pinia'
import { computed, reactive, ref, watch } from 'vue'

import {
  addDays,
  isTaskDoneOnDate,
  isTaskPendingOnDate,
  SNOOZE_DELAY_MS,
  taskOccursOn,
  timeOnDate,
  todayStr,
  TODO_DEFAULT_COLOR,
  type PetScheduleMode,
  type TodoColorKey,
  type TodoReminder,
  type TodoStatus,
  type TodoTask
} from '@/types/todoSchedule'

const STORAGE_KEY = 'workbuddy-pet-todo'
/** 提醒触发记录（重启后不重复提醒）。 */
const FIRED_KEY = 'workbuddy-pet-todo-fired'
/** 偏好设置（宠物上方日程展示范围等）。 */
const SETTINGS_KEY = 'workbuddy-pet-todo-settings'

/** 提醒扫描定时器间隔（ms）。 */
const REMINDER_TICK_MS = 15_000
/** 触发宽限期：仅结算距提醒时刻 12h 以内的到期提醒（更久远的视为过期跳过）。 */
const REMINDER_GRACE_MS = 12 * 60 * 60 * 1000
/** fired 表裁剪：只保留最近 7 天的记录。 */
const FIRED_RETAIN_MS = 7 * 24 * 60 * 60 * 1000

/** 当前生效的提醒（已到点、待用户处理）。 */
export interface ActiveReminder {
  /** 触发 key（taskId:date:time），用于去重。 */
  key: string
  taskId: string
  title: string
  color: TodoColorKey
  /** 提醒时刻对应的任务日期（每日任务为今天）。 */
  date: string
  /** HH:mm。 */
  time: string
  firedAt: number
}

/** 生成任务 id。 */
function genId(): string {
  return `t_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`
}

/** 从 localStorage 读取任务列表（容错）。 */
function loadTasks(): TodoTask[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed.filter(
      (t): t is TodoTask => !!t && typeof t.id === 'string' && typeof t.title === 'string'
    )
  } catch {
    return []
  }
}

/** 读取已触发提醒表：key → 触发时间戳。 */
function loadFired(): Record<string, number> {
  try {
    const raw = localStorage.getItem(FIRED_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw)
    return parsed && typeof parsed === 'object' ? parsed : {}
  } catch {
    return {}
  }
}

/** 偏好设置类型。 */
interface TodoSettings {
  /** 宠物形象上方日程展示范围。 */
  petScheduleMode: PetScheduleMode
}

const DEFAULT_SETTINGS: TodoSettings = { petScheduleMode: 'off' }

function loadSettings(): TodoSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY)
    if (!raw) return { ...DEFAULT_SETTINGS }
    return { ...DEFAULT_SETTINGS, ...(JSON.parse(raw) as Partial<TodoSettings>) }
  } catch {
    return { ...DEFAULT_SETTINGS }
  }
}

export const useTodoScheduleStore = defineStore('todoSchedule', () => {
  // --- 状态 ---------------------------------------------------------------
  const tasks = ref<TodoTask[]>(loadTasks())
  const settings = reactive<TodoSettings>(loadSettings())
  const fired = ref<Record<string, number>>(loadFired())
  /** 当前生效的提醒（各窗口独立结算，展示各自的提醒横幅）。 */
  const activeReminders = ref<ActiveReminder[]>([])

  // --- 持久化与跨窗口同步 ---------------------------------------------------
  let writing = false

  function persistTasks(): void {
    try {
      writing = true
      localStorage.setItem(STORAGE_KEY, JSON.stringify(tasks.value))
    } catch (e) {
      console.error('[todoSchedule] persist tasks failed:', e)
    } finally {
      writing = false
    }
  }

  function persistFired(): void {
    try {
      // 裁剪过期记录，避免无限增长。
      const cutoff = Date.now() - FIRED_RETAIN_MS
      const next: Record<string, number> = {}
      for (const [k, v] of Object.entries(fired.value)) {
        if (v >= cutoff) next[k] = v
      }
      fired.value = next
      writing = true
      localStorage.setItem(FIRED_KEY, JSON.stringify(next))
    } catch (e) {
      console.error('[todoSchedule] persist fired failed:', e)
    } finally {
      writing = false
    }
  }

  function persistSettings(): void {
    try {
      writing = true
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings))
    } catch (e) {
      console.error('[todoSchedule] persist settings failed:', e)
    } finally {
      writing = false
    }
  }

  if (typeof window !== 'undefined') {
    // 跨窗口同步：另一窗口写入后整体替换（值相同不会触发回环）。
    window.addEventListener('storage', (e) => {
      if (writing) return
      if (e.key === STORAGE_KEY) {
        tasks.value = loadTasks()
      } else if (e.key === FIRED_KEY) {
        fired.value = loadFired()
      } else if (e.key === SETTINGS_KEY) {
        Object.assign(settings, loadSettings())
      }
    })
  }

  watch(tasks, persistTasks, { deep: true })
  watch(settings, persistSettings, { deep: true })

  // --- CRUD ---------------------------------------------------------------

  /** 新增任务（缺省字段在内部补齐），返回新任务。 */
  function addTask(input: Partial<TodoTask> & { title: string }): TodoTask {
    const now = Date.now()
    const task: TodoTask = {
      id: genId(),
      title: input.title,
      note: input.note ?? '',
      date: input.date ?? todayStr(),
      color: input.color ?? TODO_DEFAULT_COLOR,
      status: input.status ?? 'pending',
      repeatDaily: input.repeatDaily ?? false,
      completedDates: input.completedDates ?? [],
      reminders: (input.reminders ?? []).filter((r) => r.time),
      createdAt: now,
      updatedAt: now
    }
    tasks.value = [...tasks.value, task]
    return task
  }

  /** 更新任务（按 id 合并字段）。返回是否存在。 */
  function updateTask(id: string, patch: Partial<TodoTask>): boolean {
    const idx = tasks.value.findIndex((t) => t.id === id)
    if (idx < 0) return false
    const next: TodoTask = {
      ...tasks.value[idx]!,
      ...patch,
      id,
      updatedAt: Date.now()
    }
    if (patch.reminders) {
      next.reminders = patch.reminders.filter((r) => r.time)
    }
    const copy = [...tasks.value]
    copy[idx] = next
    tasks.value = copy
    return true
  }

  /** 删除任务。 */
  function removeTask(id: string): void {
    tasks.value = tasks.value.filter((t) => t.id !== id)
  }

  /**
   * 切换任务在指定日期的完成状态。
   * 每日任务：维护 completedDates；普通任务：翻转 status。
   */
  function toggleDoneOnDate(id: string, dateStr: string): void {
    const task = tasks.value.find((t) => t.id === id)
    if (!task) return
    if (task.repeatDaily) {
      const done = task.completedDates.includes(dateStr)
      const dates = done
        ? task.completedDates.filter((d) => d !== dateStr)
        : [...task.completedDates, dateStr]
      updateTask(id, { completedDates: dates })
    } else {
      updateTask(id, { status: (task.status === 'done' ? 'pending' : 'done') as TodoStatus })
    }
  }

  // --- 查询 ---------------------------------------------------------------

  /** 某一天的任务（每日任务按日展开；按创建时间排序）。 */
  function tasksForDate(dateStr: string): TodoTask[] {
    return tasks.value
      .filter((t) => taskOccursOn(t, dateStr))
      .sort((a, b) => a.createdAt - b.createdAt)
  }

  /** [from, to] 闭区间内（含每日任务）出现且未完成的任务。 */
  function pendingTasksBetween(from: string, to: string): Array<TodoTask & { occurDate: string }> {
    const result: Array<TodoTask & { occurDate: string }> = []
    for (let d = from; d <= to; d = addDays(d, 1)) {
      for (const t of tasksForDate(d)) {
        if (isTaskPendingOnDate(t, d)) {
          result.push({ ...t, occurDate: d })
        }
      }
    }
    return result
  }

  /** 指定日期是否为今天。 */
  function isToday(dateStr: string): boolean {
    return dateStr === todayStr()
  }

  // --- 提醒结算（懒结算） ---------------------------------------------------

  /**
   * 扫描一遍提醒：把到点且未触发过的提醒推入 activeReminders。
   * 每日任务提醒按「今天」结算；普通任务按任务日期结算。
   */
  function settleReminders(now = Date.now()): void {
    const today = todayStr()
    const firedSnapshot = { ...fired.value }
    let firedDirty = false
    const incoming: ActiveReminder[] = []

    for (const task of tasks.value) {
      for (const r of task.reminders) {
        if (!r.enabled) continue
        const occurDate = task.repeatDaily ? today : task.date
        const dueAt = timeOnDate(occurDate, r.time)
        if (now < dueAt) continue
        if (now - dueAt > REMINDER_GRACE_MS) continue
        const key = `${task.id}:${occurDate}:${r.time}`
        if (firedSnapshot[key]) continue
        firedSnapshot[key] = now
        firedDirty = true
        incoming.push({
          key,
          taskId: task.id,
          title: task.title,
          color: task.color,
          date: occurDate,
          time: r.time,
          firedAt: now
        })
      }
    }

    if (incoming.length > 0) {
      fired.value = firedSnapshot
      if (firedDirty) persistFired()
      activeReminders.value = [...activeReminders.value, ...incoming]
    }
  }

  /** 关闭一条提醒。 */
  function dismissReminder(key: string): void {
    activeReminders.value = activeReminders.value.filter((r) => r.key !== key)
  }

  /** 稍后提醒：先关闭，SNOOZE_DELAY_MS 后重新弹出。 */
  function snoozeReminder(key: string): void {
    const item = activeReminders.value.find((r) => r.key === key)
    dismissReminder(key)
    if (!item) return
    setTimeout(() => {
      activeReminders.value = [...activeReminders.value, { ...item, firedAt: Date.now() }]
    }, SNOOZE_DELAY_MS)
  }

  /** 提醒定时器（各窗口独立运行，均只展示自己的横幅）。 */
  let reminderTimer: ReturnType<typeof setInterval> | null = null

  function startReminderTicker(): void {
    if (reminderTimer) return
    // 启动时先结算一次（应用离线期间到点的提醒在宽限期内仍会弹出）。
    settleReminders()
    reminderTimer = setInterval(() => settleReminders(), REMINDER_TICK_MS)
  }

  // --- 偏好 ---------------------------------------------------------------

  /** 设置宠物上方日程展示范围。 */
  function setPetScheduleMode(mode: PetScheduleMode): void {
    settings.petScheduleMode = mode
  }

  // --- 派生 ---------------------------------------------------------------

  /** 今天起近 N 天的未完成任务（用于宠物上方日程与固定桌面列表）。 */
  function upcomingPending(days: number): Array<TodoTask & { occurDate: string }> {
    const from = todayStr()
    return pendingTasksBetween(from, addDays(from, days - 1))
  }

  /** 近 7 日/近半月待办数量（图标角标等用）。 */
  const upcoming7Count = computed(() => upcomingPending(7).length)

  return {
    tasks,
    settings,
    activeReminders,
    addTask,
    updateTask,
    removeTask,
    toggleDoneOnDate,
    tasksForDate,
    pendingTasksBetween,
    upcomingPending,
    isTaskDoneOnDate,
    isToday,
    dismissReminder,
    snoozeReminder,
    startReminderTicker,
    setPetScheduleMode,
    upcoming7Count
  }
})

/** 导出提醒类型（供视图层复用）。 */
export type { TodoReminder, TodoTask }
