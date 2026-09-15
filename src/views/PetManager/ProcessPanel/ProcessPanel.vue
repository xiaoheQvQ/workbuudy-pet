<script setup lang="ts">
/**
 * ProcessPanel — 进程管理面板（管理窗口「进程管理」标签页）。
 *
 * 面向 tasklist / netstat 的使用习惯：
 *   - 顶部搜索框按进程名、PID、端口过滤，并支持 kill / netstat / ps / tasklist /
 *     taskkill / process 命令式关键词（netstat 默认按端口数排序并只看占用端口的进程，
 *     其余关键词默认按内存占用排序）；
 *   - 表头点击切换排序维度（内存 / CPU / 名称 / 端口数）与升降序；
 *   - 行内「结束」/ 详情区「强制结束」，支持勾选后批量结束；
 *   - 深色自包含面板（color-scheme: dark），控件全部自定义，不依赖 naive-ui。
 */
import { useProcessPanel } from './useProcessPanel'

const {
  t,
  sorted,
  loading,
  error,
  total,
  visibleCount,
  portCount,
  portsAvailable,
  elapsedMs,
  lastUpdated,
  query,
  keyword,
  keywordHint,
  keywordChips,
  setSort,
  sortIndicator,
  applyKeyword,
  onlyWithPorts,
  includePorts,
  autoRefresh,
  refreshInterval,
  refreshIntervalOptions,
  refresh,
  selectedPids,
  selectedSet,
  allSelected,
  toggleSelect,
  toggleSelectAll,
  clearSelection,
  expandedPid,
  toggleExpand,
  killVisible,
  killTargets,
  killForce,
  killBusy,
  requestKillOne,
  requestKillSelected,
  closeKill,
  confirmKill,
  handleCopyPid,
  handleCopyCmd,
  formatMemory,
  formatCpu,
  formatTime,
  portPreviewLimit,
  killListLimit
} = useProcessPanel()
</script>

<template>
  <section class="proc-app">
    <!-- ============ 工具条 ============ -->
    <div class="proc-bar">
      <label class="proc-search">
        <svg
          class="proc-search__icon"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <circle cx="11" cy="11" r="7" />
          <path d="m20 20-3.6-3.6" />
        </svg>
        <input
          v-model="query"
          class="proc-search__input"
          type="text"
          spellcheck="false"
          autocomplete="off"
          :placeholder="t('ui.process.searchPlaceholder')"
        >
        <button
          v-if="query"
          type="button"
          class="proc-search__clear"
          :title="t('ui.process.clearQuery')"
          @click="query = ''"
        >×</button>
      </label>

      <button
        type="button"
        class="pbtn pbtn--accent"
        :disabled="loading"
        @click="refresh"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M21 12a9 9 0 1 1-3-6.7M21 4v5h-5" />
        </svg>
        {{ loading ? t('ui.process.loading') : t('ui.process.refresh') }}
      </button>

      <label class="proc-toggle">
        <input
          v-model="autoRefresh"
          type="checkbox"
        >
        <span>{{ t('ui.process.autoRefresh') }}</span>
      </label>
      <select
        v-if="autoRefresh"
        v-model="refreshInterval"
        class="pselect proc-interval"
      >
        <option
          v-for="ms in refreshIntervalOptions"
          :key="ms"
          :value="ms"
        >{{ ms / 1000 }}s</option>
      </select>
    </div>

    <!-- ============ 关键词胶囊 ============ -->
    <div class="proc-keywords">
      <span class="proc-keywords__label">{{ t('ui.process.keywords') }}</span>
      <button
        v-for="chip in keywordChips"
        :key="chip"
        type="button"
        class="pkw"
        :class="{ 'pkw--active': keyword === chip, 'pkw--port': chip === 'netstat' }"
        @click="applyKeyword(chip)"
      >{{ chip }}</button>
      <span
        v-if="keywordHint"
        class="proc-keywords__hint"
      >{{ keywordHint }}</span>
    </div>

    <!-- ============ 状态行 ============ -->
    <div class="proc-status">
      <span class="proc-status__item">{{ t('ui.process.total', { count: total }) }}</span>
      <span class="proc-status__sep">·</span>
      <span class="proc-status__item">{{ t('ui.process.shown', { count: visibleCount }) }}</span>
      <span class="proc-status__sep">·</span>
      <span class="proc-status__item">{{ t('ui.process.ports', { count: portCount }) }}</span>
      <span class="proc-status__sep">·</span>
      <span class="proc-status__item proc-status__item--mono">{{ t('ui.process.elapsed', { ms: elapsedMs }) }}</span>
      <span class="proc-status__sep">·</span>
      <span class="proc-status__item proc-status__item--mono">{{ t('ui.process.updated', { time: formatTime(lastUpdated) }) }}</span>

      <span class="proc-status__spacer" />

      <label class="proc-toggle">
        <input
          v-model="onlyWithPorts"
          type="checkbox"
        >
        <span>{{ t('ui.process.onlyWithPorts') }}</span>
      </label>
      <label class="proc-toggle">
        <input
          v-model="includePorts"
          type="checkbox"
        >
        <span>{{ t('ui.process.includePorts') }}</span>
      </label>

      <template v-if="selectedPids.length > 0">
        <span class="proc-status__sel">{{ t('ui.process.selected', { count: selectedPids.length }) }}</span>
        <button
          type="button"
          class="pbtn pbtn--danger pbtn--tiny"
          @click="requestKillSelected"
        >{{ t('ui.process.killSelected') }}</button>
        <button
          type="button"
          class="pbtn pbtn--ghost pbtn--tiny"
          @click="clearSelection"
        >{{ t('ui.process.clearSelection') }}</button>
      </template>
    </div>

    <!-- ============ 异常提示 ============ -->
    <p
      v-if="error"
      class="proc-note proc-note--error"
    >{{ t('ui.process.loadFailed', { error }) }}</p>
    <p
      v-else-if="includePorts && !portsAvailable"
      class="proc-note proc-note--warn"
    >{{ t('ui.process.portsUnavailable') }}</p>

    <!-- ============ 表头 ============ -->
    <div class="proc-head">
      <span class="proc-cell proc-cell--check">
        <input
          type="checkbox"
          :checked="allSelected"
          :disabled="sorted.length === 0"
          @change="toggleSelectAll"
        >
      </span>
      <button
        type="button"
        class="proc-cell proc-cell--name proc-sort"
        @click="setSort('name')"
      >
        {{ t('ui.process.col.name') }}<i>{{ sortIndicator('name') }}</i>
      </button>
      <span class="proc-cell proc-cell--pid">{{ t('ui.process.col.pid') }}</span>
      <button
        type="button"
        class="proc-cell proc-cell--mem proc-sort"
        @click="setSort('memory')"
      >
        {{ t('ui.process.col.memory') }}<i>{{ sortIndicator('memory') }}</i>
      </button>
      <button
        type="button"
        class="proc-cell proc-cell--cpu proc-sort"
        @click="setSort('cpu')"
      >
        {{ t('ui.process.col.cpu') }}<i>{{ sortIndicator('cpu') }}</i>
      </button>
      <button
        type="button"
        class="proc-cell proc-cell--ports proc-sort"
        :class="{ 'proc-sort--hot': keyword === 'netstat' }"
        @click="setSort('ports')"
      >
        {{ t('ui.process.col.ports') }}<i>{{ sortIndicator('ports') }}</i>
      </button>
      <span class="proc-cell proc-cell--act">{{ t('ui.process.col.actions') }}</span>
    </div>

    <!-- ============ 列表 ============ -->
    <div class="proc-list">
      <div
        v-if="sorted.length === 0"
        class="proc-empty"
      >{{ loading ? t('ui.process.loading') : t('ui.process.empty') }}</div>

      <template
        v-for="item in sorted"
        :key="item.pid"
      >
        <div
          class="proc-row"
          :class="{ 'proc-row--self': item.isSelf, 'proc-row--open': expandedPid === item.pid }"
        >
          <span class="proc-cell proc-cell--check">
            <input
              type="checkbox"
              :checked="selectedSet.has(item.pid)"
              :disabled="item.isSelf"
              @change="toggleSelect(item.pid)"
            >
          </span>

          <div
            class="proc-cell proc-cell--name"
            @click="toggleExpand(item.pid)"
          >
            <span
              class="proc-name"
              :title="item.exe ?? item.name"
            >{{ item.name }}</span>
            <span
              v-if="item.isSelf"
              class="proc-tag"
            >{{ t('ui.process.self') }}</span>
          </div>

          <span class="proc-cell proc-cell--pid proc-mono">{{ item.pid }}</span>
          <span class="proc-cell proc-cell--mem proc-mono">{{ formatMemory(item.memory) }}</span>
          <span
            class="proc-cell proc-cell--cpu proc-mono"
            :class="{ 'proc-cpu--hot': item.cpu >= 20 }"
          >{{ formatCpu(item.cpu) }}</span>

          <div class="proc-cell proc-cell--ports">
            <span
              v-if="item.ports.length === 0"
              class="proc-ports__none"
            >—</span>
            <template v-else>
              <button
                v-for="port in item.ports.slice(0, portPreviewLimit)"
                :key="port"
                type="button"
                class="proc-port"
                :title="t('ui.process.filterPort', { port })"
                @click="query = String(port)"
              >{{ port }}</button>
              <span
                v-if="item.ports.length > portPreviewLimit"
                class="proc-port proc-port--more"
                :title="item.ports.join(', ')"
              >+{{ item.ports.length - portPreviewLimit }}</span>
            </template>
          </div>

          <div class="proc-cell proc-cell--act">
            <button
              type="button"
              class="pbtn pbtn--tiny pbtn--danger-ghost"
              :disabled="item.isSelf"
              :title="item.isSelf ? t('ui.process.selfBlocked') : t('ui.process.kill')"
              @click="requestKillOne(item)"
            >{{ t('ui.process.kill') }}</button>
          </div>
        </div>

        <!-- 行详情：路径 / 命令行 / 复制 / 强杀 -->
        <div
          v-if="expandedPid === item.pid"
          class="proc-detail"
        >
          <div class="proc-detail__row">
            <span class="proc-detail__key">{{ t('ui.process.detail.path') }}</span>
            <span class="proc-detail__val">{{ item.exe ?? '—' }}</span>
          </div>
          <div class="proc-detail__row">
            <span class="proc-detail__key">{{ t('ui.process.detail.cmd') }}</span>
            <span class="proc-detail__val">{{ item.cmd || '—' }}</span>
          </div>
          <div class="proc-detail__actions">
            <button
              type="button"
              class="pbtn pbtn--tiny pbtn--ghost"
              @click="handleCopyPid(item)"
            >{{ t('ui.process.copyPid') }}</button>
            <button
              type="button"
              class="pbtn pbtn--tiny pbtn--ghost"
              :disabled="!item.cmd"
              @click="handleCopyCmd(item)"
            >{{ t('ui.process.copyCmd') }}</button>
            <button
              type="button"
              class="pbtn pbtn--tiny pbtn--danger"
              :disabled="item.isSelf"
              @click="requestKillOne(item, true)"
            >{{ t('ui.process.forceKill') }}</button>
          </div>
        </div>
      </template>
    </div>

    <!-- ============ 结束确认弹层 ============ -->
    <Teleport to="body">
      <div
        v-if="killVisible"
        class="pmask"
        @click.self="closeKill"
      >
        <div class="pdialog">
          <div class="pdialog__head">
            <h3 class="pdialog__title">{{ t('ui.process.confirmTitle') }}</h3>
            <button
              type="button"
              class="pdialog__close"
              @click="closeKill"
            >×</button>
          </div>

          <div class="pdialog__body">
            <p class="pdialog__text">{{ t('ui.process.confirmBody', { count: killTargets.length }) }}</p>
            <ul class="pdialog__list">
              <li
                v-for="item in killTargets.slice(0, killListLimit)"
                :key="item.pid"
              >
                <b>{{ item.name }}</b>
                <span class="proc-mono">PID {{ item.pid }}</span>
              </li>
              <li
                v-if="killTargets.length > killListLimit"
                class="pdialog__more"
              >{{ t('ui.process.confirmMore', { count: killTargets.length - killListLimit }) }}</li>
            </ul>

            <label class="pdialog__check">
              <input
                v-model="killForce"
                type="checkbox"
              >
              <span>{{ t('ui.process.forceHint') }}</span>
            </label>
            <p class="pdialog__tip">{{ t('ui.process.confirmTip') }}</p>
          </div>

          <div class="pdialog__foot">
            <button
              type="button"
              class="pbtn pbtn--ghost"
              :disabled="killBusy"
              @click="closeKill"
            >{{ t('ui.common.cancel') }}</button>
            <button
              type="button"
              class="pbtn pbtn--danger"
              :disabled="killBusy"
              @click="confirmKill"
            >{{ killBusy ? t('ui.process.killing') : (killForce ? t('ui.process.forceKill') : t('ui.process.kill')) }}</button>
          </div>
        </div>
      </div>
    </Teleport>
  </section>
</template>

<style scoped src="./ProcessPanel.css"></style>
