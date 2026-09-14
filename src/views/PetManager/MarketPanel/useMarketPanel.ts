/**
 * useMarketPanel — 在线宠物市场面板（petdex.dev）的 composable。
 *
 * 职责：
 * - 面板首次挂载时按需拉取第 1 页（store 记录 loaded，切回标签页不重复请求）。
 * - 搜索输入防抖 350ms 后触发查询（排序/过滤/分页都在官网服务端完成）。
 * - 分类切换 / 排序切换 / 翻页 / 手动刷新。
 *
 * 排序与官网一致（默认「最多安装」；搜索时未手动选过排序则自动切「精选」，
 * 由 store 处理）。安装动作上抛给父视图统一处理。
 */
import { computed, onMounted, ref } from 'vue'
import { useDebounceFn } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { usePetMarketStore } from '@/stores/petMarket'
import type { MarketPet, MarketSortKey } from '@/types/petMarket'

/** 搜索防抖时长（输入停止后多久发起请求）。 */
const SEARCH_DEBOUNCE_MS = 350

/** 下拉选项。 */
interface Option {
  label: string
  value: string
}

export function useMarketPanel() {
  const { t } = useI18n()
  const market = usePetMarketStore()

  // 输入框即时值：与 store.keyword（已提交的查询词）分离，输入不被请求往返拖慢。
  const keywordInput = ref(market.keyword)

  /** 分类下拉：空值 = 全部（后端 facets 按数量降序返回，与官网 chip 排列一致）。 */
  const kindOptions = computed<Option[]>(() => [
    { label: t('ui.market.allKinds'), value: '' },
    ...market.kinds.map((entry) => ({
      label: `${entry.kind} (${entry.count})`,
      value: entry.kind
    }))
  ])

  /**
   * 排序下拉：与官网画廊同一组选项、同一顺序。
   * （官网 SORT_LABELS：Curated / Newest / Most liked / Most installed / Alphabetical，
   *   默认 installed。）
   */
  const sortOptions = computed<{ label: string; value: MarketSortKey }[]>(() => [
    { label: t('ui.market.sort.curated'), value: 'curated' },
    { label: t('ui.market.sort.recent'), value: 'recent' },
    { label: t('ui.market.sort.popular'), value: 'popular' },
    { label: t('ui.market.sort.installed'), value: 'installed' },
    { label: t('ui.market.sort.alpha'), value: 'alpha' }
  ])

  /** 安装量缩写：10322 → "10.3k"（官网同款展示风格）。 */
  function formatCount(value: number): string {
    if (value >= 1000) {
      const k = value / 1000
      return `${k >= 100 ? Math.round(k) : Math.round(k * 10) / 10}k`
    }
    return String(value)
  }

  /** 图鉴编号：官网规则为不足三位补零（No.069）。 */
  function dexLabel(pet: MarketPet): string {
    return pet.dexNumber != null ? String(pet.dexNumber).padStart(3, '0') : ''
  }

  const runSearch = useDebounceFn((value: string) => {
    void market.search(value)
  }, SEARCH_DEBOUNCE_MS)

  function handleKeywordInput(value: string): void {
    keywordInput.value = value
    runSearch(value)
  }

  function handleKindChange(value: string): void {
    void market.setKind(value)
  }

  function handleSortChange(value: MarketSortKey): void {
    void market.setSort(value)
  }

  function handlePageChange(page: number): void {
    void market.goToPage(page)
  }

  function handleRefresh(): void {
    void market.refresh()
  }

  onMounted(() => {
    // 已加载过就直接复用 store 里的结果（切标签页回来不重新请求）。
    if (!market.loaded && !market.loading) {
      void market.load()
    }
  })

  return {
    t,
    market,
    keywordInput,
    kindOptions,
    sortOptions,
    formatCount,
    dexLabel,
    handleKeywordInput,
    handleKindChange,
    handleSortChange,
    handlePageChange,
    handleRefresh
  }
}
