/**
 * useProcessPanel — 进程管理面板的 composable。
 *
 * 交互模型（对齐 tasklist / netstat 的使用习惯）：
 *   - 搜索框输入进程名 / PID / 端口即按关键字过滤，可临时关闭；
 *   - 输入 kill、netstat、ps、tasklist、taskkill、process 等命令式关键词会切换「关键词模式」：
 *     netstat 按端口数排序并只看占用端口的进程；其余关键词按内存占用排序；
 *     关键词后面还能跟参数（如 `netstat 3000`、`kill chrome`）继续做过滤；
 *   - 排序支持内存 / CPU / 名称 / 端口数，点击表头切换维度与升降序；
 *   - 结束进程先弹确认层（可勾选强杀），支持单个与批量。
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useMessage } from 'naive-ui'
import { useI18n } from 'vue-i18n'
import { killProcess, killProcesses, listProcesses } from '@/services/processManager'
import {
  PROCESS_KEYWORDS,
  PROCESS_KEYWORD_RULES,
  PROCESS_SORT_DEFAULT_ASC,
  type ProcessInfo,
  type ProcessSortKey
} from '@/types/process'

/** 自动刷新可选的间隔（毫秒）。 */
const REFRESH_INTERVAL_OPTIONS = [2000, 3000, 5000, 10000]

/** 端口列最多直接展示几个端口（其余折叠为 +N）。 */
const PORT_PREVIEW_LIMIT = 4

/** 确认弹层里最多列出几个进程名。 */
const KILL_LIST_LIMIT = 8

export function useProcessPanel() {
  const message = useMessage()
  const { t } = useI18n()

  // --- 列表数据 -----------------------------------------------------------
  const processes = ref<ProcessInfo[]>([])
  const loading = ref(false)
  /** 读取失败原因（网络 / IPC 异常，展示在状态行下方）。 */
  const error = ref<string | null>(null)
  const lastUpdated = ref<number | null>(null)
  const elapsedMs = ref(0)
  const total = ref(0)
  const portCount = ref(0)
  const portsAvailable = ref(true)

  // --- 查询 / 排序 --------------------------------------------------------
  const query = ref('')
  const sortKey = ref<ProcessSortKey>('memory')
  const sortAsc = ref(false)
  /** 只看占用端口的进程（netstat 关键词会自动打开）。 */
  const onlyWithPorts = ref(false)
  /** 是否读取端口信息（关闭可显著加快刷新）。 */
  const includePorts = ref(true)

  // --- 自动刷新 -----------------------------------------------------------
  const autoRefresh = ref(false)
  const refreshInterval = ref(3000)
  let timer: number | null = null

  // --- 选中 / 结束 --------------------------------------------------------
  const selectedPids = ref<number[]>([])
  const expandedPid = ref<number | null>(null)
  /** 确认弹层的目标进程（空数组 = 不展示）。 */
  const killTargets = ref<ProcessInfo[]>([])
  const killVisible = ref(false)
  const killForce = ref(false)
  const killBusy = ref(false)

  // --- 搜索解析 -----------------------------------------------------------

  /**
   * 解析搜索框内容：首个 token 若是命令式关键词则进入关键词模式，
   * 其余部分作为过滤文本（如 `netstat 3000` → 关键词 netstat + 过滤 3000）。
   */
  const parsedQuery = computed<{ keyword: string | null; text: string }>(() => {
    const raw = query.value.trim()
    if (!raw) return { keyword: null, text: '' }
    const [head, ...rest] = raw.split(/\s+/)
    const lower = head.toLowerCase()
    if ((PROCESS_KEYWORDS as readonly string[]).includes(lower)) {
      return { keyword: lower, text: rest.join(' ') }
    }
    return { keyword: null, text: raw }
  })

  const keyword = computed(() => parsedQuery.value.keyword)

  /** 关键词模式提示（说明当前默认排序规则）。 */
  const keywordHint = computed(() => {
    if (!keyword.value) return ''
    return keyword.value === 'netstat' ? t('ui.process.hintNetstat') : t('ui.process.hintKill')
  })

  // 命中关键词即套用其默认排序与筛选（kill 系列按内存、netstat 按端口）。
  watch(keyword, (value) => {
    if (!value) return
    const rule = PROCESS_KEYWORD_RULES[value as keyof typeof PROCESS_KEYWORD_RULES]
    if (!rule) return
    sortKey.value = rule.sort
    sortAsc.value = PROCESS_SORT_DEFAULT_ASC[rule.sort]
    onlyWithPorts.value = rule.onlyWithPorts
  })

  // --- 过滤 / 排序 --------------------------------------------------------

  /** 单条进程是否命中过滤词（名称 / PID / 路径 / 命令行 / 端口）。 */
  function matches(process: ProcessInfo, text: string): boolean {
    if (process.name.toLowerCase().includes(text)) return true
    if (String(process.pid).includes(text)) return true
    if (process.exe && process.exe.toLowerCase().includes(text)) return true
    if (process.cmd.toLowerCase().includes(text)) return true
    // 端口按精确匹配：#3000 不该命中 30001。
    return process.ports.some((port) => String(port) === text)
  }

  const filtered = computed<ProcessInfo[]>(() => {
    const text = parsedQuery.value.text.trim().toLowerCase()
    let list = processes.value
    if (onlyWithPorts.value) {
      list = list.filter((process) => process.ports.length > 0)
    }
    if (!text) return list
    return list.filter((process) => matches(process, text))
  })

  /** 排序比较器（仅返回「原始」差值，方向由调用方决定）。 */
  function compare(a: ProcessInfo, b: ProcessInfo, key: ProcessSortKey): number {
    switch (key) {
      case 'name':
        return a.name.localeCompare(b.name)
      case 'cpu':
        return a.cpu - b.cpu
      case 'ports':
        return a.ports.length - b.ports.length
      default:
        return a.memory - b.memory
    }
  }

  const sorted = computed<ProcessInfo[]>(() => {
    const key = sortKey.value
    const asc = sortAsc.value
    return [...filtered.value].sort((a, b) => {
      const directed = asc ? compare(a, b, key) : -compare(a, b, key)
      // 同值兜底按名称升序，避免刷新时行序抖动。
      if (directed !== 0) return directed
      return a.name.localeCompare(b.name)
    })
  })

  /** 当前展示条数。 */
  const visibleCount = computed(() => sorted.value.length)

  /** 本应用自身进程（用于状态行提示）。 */
  const selfProcess = computed(() => processes.value.find((item) => item.isSelf) ?? null)

  // --- 选中 --------------------------------------------------------------

  const selectedSet = computed(() => new Set(selectedPids.value))

  /** 可勾选的可见行（排除本应用自身进程）。 */
  const selectableShown = computed(() => sorted.value.filter((process) => !process.isSelf))

  const allSelected = computed(
    () =>
      selectableShown.value.length > 0 &&
      selectableShown.value.every((process) => selectedSet.value.has(process.pid))
  )

  function toggleSelect(pid: number): void {
    selectedPids.value = selectedPids.value.includes(pid)
      ? selectedPids.value.filter((value) => value !== pid)
      : [...selectedPids.value, pid]
  }

  function toggleSelectAll(): void {
    selectedPids.value = allSelected.value
      ? []
      : selectableShown.value.map((process) => process.pid)
  }

  function clearSelection(): void {
    selectedPids.value = []
  }

  function toggleExpand(pid: number): void {
    expandedPid.value = expandedPid.value === pid ? null : pid
  }

  // --- 读取 --------------------------------------------------------------

  /** 拉取一次进程快照。并发保护：上一次未结束时忽略本次触发。 */
  async function refresh(): Promise<void> {
    if (loading.value) return
    loading.value = true
    error.value = null
    try {
      const snapshot = await listProcesses(includePorts.value)
      processes.value = snapshot.processes
      total.value = snapshot.total
      portCount.value = snapshot.portCount
      portsAvailable.value = snapshot.portsAvailable
      elapsedMs.value = snapshot.elapsedMs
      lastUpdated.value = Date.now()

      // 清理已退出进程的选中 / 展开状态，避免结束进程后残留幽灵选中项。
      const alive = new Set(snapshot.processes.map((process) => process.pid))
      selectedPids.value = selectedPids.value.filter((pid) => alive.has(pid))
      if (expandedPid.value !== null && !alive.has(expandedPid.value)) {
        expandedPid.value = null
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      loading.value = false
    }
  }

  // --- 排序交互 ----------------------------------------------------------

  /** 点击表头：同维度切换升降序，换维度则用该维度的默认方向。 */
  function setSort(key: ProcessSortKey): void {
    if (sortKey.value === key) {
      sortAsc.value = !sortAsc.value
      return
    }
    sortKey.value = key
    sortAsc.value = PROCESS_SORT_DEFAULT_ASC[key]
  }

  /** 表头排序箭头（非当前维度返回空串）。 */
  function sortIndicator(key: ProcessSortKey): string {
    if (sortKey.value !== key) return ''
    return sortAsc.value ? '↑' : '↓'
  }

  /** 点击关键词胶囊：写入搜索框，watch 会自动套用该关键词的默认排序。 */
  function applyKeyword(value: string): void {
    query.value = value
  }

  // --- 结束进程 ----------------------------------------------------------

  /** 打开结束确认层（单个 / 批量共用）；自身进程一律排除。 */
  function requestKill(targets: ProcessInfo[], force = false): void {
    const list = targets.filter((process) => !process.isSelf)
    if (list.length === 0) return
    killTargets.value = list
    killForce.value = force
    killVisible.value = true
  }

  function requestKillOne(process: ProcessInfo, force = false): void {
    requestKill([process], force)
  }

  function requestKillSelected(): void {
    requestKill(sorted.value.filter((process) => selectedSet.value.has(process.pid)))
  }

  function closeKill(): void {
    if (killBusy.value) return
    killVisible.value = false
  }

  /** 确认结束：按目标数量走单个 / 批量命令，成功后立即刷新列表。 */
  async function confirmKill(): Promise<void> {
    if (killBusy.value || killTargets.value.length === 0) return
    killBusy.value = true
    const targets = [...killTargets.value]
    try {
      const outcomes =
        targets.length === 1
          ? [await killProcess(targets[0].pid, killForce.value)]
          : await killProcesses(
              targets.map((process) => process.pid),
              killForce.value
            )

      const succeeded = outcomes.filter((item) => item.ok)
      const failed = outcomes.filter((item) => !item.ok)

      if (succeeded.length === 1) {
        message.success(t('ui.process.killOk', { name: succeeded[0].name }))
      } else if (succeeded.length > 1) {
        message.success(t('ui.process.killOkBatch', { count: succeeded.length }))
      }
      if (failed.length > 0) {
        message.error(
          t('ui.process.killFail', {
            count: failed.length,
            error: failed[0].error ?? ''
          }),
          { duration: 6000 }
        )
      }
      clearSelection()
    } catch (e) {
      message.error(
        t('ui.process.killFail', {
          count: targets.length,
          error: e instanceof Error ? e.message : String(e)
        }),
        { duration: 6000 }
      )
    } finally {
      killBusy.value = false
      killVisible.value = false
      await refresh()
    }
  }

  // --- 复制（管理动作）---------------------------------------------------

  /** 复制文本到剪贴板（WebView 下 navigator.clipboard 可能不可用，回退 execCommand）。 */
  async function copyText(text: string, label: string): Promise<void> {
    if (!text) return
    try {
      await navigator.clipboard.writeText(text)
      message.success(t('ui.process.copied', { label }))
      return
    } catch {
      // 继续走下面的兜底实现。
    }
    try {
      const area = document.createElement('textarea')
      area.value = text
      area.style.position = 'fixed'
      area.style.opacity = '0'
      document.body.appendChild(area)
      area.select()
      const ok = document.execCommand('copy')
      document.body.removeChild(area)
      message[ok ? 'success' : 'error'](
        ok ? t('ui.process.copied', { label }) : t('ui.process.copyFailed')
      )
    } catch {
      message.error(t('ui.process.copyFailed'))
    }
  }

  function handleCopyPid(process: ProcessInfo): void {
    void copyText(String(process.pid), t('ui.process.col.pid'))
  }

  function handleCopyCmd(process: ProcessInfo): void {
    void copyText(process.cmd, t('ui.process.detail.cmd'))
  }

  // --- 格式化 ------------------------------------------------------------

  const MEMORY_UNITS = ['B', 'KB', 'MB', 'GB', 'TB']

  /** 字节 → 人类可读（如 1.2 GB）；小于 1KB 直接显示字节。 */
  function formatMemory(bytes: number): string {
    if (!bytes) return '0 B'
    let value = bytes
    let unit = 0
    while (value >= 1024 && unit < MEMORY_UNITS.length - 1) {
      value /= 1024
      unit += 1
    }
    return `${unit === 0 ? value : value.toFixed(value >= 100 ? 0 : 1)} ${MEMORY_UNITS[unit]}`
  }

  /** CPU 百分比：小数值保留一位，大数值取整避免抖动。 */
  function formatCpu(cpu: number): string {
    return `${cpu >= 10 ? cpu.toFixed(0) : cpu.toFixed(1)}%`
  }

  /** 时间戳 → HH:mm:ss（刷新时间展示）。 */
  function formatTime(timestamp: number | null): string {
    if (!timestamp) return '—'
    const date = new Date(timestamp)
    const pad = (value: number): string => String(value).padStart(2, '0')
    return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
  }

  // --- 自动刷新 ----------------------------------------------------------

  function stopTimer(): void {
    if (timer !== null) {
      window.clearInterval(timer)
      timer = null
    }
  }

  function syncTimer(): void {
    stopTimer()
    if (!autoRefresh.value) return
    timer = window.setInterval(() => {
      void refresh()
    }, refreshInterval.value)
  }

  watch([autoRefresh, refreshInterval], syncTimer)
  // 切换是否读端口后立即重新采样（否则端口列会一直空着）。
  watch(includePorts, () => {
    void refresh()
  })

  onMounted(() => {
    void refresh()
  })

  onUnmounted(stopTimer)

  return {
    t,
    // 数据
    processes,
    sorted,
    loading,
    error,
    total,
    visibleCount,
    portCount,
    portsAvailable,
    elapsedMs,
    lastUpdated,
    selfProcess,
    // 查询 / 排序
    query,
    keyword,
    keywordHint,
    keywordChips: PROCESS_KEYWORDS,
    sortKey,
    sortAsc,
    setSort,
    sortIndicator,
    applyKeyword,
    onlyWithPorts,
    includePorts,
    // 自动刷新
    autoRefresh,
    refreshInterval,
    refreshIntervalOptions: REFRESH_INTERVAL_OPTIONS,
    refresh,
    // 选中
    selectedPids,
    selectedSet,
    allSelected,
    toggleSelect,
    toggleSelectAll,
    clearSelection,
    expandedPid,
    toggleExpand,
    // 结束进程
    killVisible,
    killTargets,
    killForce,
    killBusy,
    requestKillOne,
    requestKillSelected,
    closeKill,
    confirmKill,
    // 管理动作
    handleCopyPid,
    handleCopyCmd,
    // 格式化
    formatMemory,
    formatCpu,
    formatTime,
    // 常量
    portPreviewLimit: PORT_PREVIEW_LIMIT,
    killListLimit: KILL_LIST_LIMIT
  }
}
