/**
 * usePetManager — 宠物管理窗口主视图的 composable。
 *
 * 集成代理扩展（WorkBuddy 联动）：
 *   - 接通 vue-i18n：选项 / label / 提示统一走 `t()`，文案 key 定义于 src/locales。
 *   - locale 切换：watch petSettings.locale → 同步 useI18n 的 `locale.value`（与全局同步）。
 *   - 缩放：双向绑定 petSettings.scale（PetView 监听重建 PetApp）。
 *   - WorkBuddy 联动：`n-switch` 绑定 workbuddyLinked，切换调 `link_workbuddy`，失败 toast，成功提示新建会话。
 */
import { computed, onMounted, ref, watch } from 'vue'
import { useMessage } from 'naive-ui'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { usePetSettingsStore } from '@/stores/petSettings'
import { useDesktopPetStore } from '@/stores/desktopPet'
import { usePetMarketStore } from '@/stores/petMarket'
import { toLocalAssetUrl } from '@/services/desktopPet'
import { useAppUpdate } from '@/composables/useAppUpdate'
import type { AppLocale } from '@/locales'
import { supportedLocales } from '@/locales'
import type { LocalPetInfo } from '@/types/desktopPet'
import type { MarketPet } from '@/types/petMarket'
import type { DetailPet } from './PetDetailModal/usePetDetailModal'

/** 下拉选项。 */
interface Option {
  value: string
  label: string
}

export function usePetManager() {
  const message = useMessage()
  const { t, locale } = useI18n()
  const petSettings = usePetSettingsStore()
  const desktopPetStore = useDesktopPetStore()
  const petMarketStore = usePetMarketStore()
  const { updateInfo, checkForAppUpdate, downloadAndInstallUpdate, installing } = useAppUpdate()

  // --- 设置区折叠状态 -----------------------------------------------------
  // 默认收起：设置区常驻会挤压宠物列表垂直空间，收起后宠物列表获得最大高度。
  const settingsOpen = ref(false)

  // --- 标签页（我的宠物 / 在线市场）----------------------------------------
  const activeTab = ref<'local' | 'market'>('local')

  // --- 详情弹窗状态 ------------------------------------------------------
  const detailVisible = ref(false)
  const detailPet = ref<DetailPet | null>(null)

  // --- WorkBuddy 联动状态 ----------------------------------------------------
  // 与后端 `link_workbuddy(enabled)` 返回的 LinkResult 对齐（serde camelCase）。
  interface WorkBuddyLinkResult {
    ok: boolean
    linked: boolean
    scriptPath: string
    configPath: string
  }
  /** 当前联动状态（onMounted 拉取真实值；切换时按返回值更新）。 */
  const workbuddyLinked = ref(false)
  /** 切换中（禁用 switch，防止重复 IPC）。 */
  const workbuddyToggling = ref(false)

  // --- WorkBuddy 数据目录（token 统计用） ------------------------------------
  /** 自动检测到的 DB 路径（展示给用户，证明自动检测已生效）。 */
  const workbuddyDbPath = ref<string | null>(null)
  /** 用户手动输入的覆盖路径（空串=自动检测）。 */
  const workbuddyDataDirInput = ref('')
  /** 路径检测是否失败（失败时高亮提示用户手动填）。 */
  const workbuddyDataDirMissing = ref(false)
  /** 设置数据目录中。 */
  const workbuddyDataDirSaving = ref(false)

  // --- 选项 --------------------------------------------------------------

  /** 语言下拉选项（label 本地化）。 */
  const languageOptions = computed<Option[]>(() => [
    { value: 'zh-CN', label: '中文' },
    { value: 'en-US', label: 'English' }
  ])

  /** 漫游模式下拉选项：自由（走步）/ 固定（不走步，停在原地）。 */
  const movementModeOptions = computed<Option[]>(() => [
    { value: 'free', label: t('ui.pet.movementFree') },
    { value: 'fixed', label: t('ui.pet.movementFixed') }
  ])

  /** 缩放滑块范围（与 PetView scale/100 一致）。 */
  const SCALE_MIN = 50
  const SCALE_MAX = 125
  const SCALE_STEP = 5

  // --- handlers：开关 ----------------------------------------------------

  /** 启用/禁用桌面宠物。开启时显示窗口并自动联动 WorkBuddy；关闭时隐藏并断开联动。 */
  async function handleToggleEnabled(enabled: boolean): Promise<void> {
    petSettings.enabled = enabled
    if (enabled) {
      await desktopPetStore.loadLocalPets()
      await desktopPetStore.showPet()
      // 开启宠物时自动联动 WorkBuddy（无需用户单独开开关）。
      // Node.js 检测失败时静默跳过（不阻断宠物开启，仅 toast 提示）。
      if (!workbuddyLinked.value) {
        try {
          await invoke<string>('check_node_available')
          const result = await invoke<WorkBuddyLinkResult>('link_workbuddy', { enabled: true })
          if (result.ok) {
            workbuddyLinked.value = result.linked
            message.info(t('ui.workbuddy.relinkHint'), { duration: 4000 })
          }
        } catch {
          // Node.js 不可用或联动注入失败：静默，宠物仍可正常使用。
        }
      }
    } else {
      await desktopPetStore.hidePet()
    }
  }

  /** 切换始终置顶。 */
  async function handleToggleAlwaysOnTop(value: boolean): Promise<void> {
    await desktopPetStore.setAlwaysOnTop(value)
  }

  /**
   * 选择宠物为当前使用。
   * 若宠物未启用，则自动开启并显示悬浮窗。
   */
  async function handleSelectPet(petId: string): Promise<void> {
    await desktopPetStore.setActivePet(petId)
    if (!petSettings.enabled) {
      petSettings.enabled = true
      await desktopPetStore.showPet()
    }
  }

  // --- handlers：详情弹窗 ------------------------------------------------

  /** 本地宠物 → 详情（src 用 convertFileSrc）。 */
  function openLocalDetail(pet: LocalPetInfo): void {
    detailPet.value = {
      id: pet.id,
      displayName: pet.displayName,
      description: pet.description,
      kind: pet.kind,
      tags: pet.tags,
      spritesheetSrc: toLocalAssetUrl(pet.spritesheetPath),
      installed: true,
      source: pet.source,
      installedAt: pet.installedAt
    }
    detailVisible.value = true
  }

  /** 详情弹窗：设为当前宠物。 */
  async function handleDetailUse(petId: string): Promise<void> {
    await desktopPetStore.setActivePet(petId)
  }

  /** 市场宠物 → 详情（remote spritesheetSrc；已安装则与本地状态一致）。 */
  function openMarketDetail(pet: MarketPet): void {
    const installed = desktopPetStore.localPets.some((item) => item.id === pet.slug)
    detailPet.value = {
      id: pet.slug,
      displayName: pet.displayName,
      // 官网搜索接口带描述与标签，未安装也能完整预览。
      description: pet.description,
      kind: pet.kind,
      tags: pet.tags,
      spritesheetSrc: pet.spritesheetUrl,
      installed,
      // 已安装的走本地来源标签，未安装的标为在线来源（决定按钮显示「安装」还是「设为当前」）。
      source: installed ? 'downloaded' : 'market',
      submittedBy: pet.submittedBy
    }
    detailVisible.value = true
  }

  /**
   * 安装市场宠物（市场卡片与详情弹窗共用入口）。
   *
   * 成功后 store 会自动刷新本地列表并设为当前宠物；若详情弹窗正展示该宠物，
   * 同步把其状态改为「已安装」，让「安装到本地」按钮立即变成「设为当前」。
   */
  async function handleInstallPet(slug: string): Promise<void> {
    try {
      const info = await petMarketStore.install(slug)
      message.success(t('ui.market.installSuccess', { name: info.displayName }))
      if (detailPet.value?.id === slug) {
        detailPet.value = { ...detailPet.value, installed: true, source: 'downloaded' }
      }
    } catch (e) {
      const detail = e instanceof Error ? e.message : String(e)
      message.error(t('ui.market.installFailed', { error: detail }), { duration: 6000 })
    }
  }

  // --- handlers：语言 / 缩放 / WorkBuddy 联动 -------------------------------

  /**
   * 切换界面语言：同步写入 petSettings.locale（持久化）与 useI18n 的 locale.value
   * （后者与全局 i18n.global.locale 同步，legacy:false 模式下即时生效）。
   */
  function handleLanguageChange(value: AppLocale): void {
    petSettings.locale = value
    locale.value = value
  }

  /**
   * 切换缩放百分比。仅写 store（持久化 + 联动）；PetView 监听该字段重建 PetApp。
   * 钳制到 [50, 125] 以 5 为步进，避免越界。
   */
  function handleScaleChange(value: number): void {
    const clamped = Math.max(SCALE_MIN, Math.min(SCALE_MAX, value))
    petSettings.scale = clamped
  }

  /**
   * 切换漫游模式（自由 / 固定）。仅写 store（持久化）；PetView 监听该字段实时应用到引擎，
   * 无需重建 PetApp。管理窗口与宠物窗口经 localStorage 同步。
   */
  function handleMovementModeChange(value: 'free' | 'fixed'): void {
    petSettings.movementMode = value
  }

  /**
   * 切换 WorkBuddy shell hook 联动：调 `link_workbuddy(enabled)`，成功提示「新建会话生效」，
   * 失败回滚 switch 状态并 toast 报错。
   *
   * 开启前先检测系统 Node.js：hook 脚本以 `node <script>` 被 WorkBuddy 拉起，
   * 无 node 则联动静默失效，故提前拦截并提示用户安装。
   */
  async function handleToggleWorkBuddyLink(enabled: boolean): Promise<void> {
    if (workbuddyToggling.value) return
    workbuddyToggling.value = true
    try {
      // 开启联动前检测 Node.js 是否可用（关闭不需要）。
      if (enabled) {
        try {
          const version = await invoke<string>('check_node_available')
          message.success(t('ui.workbuddy.nodeOk', { version }), { duration: 2500 })
        } catch {
          // node 不可用：回滚开关并提示安装，不继续注入配置。
          workbuddyLinked.value = false
          message.error(t('ui.workbuddy.nodeMissing'), { duration: 6000 })
          return
        }
      }

      const result = await invoke<WorkBuddyLinkResult>('link_workbuddy', { enabled })
      if (!result.ok) {
        // 操作未成功：回滚 UI 状态并提示。
        workbuddyLinked.value = !enabled
        message.error(t('ui.workbuddy.relinkHint'))
        return
      }
      workbuddyLinked.value = result.linked
      message.info(t('ui.workbuddy.relinkHint'), { duration: 4000 })
    } catch (e) {
      // 异常：回滚 UI 状态并报错。
      workbuddyLinked.value = !enabled
      message.error(String(e instanceof Error ? e.message : e))
    } finally {
      workbuddyToggling.value = false
    }
  }

  /** 设置 WorkBuddy 数据目录覆盖路径（空串=恢复自动检测）。 */
  async function handleSetWorkBuddyDataDir(): Promise<void> {
    if (workbuddyDataDirSaving.value) return
    workbuddyDataDirSaving.value = true
    try {
      const dir = workbuddyDataDirInput.value.trim()
      const ok = await invoke<boolean>('set_workbuddy_data_dir', { dir: dir || null })
      if (ok) {
        // 重新拉取检测到的路径。
        workbuddyDbPath.value = await invoke<string | null>('get_workbuddy_db_path')
        workbuddyDataDirMissing.value = false
        message.success(t('ui.stats.dataDirOk'))
      } else {
        workbuddyDataDirMissing.value = true
        message.warning(t('ui.stats.dataDirNotFound'))
      }
    } catch (e) {
      message.error(t('ui.stats.dataDirError', { error: e instanceof Error ? e.message : String(e) }))
    } finally {
      workbuddyDataDirSaving.value = false
    }
  }

  // --- handlers：导入 / 删除 ---------------------------------------------

  /** 打开文件选择器导入本地宠物（PNG / WebP）。 */
  async function handleImportPet(): Promise<void> {
    try {
      const selected = await openDialog({
        title: t('ui.pet.import'),
        multiple: false,
        filters: [
          { name: 'Image', extensions: ['png', 'webp'] }
        ]
      })
      if (!selected || Array.isArray(selected)) return

      const filePath = selected
      // 用文件名（去扩展名）作为宠物名。
      const fileName = filePath.split(/[\\/]/).pop() ?? 'imported'
      const displayName = fileName.replace(/\.(png|webp)$/i, '')

      const info = await desktopPetStore.importPet(filePath, displayName)
      message.success(t('ui.pet.importSuccess', { name: info.displayName }))
    } catch (e) {
      message.error(t('ui.pet.importFailed', { error: e instanceof Error ? e.message : String(e) }))
    }
  }

  /** 删除宠物（非内置），删除后 toast 反馈。 */
  async function handleDeletePet(petId: string): Promise<void> {
    try {
      await desktopPetStore.removePet(petId)
      message.success(t('ui.pet.deleteSuccess'))
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      // 内置宠物删除被后端拒绝时给出友好提示。
      if (msg.includes('内置')) {
        message.error(t('ui.pet.deleteBuiltin'))
      } else {
        message.error(msg)
      }
    }
  }

  /**
   * 下载并安装新版本（应用内更新）。
   * updater 插件校验签名后下载安装，完成后自动 relaunch 重启应用。
   * 失败时 toast 提示错误。
   */
  async function handleDownloadUpdate(): Promise<void> {
    if (installing.value || !updateInfo.value.hasUpdate) return
    try {
      await downloadAndInstallUpdate()
    } catch (e) {
      message.error(t('ui.update.downloadFailed', { error: e instanceof Error ? e.message : String(e) }))
    }
  }

  // --- 生命周期 ----------------------------------------------------------

  onMounted(async () => {
    // 启动时按持久化语言同步 i18n（main.ts 已尝试，这里确保管理窗口内一致）。
    locale.value = petSettings.locale

    // 拉取 WorkBuddy 联动真实状态初始化开关。
    try {
      workbuddyLinked.value = await invoke<boolean>('get_workbuddy_link_status')
    } catch (e) {
      console.error('[PetManager] get_workbuddy_link_status failed:', e)
      workbuddyLinked.value = false
    }

    // 检测 WorkBuddy 数据目录（token 统计功能依赖）。
    try {
      const path = await invoke<string | null>('get_workbuddy_db_path')
      workbuddyDbPath.value = path
      workbuddyDataDirMissing.value = !path
    } catch {
      workbuddyDataDirMissing.value = true
    }

    await desktopPetStore.loadLocalPets()

    // 静默检测应用更新（失败不影响主流程；有新版本时顶部显示下载按钮）。
    void checkForAppUpdate()
  })

  // petSettings.locale 被外部改写（如 PetView storage 事件）时同步 i18n。
  watch(
    () => petSettings.locale,
    (value) => {
      if (supportedLocales.includes(value)) {
        locale.value = value
      }
    }
  )

  return {
    // i18n
    t,
    // stores
    petSettings,
    desktopPetStore,
    // options
    languageOptions,
    movementModeOptions,
    scaleMin: SCALE_MIN,
    scaleMax: SCALE_MAX,
    scaleStep: SCALE_STEP,
    // settings collapse
    settingsOpen,
    // tabs
    activeTab,
    // market
    petMarketStore,
    // app update
    updateInfo,
    installing,
    handleDownloadUpdate,
    // detail modal
    detailVisible,
    detailPet,
    // workbuddy link
    workbuddyLinked,
    workbuddyToggling,
    // workbuddy data dir
    workbuddyDbPath,
    workbuddyDataDirInput,
    workbuddyDataDirMissing,
    workbuddyDataDirSaving,
    // handlers
    handleToggleEnabled,
    handleToggleAlwaysOnTop,
    handleSelectPet,
    openLocalDetail,
    handleDetailUse,
    openMarketDetail,
    handleInstallPet,
    handleLanguageChange,
    handleScaleChange,
    handleMovementModeChange,
    handleToggleWorkBuddyLink,
    handleSetWorkBuddyDataDir,
    handleImportPet,
    handleDeletePet,
    // utils
    toLocalAssetUrl
  }
}

export type { LocalPetInfo }
