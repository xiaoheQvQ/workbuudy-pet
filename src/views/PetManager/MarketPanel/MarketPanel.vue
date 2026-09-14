<script setup lang="ts">
/**
 * MarketPanel — 在线宠物市场面板（数据来自 petdex.dev）。
 *
 * 浏览 / 搜索 / 排序 / 分类筛选 / 分页 / 一键安装；点击卡片查看详情（复用详情弹窗）。
 * 排序选项与官网画廊一致（默认「最多安装」）。安装与详情事件上抛给父视图统一处理。
 */
import PetThumb from '../PetThumb/PetThumb.vue'
import { useMarketPanel } from './useMarketPanel'
import type { MarketPet } from '@/types/petMarket'

const emit = defineEmits<{
  (event: 'open', pet: MarketPet): void
  (event: 'install', slug: string): void
}>()

const {
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
} = useMarketPanel()
</script>

<template>
  <div class="market-panel">
    <!-- 工具条：搜索 + 排序 + 分类 + 刷新 -->
    <div class="market-toolbar">
      <n-input
        :value="keywordInput"
        :placeholder="t('ui.market.searchPlaceholder')"
        size="small"
        clearable
        class="market-toolbar__search"
        @update:value="handleKeywordInput"
      />
      <n-select
        :value="market.sort"
        :options="sortOptions"
        size="small"
        class="market-toolbar__sort"
        @update:value="handleSortChange"
      />
      <n-select
        :value="market.kind"
        :options="kindOptions"
        size="small"
        class="market-toolbar__kind"
        @update:value="handleKindChange"
      />
      <n-button
        size="small"
        quaternary
        :loading="market.loading"
        @click="handleRefresh"
      >
        {{ t('ui.market.refresh') }}
      </n-button>
    </div>

    <!-- 内容区（独立滚动） -->
    <div class="market-body">
      <!-- 拉取失败（多为网络不可达） -->
      <div
        v-if="market.error"
        class="pm-empty market-empty"
      >
        <p class="market-empty__text">{{ t('ui.market.error', { error: market.error }) }}</p>
        <n-button
          size="small"
          @click="handleRefresh"
        >
          {{ t('ui.market.retry') }}
        </n-button>
      </div>

      <!-- 首次加载 -->
      <div
        v-else-if="market.loading && market.items.length === 0"
        class="pm-empty"
      >
        {{ t('ui.pet.loading') }}
      </div>

      <!-- 无结果 -->
      <div
        v-else-if="market.items.length === 0"
        class="pm-empty"
      >
        {{ t('ui.market.empty') }}
      </div>

      <div
        v-else
        class="pm-grid"
      >
        <article
          v-for="pet in market.items"
          :key="pet.slug"
          class="pm-card market-card"
          @click="emit('open', pet)"
        >
          <div class="pm-card__thumb">
            <!-- 行数交给 PetThumb 按图片真实尺寸探测：上游标注的版本号不可靠 -->
            <PetThumb
              :src="pet.spritesheetUrl"
              :row="0"
              :col="0"
            />
            <span
              v-if="market.isInstalled(pet.slug)"
              class="pm-card__tag pm-card__tag--done"
            >{{ t('ui.market.installed') }}</span>
            <span
              v-else-if="pet.featured"
              class="pm-card__tag"
            >★ {{ t('ui.market.sort.curated') }}</span>
          </div>

          <div class="pm-card__body">
            <h3 class="pm-card__name">{{ pet.displayName }}</h3>
            <span class="pm-card__kind">
              {{ pet.submittedBy ? `by ${pet.submittedBy}` : pet.kind }}
            </span>
            <!-- 图鉴编号 + 安装量（与官网卡片同款信息） -->
            <div
              v-if="pet.dexNumber != null || pet.installCount"
              class="market-card__meta"
            >
              <span
                v-if="pet.dexNumber != null"
                class="market-card__dex"
              >No.{{ dexLabel(pet) }}</span>
              <span
                v-if="pet.installCount"
                class="market-card__stat"
              >↓ {{ formatCount(pet.installCount) }}</span>
            </div>
          </div>

          <n-button
            class="market-card__action"
            size="tiny"
            :type="market.isInstalled(pet.slug) ? 'default' : 'primary'"
            :ghost="!market.isInstalled(pet.slug)"
            :loading="market.isInstalling(pet.slug)"
            :disabled="market.isInstalled(pet.slug)"
            @click.stop="emit('install', pet.slug)"
          >
            {{ market.isInstalled(pet.slug) ? t('ui.market.installed') : t('ui.market.install') }}
          </n-button>
        </article>
      </div>
    </div>

    <!-- 分页 -->
    <div
      v-if="market.pageCount > 1"
      class="market-pager"
    >
      <n-pagination
        :page="market.page"
        :page-count="market.pageCount"
        size="small"
        @update:page="handlePageChange"
      />
    </div>
  </div>
</template>
<style scoped src="./styles.css"></style>
<!-- 卡片/网格/空态样式与本地列表共用（scoped 样式无法从父组件穿透，需在此引入） -->
<style scoped src="../cards.css"></style>
