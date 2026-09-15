/**
 * useTodoPanel — 待办日历面板（管理窗口第三个标签页）的 composable。
 *
 * 职责：
 *   - 月历视图：生成 6 周网格，格子上以色点标记当天任务，点击选中某天；
 *   - 选中日的任务列表：新增（快速回车）/ 编辑 / 删除 / 勾选完成；
 *   - 任务编辑器：标题 / 备注 / 日期 / 颜色 / 状态 / 多条提醒（HH:mm）/ 每日任务；
 *   - 提醒横幅：展示到点的提醒，支持「标记完成 / 稍后 / 关闭」；
 *   - 固定到桌面：切换 todo-board 置顶小窗，并同步按钮固定状态；
 *   - 宠物上方日程展示范围（近七日 / 近半月 / 关闭）。
 */
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { useTodoScheduleStore } from '@/stores/todoSchedule'
import {
  addDays,
  fromDateStr,
  isTaskDoneOnDate,
  taskOccursOn,
  toDateStr,
  todayStr,
  TODO_COLORS,
  TODO_DEFAULT_COLOR,
  type PetScheduleMode,
  type TodoColorKey,
  type TodoReminder,
  type TodoStatus,
  type TodoTask
} from '@/types/todoSchedule'
import {
  isTodoBoardVisible,
  toggleTodoBoardWindow
} from '@/services/todoBoard'

/** 一周从周日开始（与格子网格一致）。 */
const WEEKDAY_KEYS = [
  'ui.todo.weekday.sun',
  'ui.todo.weekday.mon',
  'ui.todo.weekday.tue',
  'ui.todo.weekday.wed',
  'ui.todo.weekday.thu',
  'ui.todo.weekday.fri',
  'ui.todo.weekday.sat'
] as const

/** 日历格子。 */
export interface CalendarCell {
  /** YYYY-MM-DD。 */
  date: string
  day: number
  /** 是否属于当前浏览月份。 */
  inMonth: boolean
  /** 是否今天。 */
  isToday: boolean
  /** 是否选中日。 */
  isSelected: boolean
}

/** 编辑器里的提醒草稿（编辑中允许空行）。 */
interface ReminderDraft {
  id: string
  time: string
  enabled: boolean
}

/** 任务编辑表单草稿。 */
interface TaskDraft {
  title: string
  note: string
  date: string
  color: TodoColorKey
  status: TodoStatus
  repeatDaily: boolean
  reminders: ReminderDraft[]
}

/** 新建时的默认草稿。 */
function emptyDraft(date: string): TaskDraft {
  return {
    title: '',
    note: '',
    date,
    color: TODO_DEFAULT_COLOR,
    status: 'pending',
    repeatDaily: false,
    reminders: []
  }
}

/** 任务 → 表单草稿。 */
function taskToDraft(task: TodoTask): TaskDraft {
  return {
    title: task.title,
    note: task.note,
    date: task.date,
    color: task.color,
    status: task.status,
    repeatDaily: task.repeatDaily,
    reminders: task.reminders.map((r) => ({ ...r }))
  }
}

let draftSeq = 0
function nextDraftId(): string {
  draftSeq += 1
  return `d_${draftSeq}_${Math.random().toString(36).slice(2, 6)}`
}

export function useTodoPanel() {
  const { t, locale } = useI18n()
  const todoStore = useTodoScheduleStore()

  // --- 月历导航 -------------------------------------------------------------
  const now = new Date()
  const viewYear = ref(now.getFullYear())
  const viewMonth = ref(now.getMonth()) // 0-11
  const selectedDate = ref(todayStr())

  /** 月份标题，如 "2026年9月" / "September 2026"。 */
  const monthTitle = computed(() => {
    const d = new Date(viewYear.value, viewMonth.value, 1)
    return d.toLocaleDateString(locale.value === 'zh-CN' ? 'zh-CN' : 'en-US', {
      year: 'numeric',
      month: 'long'
    })
  })

  const weekdayLabels = computed(() => WEEKDAY_KEYS.map((k) => t(k)))

  /** 6 周（42 格）日历网格：从当月 1 号所在周的周日开始。 */
  const calendarCells = computed<CalendarCell[]>(() => {
    const first = new Date(viewYear.value, viewMonth.value, 1)
    const start = new Date(first)
    start.setDate(1 - first.getDay()) // 回退到本周周日
    const today = todayStr()
    const cells: CalendarCell[] = []
    for (let i = 0; i < 42; i += 1) {
      const d = new Date(start)
      d.setDate(start.getDate() + i)
      const date = toDateStr(d)
      cells.push({
        date,
        day: d.getDate(),
        inMonth: d.getMonth() === viewMonth.value,
        isToday: date === today,
        isSelected: date === selectedDate.value
      })
    }
    return cells
  })

  function prevMonth(): void {
    if (viewMonth.value === 0) {
      viewYear.value -= 1
      viewMonth.value = 11
    } else {
      viewMonth.value -= 1
    }
  }

  function nextMonth(): void {
    if (viewMonth.value === 11) {
      viewYear.value += 1
      viewMonth.value = 0
    } else {
      viewMonth.value += 1
    }
  }

  function goToday(): void {
    const t0 = new Date()
    viewYear.value = t0.getFullYear()
    viewMonth.value = t0.getMonth()
    selectedDate.value = toDateStr(t0)
  }

  // --- 选中日任务 -----------------------------------------------------------

  const selectedTasks = computed(() => todoStore.tasksForDate(selectedDate.value))
  const selectedPendingCount = computed(
    () => selectedTasks.value.filter((t) => !isTaskDoneOnDate(t, selectedDate.value)).length
  )

  /** 某格日期的任务色点（颜色去重）。 */
  function colorDotsOf(date: string): TodoColorKey[] {
    const seen = new Set<TodoColorKey>()
    for (const task of todoStore.tasksForDate(date)) {
      seen.add(task.color)
    }
    return [...seen].slice(0, 4)
  }

  /** 某格日期是否有未完成任务（加粗标记）。 */
  function hasPendingOn(date: string): boolean {
    return todoStore.tasksForDate(date).some((t) => isTaskPending(t, date))
  }

  /** 局部别名避免每格调用 long 名（保持可读）。 */
  function isTaskPending(task: TodoTask, date: string): boolean {
    return taskOccursOn(task, date) && !isTaskDoneOnDate(task, date)
  }

  // --- 快速添加 -------------------------------------------------------------

  const quickTitle = ref('')

  /** 回车快速添加到选中日期（继承默认色，可再编辑补提醒）。 */
  function handleQuickAdd(): void {
    const title = quickTitle.value.trim()
    if (!title) return
    todoStore.addTask({ title, date: selectedDate.value })
    quickTitle.value = ''
  }

  // --- 任务编辑器 -----------------------------------------------------------

  const editorVisible = ref(false)
  /** 正在编辑的任务 id；null 表示新建。 */
  const editingId = ref<string | null>(null)
  const draft = ref<TaskDraft>(emptyDraft(selectedDate.value))

  function openCreate(date: string = selectedDate.value): void {
    editingId.value = null
    draft.value = emptyDraft(date)
    editorVisible.value = true
  }

  function openEdit(task: TodoTask): void {
    editingId.value = task.id
    draft.value = taskToDraft(task)
    editorVisible.value = true
  }

  function closeEditor(): void {
    editorVisible.value = false
  }

  /** 保存编辑器：新建或更新。标题为空时不保存。 */
  function saveEditor(): void {
    const title = draft.value.title.trim()
    if (!title) return
    const reminders: TodoReminder[] = draft.value.reminders
      .filter((r) => /^\d{2}:\d{2}$/.test(r.time))
      .map((r) => ({ id: r.id || nextDraftId(), time: r.time, enabled: r.enabled }))
    if (editingId.value) {
      todoStore.updateTask(editingId.value, { ...draft.value, title, reminders })
    } else {
      todoStore.addTask({ ...draft.value, title, reminders })
    }
    // 编辑器日期若在别的月份，日历跟随跳转。
    const d = fromDateStr(draft.value.date)
    viewYear.value = d.getFullYear()
    viewMonth.value = d.getMonth()
    selectedDate.value = draft.value.date
    editorVisible.value = false
  }

  function removeTask(task: TodoTask): void {
    todoStore.removeTask(task.id)
  }

  function toggleDone(task: TodoTask): void {
    todoStore.toggleDoneOnDate(task.id, selectedDate.value)
  }

  /** 切换任务状态（编辑器内）。 */
  function setDraftStatus(status: TodoStatus): void {
    draft.value.status = status
  }

  // 编辑器：提醒行操作

  function addReminderRow(): void {
    draft.value.reminders.push({ id: nextDraftId(), time: '09:00', enabled: true })
  }

  function removeReminderRow(index: number): void {
    draft.value.reminders.splice(index, 1)
  }

  // 编辑器：颜色色板

  const colorOptions = TODO_COLORS

  function colorLabel(key: TodoColorKey): string {
    return t(`ui.todo.color.${key}`)
  }

  // --- 提醒横幅 -------------------------------------------------------------

  const activeReminders = computed(() => todoStore.activeReminders)

  /** 提醒横幅动作：标记完成（每日任务按触发日期记完成）。 */
  function handleReminderDone(key: string): void {
    const item = todoStore.activeReminders.find((r) => r.key === key)
    if (item) todoStore.toggleDoneOnDate(item.taskId, item.date)
    todoStore.dismissReminder(key)
  }

  function handleReminderSnooze(key: string): void {
    todoStore.snoozeReminder(key)
  }

  /** 提醒横幅动作：仅关闭。 */
  function handleReminderClose(key: string): void {
    todoStore.dismissReminder(key)
  }

  // --- 固定到桌面 ------------------------------------------------------------

  const boardVisible = ref(false)

  async function refreshBoardVisible(): Promise<void> {
    try {
      boardVisible.value = await isTodoBoardVisible()
    } catch {
      boardVisible.value = false
    }
  }

  /** 切换「固定到桌面」：显示 / 隐藏 todo-board 置顶小窗。 */
  async function handleToggleBoard(): Promise<void> {
    try {
      boardVisible.value = await toggleTodoBoardWindow()
    } catch (e) {
      console.error('[TodoPanel] toggle board failed:', e)
    }
  }

  // --- 宠物上方日程展示范围 ----------------------------------------------------

  const petScheduleMode = computed(() => todoStore.settings.petScheduleMode)

  const petScheduleOptions = computed<Array<{ value: PetScheduleMode; label: string }>>(() => [
    { value: 'off', label: t('ui.todo.petSchedule.off') },
    { value: '7d', label: t('ui.todo.petSchedule.7d') },
    { value: '15d', label: t('ui.todo.petSchedule.15d') }
  ])

  function handlePetScheduleMode(value: PetScheduleMode): void {
    todoStore.setPetScheduleMode(value)
  }

  // --- 近期待办数量（面板头部徽标） ---------------------------------------------

  const upcoming7Count = computed(() => todoStore.upcoming7Count)

  // --- 生命周期 --------------------------------------------------------------

  onMounted(() => {
    // 各窗口独立运行提醒结算（横幅只在本窗口展示）。
    todoStore.startReminderTicker()
    void refreshBoardVisible()
  })

  // 跨窗口开关 todo-board 后同步按钮状态（简单轮询，低频无压力）。
  let boardPollTimer: ReturnType<typeof setInterval> | null = null
  boardPollTimer = setInterval(() => void refreshBoardVisible(), 5000)
  if (typeof window !== 'undefined') {
    window.addEventListener('unload', () => {
      if (boardPollTimer) clearInterval(boardPollTimer)
    })
  }

  // 外部修改任务（如固定窗口勾选）时，编辑器若开着且编辑同一任务，关闭避免覆盖。
  watch(
    () => todoStore.tasks,
    () => {
      if (editorVisible.value && editingId.value) {
        const stillThere = todoStore.tasks.some((t) => t.id === editingId.value)
        if (!stillThere) editorVisible.value = false
      }
    },
    { deep: true }
  )

  return {
    // i18n / store
    t,
    todoStore,
    // 月历
    viewYear,
    viewMonth,
    monthTitle,
    weekdayLabels,
    calendarCells,
    selectedDate,
    prevMonth,
    nextMonth,
    goToday,
    colorDotsOf,
    hasPendingOn,
    // 选中日
    selectedTasks,
    selectedPendingCount,
    toggleDone,
    removeTask,
    // 快速添加
    quickTitle,
    handleQuickAdd,
    // 编辑器
    editorVisible,
    editingId,
    draft,
    openCreate,
    openEdit,
    closeEditor,
    saveEditor,
    setDraftStatus,
    addReminderRow,
    removeReminderRow,
    colorOptions,
    colorLabel,
    // 提醒
    activeReminders,
    handleReminderDone,
    handleReminderSnooze,
    handleReminderClose,
    // 固定到桌面
    boardVisible,
    handleToggleBoard,
    // 宠物日程
    petScheduleMode,
    petScheduleOptions,
    handlePetScheduleMode,
    upcoming7Count,
    // 工具
    addDays
  }
}
