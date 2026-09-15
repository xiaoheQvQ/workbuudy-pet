/**
 * useTodoBoard — 桌面固定待办列表窗口（todo-board）的 composable。
 *
 * 「固定到桌面」小窗：始终置顶，展示完整待办列表（今天起近 15 日按日分组），
 * 可直接勾选完成 / 删除 / 回车快速添加；到点提醒在本窗口以横幅展示。
 * 数据来自 todoSchedule store（localStorage 跨窗口同步，主窗口改动实时反映）。
 */
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { useTodoScheduleStore } from '@/stores/todoSchedule'
import {
  addDays,
  isTaskDoneOnDate,
  todoColorHex,
  todayStr,
  type TodoTask
} from '@/types/todoSchedule'
import { hideTodoBoardWindow } from '@/services/todoBoard'

/** 列表展示范围：今天起 15 天。 */
const RANGE_DAYS = 15
/** 「已完成」区展示最近 7 天。 */
const DONE_RANGE_DAYS = 7

/** 按日期分组的待办。 */
export interface DayGroup {
  date: string
  /** 分组标题：今天 / 明天 / 其余显示 MM-DD。 */
  label: string
  tasks: TodoTask[]
}

export function useTodoBoard() {
  const { t, locale } = useI18n()
  const todoStore = useTodoScheduleStore()

  /** 日期分组标题。 */
  function dayLabel(date: string): string {
    const today = todayStr()
    if (date === today) return t('ui.todo.todayLabel', { date })
    if (date === addDays(today, 1)) return t('ui.todo.tomorrow', { date })
    return date.slice(5)
  }

  /** 今天起 15 天内的待办，按日分组（只含有任务的日期）。 */
  const dayGroups = computed<DayGroup[]>(() => {
    const today = todayStr()
    const groups: DayGroup[] = []
    for (let i = 0; i < RANGE_DAYS; i += 1) {
      const date = addDays(today, i)
      const tasks = todoStore
        .tasksForDate(date)
        .filter((task) => !isTaskDoneOnDate(task, date))
      if (tasks.length > 0) {
        groups.push({ date, label: dayLabel(date), tasks })
      }
    }
    return groups
  })

  const pendingCount = computed(() =>
    dayGroups.value.reduce((sum, g) => sum + g.tasks.length, 0)
  )

  // --- 已完成（近 7 日，可折叠） --------------------------------------------

  const showDone = ref(false)

  /** 近 7 天完成的任务（普通任务按 status，每日任务按 completedDates）。 */
  const recentDone = computed<Array<{ task: TodoTask; date: string }>>(() => {
    const today = todayStr()
    const from = addDays(today, -(DONE_RANGE_DAYS - 1))
    const result: Array<{ task: TodoTask; date: string }> = []
    for (const task of todoStore.tasks) {
      if (task.repeatDaily) {
        for (const d of task.completedDates) {
          if (d >= from && d <= today) result.push({ task, date: d })
        }
      } else if (task.status === 'done' && task.date >= from && task.date <= today) {
        result.push({ task, date: task.date })
      }
    }
    return result.sort((a, b) => b.date.localeCompare(a.date))
  })

  // --- 操作 ----------------------------------------------------------------

  const quickTitle = ref('')

  function handleQuickAdd(): void {
    const title = quickTitle.value.trim()
    if (!title) return
    todoStore.addTask({ title, date: todayStr() })
    quickTitle.value = ''
  }

  function toggleDone(task: TodoTask, date: string): void {
    todoStore.toggleDoneOnDate(task.id, date)
  }

  function removeTask(task: TodoTask): void {
    todoStore.removeTask(task.id)
  }

  /** 打开日历（聚焦管理窗口，用户在「待办日历」标签页编辑完整日程）。 */
  async function openCalendar(): Promise<void> {
    try {
      const { emit } = await import('@tauri-apps/api/event')
      await emit('desktop-pet:open-todo', {})
    } catch (e) {
      console.error('[TodoBoard] open calendar failed:', e)
    }
  }

  /** 收起（隐藏窗口 = 取消固定）。 */
  async function hideBoard(): Promise<void> {
    try {
      await hideTodoBoardWindow()
    } catch (e) {
      console.error('[TodoBoard] hide board failed:', e)
    }
  }

  /** 提醒横幅动作。 */
  function reminderDone(key: string): void {
    const item = todoStore.activeReminders.find((r) => r.key === key)
    if (item) todoStore.toggleDoneOnDate(item.taskId, item.date)
    todoStore.dismissReminder(key)
  }

  function reminderSnooze(key: string): void {
    todoStore.snoozeReminder(key)
  }

  function reminderClose(key: string): void {
    todoStore.dismissReminder(key)
  }

  /** 色点样式。 */
  function dotStyle(key: string): Record<string, string> {
    return { background: todoColorHex(key as never) }
  }

  onMounted(() => {
    todoStore.startReminderTicker()
  })

  return {
    t,
    locale,
    dayGroups,
    pendingCount,
    showDone,
    recentDone,
    quickTitle,
    handleQuickAdd,
    toggleDone,
    removeTask,
    openCalendar,
    hideBoard,
    activeReminders: computed(() => todoStore.activeReminders),
    reminderDone,
    reminderSnooze,
    reminderClose,
    dotStyle
  }
}
