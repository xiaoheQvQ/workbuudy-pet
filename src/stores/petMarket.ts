/**
 * 在线宠物市场 store（petdex.dev）。
 *
 * 数据与官网画廊同源（/api/pets/search）：排序、过滤、分页都在官网服务端完成，
 * 前端只维护「筛选条件 + 当前页 + 当前结果」。
 *
 * 排序行为对齐官网 pet-gallery.tsx：
 * - 默认 `installed`（最多安装）；
 * - 输入关键词时，若用户没有手动选过排序，自动切换为 `curated`（精选/相关性），
 *   清空关键词后切回 `installed`。
 *
 * 安装成功后自动刷新本地宠物列表并设为当前宠物（与本地导入行为一致）。
 */
import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { fetchMarketPets, installMarketPet } from '@/services/petMarket'
import type { LocalPetInfo } from '@/types/desktopPet'
import type { MarketPet, MarketSortKey } from '@/types/petMarket'
import { useDesktopPetStore } from './desktopPet'

/** 默认每页条数（与官网画廊 PAGE_SIZE 一致）。 */
const DEFAULT_PAGE_SIZE = 24

export const usePetMarketStore = defineStore('petMarket', () => {
  const desktopPetStore = useDesktopPetStore()

  // --- 结果状态 ----------------------------------------------------------
  const items = ref<MarketPet[]>([])
  const kinds = ref<{ kind: string; count: number }[]>([])
  const total = ref(0)
  const loaded = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)

  // --- 查询条件 ----------------------------------------------------------
  const keyword = ref('')
  const kind = ref('')
  /** 排序键：与官网一致，默认「最多安装」。 */
  const sort = ref<MarketSortKey>('installed')
  /** 用户是否手动选过排序（选过后搜索时不再自动切换 curated/installed）。 */
  const sortTouched = ref(false)
  const page = ref(1)
  const pageSize = ref(DEFAULT_PAGE_SIZE)

  /** 正在安装的 slug（卡片显示 loading 并禁用按钮，避免重复下载）。 */
  const installing = ref<string[]>([])

  const pageCount = computed(() =>
    Math.max(1, Math.ceil(total.value / Math.max(1, pageSize.value)))
  )

  /** 本地已安装的宠物 id 集合（市场卡片据此显示「已安装」）。 */
  const installedSlugs = computed(
    () => new Set(desktopPetStore.localPets.map((pet) => pet.id))
  )

  /** 该 slug 是否已安装（大小写不敏感：上游 slug 可能含大写，本地目录名统一小写）。 */
  function isInstalled(slug: string): boolean {
    return installedSlugs.value.has(slug.trim().toLowerCase())
  }

  /** 按当前条件拉取一页数据。 */
  async function load(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      const result = await fetchMarketPets({
        query: keyword.value,
        kind: kind.value,
        sort: sort.value,
        page: page.value,
        pageSize: pageSize.value
      })
      items.value = result.items
      total.value = result.total
      kinds.value = result.kinds
      // 后端会钳制 pageSize / page，回写以保持分页器与数据一致。
      pageSize.value = result.pageSize
      page.value = result.page
      loaded.value = true
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      items.value = []
      total.value = 0
    } finally {
      loading.value = false
    }
  }

  /**
   * 关键词搜索（回到第 1 页）。
   * 对齐官网行为：用户没手动选过排序时，有关键词 → curated，无关键词 → installed。
   */
  async function search(value: string): Promise<void> {
    keyword.value = value
    if (!sortTouched.value) {
      sort.value = value.trim() ? 'curated' : 'installed'
    }
    page.value = 1
    await load()
  }

  /** 切换分类过滤（回到第 1 页）。 */
  async function setKind(value: string): Promise<void> {
    kind.value = value
    page.value = 1
    await load()
  }

  /** 切换排序（回到第 1 页），并记住用户手动选择。 */
  async function setSort(value: MarketSortKey): Promise<void> {
    sortTouched.value = true
    sort.value = value
    page.value = 1
    await load()
  }

  /** 翻页（越界时钳制到有效范围）。 */
  async function goToPage(value: number): Promise<void> {
    const next = Math.min(Math.max(1, Math.trunc(value)), pageCount.value)
    if (next === page.value) return
    page.value = next
    await load()
  }

  /** 重新拉取当前页。 */
  async function refresh(): Promise<void> {
    await load()
  }

  /**
   * 安装市场宠物：后端下载并落盘 → 刷新本地列表 → 设为当前宠物。
   * 返回落地后的本地宠物信息；失败时抛错，由调用方转成用户提示。
   */
  async function install(slug: string): Promise<LocalPetInfo> {
    if (installing.value.includes(slug)) {
      throw new Error('该宠物正在安装中')
    }
    installing.value = [...installing.value, slug]
    try {
      const info = await installMarketPet(slug)
      await desktopPetStore.loadLocalPets()
      await desktopPetStore.setActivePet(info.id)
      return info
    } finally {
      installing.value = installing.value.filter((item) => item !== slug)
    }
  }

  function isInstalling(slug: string): boolean {
    return installing.value.includes(slug)
  }

  return {
    // 结果
    items,
    kinds,
    total,
    loaded,
    loading,
    error,
    // 查询条件
    keyword,
    kind,
    sort,
    sortTouched,
    page,
    pageSize,
    pageCount,
    // 安装
    installing,
    installedSlugs,
    // actions
    load,
    search,
    setKind,
    setSort,
    goToPage,
    refresh,
    install,
    isInstalling,
    isInstalled
  }
})
