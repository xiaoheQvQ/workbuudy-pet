<script setup lang="ts">
/**
 * PetManager 管理窗口主视图：宠物选择/查看/本地导入/在线市场。
 * 按 taste-skills redesign 规范设计。
 */
import { usePetManager } from './index'
import PetThumb from './PetThumb/PetThumb.vue'
import PetDetailModal from './PetDetailModal/PetDetailModal.vue'
import MarketPanel from './MarketPanel/MarketPanel.vue'
import TodoPanel from './TodoPanel/TodoPanel.vue'
import ProcessPanel from './ProcessPanel/ProcessPanel.vue'

const {
  t,
  petSettings,
  desktopPetStore,
  languageOptions,
  movementModeOptions,
  scaleMin,
  scaleMax,
  scaleStep,
  settingsOpen,
  activeTab,
  upcoming7Count,
  petMarketStore,
  updateInfo,
  installing,
  handleDownloadUpdate,
  detailVisible,
  detailPet,
  workbuddyLinked,
  workbuddyDbPath,
  workbuddyDataDirInput,
  workbuddyDataDirSaving,
  aiModels,
  aiModelError,
  aiModelOptions,
  aiTopicOptions,
  aiIntervalOptions,
  handleToggleAiTalk,
  handleAiModelChange,
  handleAiTopicChange,
  handleAiIntervalChange,
  handleToggleEnabled,
  openLocalDetail,
  handleDetailUse,
  openMarketDetail,
  handleInstallPet,
  handleDeletePet,
  handleLanguageChange,
  handleScaleChange,
  handleMovementModeChange,
  handleSetWorkBuddyDataDir,
  handleImportPet,
  toLocalAssetUrl
} = usePetManager()
</script>

<template>
  <main class="pet-manager">
    <!-- 顶部：标题 + 更新按钮 + 总开关 -->
    <header class="pm-header">
      <div class="pm-header__brand">
        <h1 class="pm-title">桌面宠物</h1>
        <p class="pm-subtitle">选一只陪伴你的小家伙</p>
      </div>
      <div class="pm-header__actions">
        <!-- 有新版本时显示下载按钮（应用内更新，安装后自动重启） -->
        <button
          v-if="updateInfo.hasUpdate"
          type="button"
          class="pm-update-btn"
          :disabled="installing"
          :title="t('ui.update.tooltip', { version: updateInfo.latestVersion })"
          @click="handleDownloadUpdate"
        >
          <span class="pm-update-btn__dot" />
          {{ installing ? t('ui.update.installing') : t('ui.update.available', { version: updateInfo.latestVersion }) }}
        </button>
        <label class="pm-power">
          <span class="pm-power__label">{{ petSettings.enabled ? '运行中' : '已关闭' }}</span>
          <n-switch
            :value="petSettings.enabled"
            :round="false"
            @update:value="handleToggleEnabled"
          />
        </label>
      </div>
    </header>

    <!-- 未开启引导条 -->
    <div
      v-if="!petSettings.enabled"
      class="pm-guide"
      role="status"
    >
      <div class="pm-guide__dot" />
      <span class="pm-guide__text">宠物已关闭，打开右上角开关即可在桌面显示。先选一只喜欢的吧。</span>
    </div>

    <!-- 运行中状态条 -->
    <div
      v-if="petSettings.enabled"
      class="pm-status-bar"
    >
      <div class="pm-status-bar__item">
        <span class="pm-status-bar__key">状态</span>
        <span class="pm-status-bar__val pm-status-bar__val--live">
          <i class="pm-live-dot" />运行中
        </span>
      </div>
      <div class="pm-status-bar__divider" />
      <div class="pm-status-bar__item">
        <span class="pm-status-bar__key">当前宠物</span>
        <span class="pm-status-bar__val">{{ desktopPetStore.activePet?.displayName ?? '未选择' }}</span>
      </div>
    </div>

    <!-- 设置区：语言 / 缩放 / WorkBuddy 联动（可折叠，默认收起，释放垂直空间给宠物列表） -->
    <section class="pm-settings">
      <button
        type="button"
        class="pm-settings__toggle"
        :class="{ 'pm-settings__toggle--open': settingsOpen }"
        @click="settingsOpen = !settingsOpen"
      >
        <span class="pm-settings__toggle-text">{{ t('ui.settings.title') }}</span>
        <span class="pm-settings__toggle-summary">
          <span class="pm-settings__chip">{{ petSettings.scale }}%</span>
          <span
            class="pm-settings__chip pm-settings__chip--status"
            :class="{ 'pm-settings__chip--on': workbuddyLinked }"
          >{{ workbuddyLinked ? t('ui.workbuddy.linked') : t('ui.workbuddy.unlinked') }}</span>
          <!-- 收起状态下也能看到 AI 搭话是否已开启 -->
          <span
            class="pm-settings__chip pm-settings__chip--status"
            :class="{ 'pm-settings__chip--on': petSettings.aiTalkEnabled }"
          >{{ t('ui.ai.title') }}</span>
        </span>
        <span class="pm-settings__chevron">⌄</span>
      </button>

      <div
        v-show="settingsOpen"
        class="pm-settings__body"
      >
        <!-- 语言 -->
        <div class="pm-setting">
          <div class="pm-setting__head">
            <span class="pm-setting__label">{{ t('ui.settings.language') }}</span>
          </div>
          <n-select
            :value="petSettings.locale"
            :options="languageOptions"
            class="pm-setting__control pm-setting__control--lang"
            @update:value="handleLanguageChange"
          />
        </div>

        <!-- 漫游模式：自由走步 / 固定位置 -->
        <div class="pm-setting">
          <div class="pm-setting__head">
            <span class="pm-setting__label">{{ t('ui.pet.movementMode') }}</span>
          </div>
          <n-select
            :value="petSettings.movementMode"
            :options="movementModeOptions"
            class="pm-setting__control pm-setting__control--lang"
            @update:value="handleMovementModeChange"
          />
        </div>

        <!-- 缩放 -->
        <div class="pm-setting">
          <div class="pm-setting__head">
            <span class="pm-setting__label">{{ t('ui.pet.scale') }}</span>
            <span class="pm-setting__value">{{ petSettings.scale }}%</span>
          </div>
          <n-slider
            :value="petSettings.scale"
            :min="scaleMin"
            :max="scaleMax"
            :step="scaleStep"
            :tooltip="false"
            class="pm-setting__control pm-setting__control--scale"
            @update:value="handleScaleChange"
          />
        </div>

        <!-- WorkBuddy 数据目录（token 统计用，自动检测，异常时可手动填） -->
        <div class="pm-setting pm-setting--wide">
          <div class="pm-setting__head">
            <span class="pm-setting__label">{{ t('ui.stats.dataDir') }}</span>
            <span
              v-if="workbuddyDbPath"
              class="pm-setting__value pm-setting__value--ok"
            >✓ {{ t('ui.stats.autoDetected') }}</span>
            <span
              v-else
              class="pm-setting__value pm-setting__value--warn"
            >⚠ {{ t('ui.stats.notDetected') }}</span>
          </div>
          <p
            v-if="workbuddyDbPath"
            class="pm-setting__hint pm-setting__hint--mono"
            :title="workbuddyDbPath"
          >{{ workbuddyDbPath }}</p>
          <div class="pm-datadir-input">
            <n-input
              v-model:value="workbuddyDataDirInput"
              :placeholder="t('ui.stats.dataDirPlaceholder')"
              size="small"
              clearable
            />
            <n-button
              size="small"
              :loading="workbuddyDataDirSaving"
              @click="handleSetWorkBuddyDataDir"
            >{{ t('ui.stats.dataDirApply') }}</n-button>
          </div>
        </div>

        <!-- AI 搭话：复用 WorkBuddy 已配置的免费模型生成宠物台词 -->
        <div class="pm-setting pm-setting--wide">
          <div class="pm-setting__head">
            <span class="pm-setting__label">{{ t('ui.ai.title') }}</span>
            <n-switch
              :value="petSettings.aiTalkEnabled"
              :round="false"
              size="small"
              @update:value="handleToggleAiTalk"
            />
          </div>
          <p class="pm-setting__hint">{{ t('ui.ai.hint') }}</p>

          <div class="pm-ai-grid">
            <label class="pm-ai-field">
              <span class="pm-ai-field__label">{{ t('ui.ai.model') }}</span>
              <n-select
                :value="petSettings.aiModelId ?? ''"
                :options="aiModelOptions"
                size="small"
                @update:value="handleAiModelChange"
              />
            </label>
            <label class="pm-ai-field">
              <span class="pm-ai-field__label">{{ t('ui.ai.topic') }}</span>
              <n-select
                :value="petSettings.aiTopic"
                :options="aiTopicOptions"
                size="small"
                @update:value="handleAiTopicChange"
              />
            </label>
            <label class="pm-ai-field">
              <span class="pm-ai-field__label">{{ t('ui.ai.interval') }}</span>
              <n-select
                :value="String(petSettings.aiIntervalMinutes)"
                :options="aiIntervalOptions"
                size="small"
                @update:value="handleAiIntervalChange"
              />
            </label>
          </div>

          <p
            v-if="aiModelError"
            class="pm-setting__hint pm-setting__hint--warn"
          >{{ aiModelError }}</p>
          <p
            v-else-if="aiModels.length === 0"
            class="pm-setting__hint pm-setting__hint--warn"
          >{{ t('ui.ai.modelEmpty') }}</p>
          <p
            v-else-if="petSettings.aiTopic === 'news'"
            class="pm-setting__hint"
          >{{ t('ui.ai.newsHint') }}</p>
        </div>
      </div>
    </section>

    <!-- 列表区：我的宠物 / 在线市场 -->
    <section class="pm-panel">
      <!-- 标签页切换 -->
      <div class="pm-tabs">
        <button
          type="button"
          class="pm-tab"
          :class="{ 'pm-tab--active': activeTab === 'local' }"
          @click="activeTab = 'local'"
        >
          {{ t('ui.pet.myPets') }}
          <span class="pm-tab__count">{{ desktopPetStore.localPets.length }}</span>
        </button>
        <button
          type="button"
          class="pm-tab"
          :class="{ 'pm-tab--active': activeTab === 'market' }"
          @click="activeTab = 'market'"
        >
          {{ t('ui.market.title') }}
          <span
            v-if="petMarketStore.loaded"
            class="pm-tab__count"
          >{{ petMarketStore.total }}</span>
        </button>
        <button
          type="button"
          class="pm-tab"
          :class="{ 'pm-tab--active': activeTab === 'todo' }"
          @click="activeTab = 'todo'"
        >
          {{ t('ui.todo.tab') }}
          <span class="pm-tab__count">{{ upcoming7Count }}</span>
        </button>
        <button
          type="button"
          class="pm-tab"
          :class="{ 'pm-tab--active': activeTab === 'process' }"
          @click="activeTab = 'process'"
        >
          {{ t('ui.process.tab') }}
        </button>
      </div>

      <!-- 我的宠物 -->
      <template v-if="activeTab === 'local'">
      <!-- 列表头部：导入按钮 -->
      <div class="pm-list__header">
        <span class="pm-list__count">{{ desktopPetStore.localPets.length }} 只宠物</span>
        <n-button
          size="small"
          type="primary"
          ghost
          @click="handleImportPet"
        >
          + {{ t('ui.pet.import') }}
        </n-button>
      </div>
      <div class="pm-list">
        <div
          v-if="desktopPetStore.localPets.length === 0"
          class="pm-empty"
        >
          还没有导入任何宠物，点击右上角「导入宠物」添加一只吧
        </div>
        <div
          v-else
          class="pm-grid"
        >
          <article
            v-for="pet in desktopPetStore.localPets"
            :key="pet.id"
            class="pm-card"
            :class="{ 'pm-card--active': pet.id === desktopPetStore.activePetId }"
            @click="openLocalDetail(pet)"
          >
            <div class="pm-card__thumb">
              <PetThumb
                :src="toLocalAssetUrl(pet.spritesheetPath)"
                :row="0"
                :col="0"
              />
              <span
                v-if="pet.source === 'builtin'"
                class="pm-card__tag"
              >内置</span>
              <span
                v-else-if="pet.source === 'downloaded'"
                class="pm-card__tag pm-card__tag--done"
              >{{ t('ui.pet.tag.market') }}</span>
              <span
                v-else-if="pet.source === 'uploaded'"
                class="pm-card__tag pm-card__tag--done"
              >已上传</span>
              <span
                v-if="pet.id === desktopPetStore.activePetId"
                class="pm-card__tag pm-card__tag--active"
              >使用中</span>
            </div>
            <div class="pm-card__body">
              <h3 class="pm-card__name">{{ pet.displayName }}</h3>
              <span
                v-if="pet.kind"
                class="pm-card__kind"
              >{{ pet.kind }}</span>
            </div>
          </article>
        </div>
      </div>
      </template>

      <!-- 在线市场（petdex.dev） -->
      <MarketPanel
        v-else-if="activeTab === 'market'"
        @open="openMarketDetail"
        @install="handleInstallPet"
      />

      <!-- 待办日历 -->
      <TodoPanel v-else-if="activeTab === 'todo'" />

      <!-- 进程管理 -->
      <ProcessPanel v-else />
    </section>

    <!-- 详情弹窗 -->
    <PetDetailModal
      :visible="detailVisible"
      :pet="detailPet"
      :is-active="detailPet ? detailPet.id === desktopPetStore.activePetId : false"
      @update:visible="detailVisible = $event"
      @use="handleDetailUse"
      @delete="handleDeletePet"
      @install="handleInstallPet"
    />
  </main>
</template>
<style scoped src="./index.css"></style>
<!-- 卡片/网格/空态样式为本地列表与在线市场共用；scoped 样式命不中子组件内部元素，
     故 MarketPanel 也自行引入同一份（见 cards.css 头部说明）。 -->
<style scoped src="./cards.css"></style>
