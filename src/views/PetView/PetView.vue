<script setup lang="ts">
/** PetView 组件：桌面宠物视图，承载宠物宿主、上下文与动作菜单（逻辑见 usePetView.ts） */
import { useI18n } from 'vue-i18n'
import { usePetView } from './usePetView'

const {
  hostRef,
  loadError,
  isLoading,
  contextMenuVisible,
  contextMenuX,
  contextMenuY,
  actionMenuVisible,
  actionMenuX,
  actionMenuY,
  actions,
  activeCare,
  bondLevelTitle,
  handleFeed,
  handlePointerDown,
  handleContextMenu,
  handleAction,
  switchToNextPet,
  handleHide,
  handleOpenSettings,
  handleResetToCenter,
  handleToggleMovementMode,
  cyclePetScheduleMode,
  scheduleMenuLabel,
  scheduleTasks,
  scheduleTitle,
  scheduleX,
  scheduleY,
  scheduleEl,
  todoStore,
  closeAllMenus
} = usePetView()

import { usePetSettingsStore } from '@/stores/petSettings'
import { PET_FOODS } from '@/types/petCare'
import { todoColorHex } from '@/types/todoSchedule'
const petSettings = usePetSettingsStore()

const { t } = useI18n()

/** 饱食度条颜色档位：低于饿肚子阈值标红。 */
function hungerBarClass(hunger: number): string {
  return hunger < 10 ? 'pet-care__bar--low' : ''
}
</script>

<template>
  <div
    class="pet-window"
    @pointerdown="handlePointerDown"
    @contextmenu="handleContextMenu"
  >
    <!-- Pixi 画布挂载点 -->
    <div
      ref="hostRef"
      class="pet-window__host"
    />

    <!-- 加载中 -->
    <div
      v-if="isLoading"
      class="pet-window__hint"
    >
      …
    </div>

    <!-- 无宠物可显示 -->
    <div
      v-else-if="loadError === 'NO_PET_INSTALLED'"
      class="pet-window__hint"
    >
      无可用宠物
    </div>

    <!-- 宠物上方日程浮层：跟随宠物，展示近七日 / 近半月待办（纯展示，窗口保持穿透） -->
    <div
      v-if="scheduleTasks.length > 0"
      ref="scheduleEl"
      class="pet-schedule"
      :style="{ left: `${scheduleX}px`, top: `${scheduleY}px` }"
    >
      <div class="pet-schedule__head">{{ scheduleTitle }}</div>
      <div
        v-for="task in scheduleTasks"
        :key="`${task.id}:${task.occurDate}`"
        class="pet-schedule__item"
        :class="{ 'pet-schedule__item--today': todoStore.isToday(task.occurDate) }"
      >
        <i
          class="pet-schedule__dot"
          :style="{ background: todoColorHex(task.color) }"
        />
        <span class="pet-schedule__date">{{ task.occurDate.slice(5) }}</span>
        <span class="pet-schedule__title">{{ task.title }}</span>
      </div>
    </div>

    <!-- 点击宠物弹出的动作菜单 -->
    <div
      v-if="actionMenuVisible"
      class="pet-menu pet-menu--actions"
      :style="{ left: `${actionMenuX}px`, top: `${actionMenuY}px` }"
      @pointerdown.stop
    >
      <button
        v-for="action in actions"
        :key="action.id"
        type="button"
        class="pet-menu__item"
        @click="handleAction(action.id)"
      >
        {{ action.label }}
      </button>
    </div>

    <!-- 右键上下文菜单 -->
    <div
      v-if="contextMenuVisible"
      class="pet-menu pet-menu--context"
      :style="{ left: `${contextMenuX}px`, top: `${contextMenuY}px` }"
      @pointerdown.stop
    >
      <!-- 养成状态条：饱食度 / 亲密度 -->
      <div
        v-if="activeCare"
        class="pet-menu__care"
      >
        <div class="pet-care__row">
          <span class="pet-care__label">{{ t('ui.care.hunger') }}</span>
          <div
            class="pet-care__bar"
            :class="hungerBarClass(activeCare.hunger)"
          >
            <i :style="{ width: `${Math.round(activeCare.hunger)}%` }" />
          </div>
        </div>
        <div class="pet-care__row">
          <span class="pet-care__label">{{ t('ui.care.bond') }}</span>
          <div class="pet-care__bar pet-care__bar--bond">
            <i :style="{ width: `${Math.round(activeCare.bond)}%` }" />
          </div>
          <span class="pet-care__level">{{ bondLevelTitle }}</span>
        </div>
      </div>
      <div class="pet-menu__divider" />
      <!-- 喂食区 -->
      <button
        v-for="food in PET_FOODS"
        :key="food.id"
        type="button"
        class="pet-menu__item pet-menu__item--food"
        @click="handleFeed(food.id)"
      >
        {{ food.icon }} {{ t(food.labelKey) }}
      </button>
      <div class="pet-menu__divider" />
      <button
        type="button"
        class="pet-menu__item"
        @click="switchToNextPet(); closeAllMenus()"
      >
        下一只宠物
      </button>
      <button
        type="button"
        class="pet-menu__item"
        @click="handleResetToCenter(); closeAllMenus()"
      >
        重置到屏幕中间
      </button>
      <button
        type="button"
        class="pet-menu__item"
        @click="handleToggleMovementMode(); closeAllMenus()"
      >
        {{ petSettings.movementMode === 'fixed' ? '恢复自由漫游' : '固定位置' }}
      </button>
      <button
        type="button"
        class="pet-menu__item"
        @click="cyclePetScheduleMode(); closeAllMenus()"
      >
        {{ scheduleMenuLabel }}
      </button>
      <button
        type="button"
        class="pet-menu__item"
        @click="handleOpenSettings"
      >
        打开设置
      </button>
      <button
        type="button"
        class="pet-menu__item pet-menu__item--danger"
        @click="handleHide"
      >
        隐藏宠物
      </button>
    </div>
  </div>
</template>
<style scoped src="./PetView.css"></style>
