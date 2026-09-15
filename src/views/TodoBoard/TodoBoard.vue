<script setup lang="ts">
/**
 * TodoBoard — 桌面固定待办列表窗口（/todo-board）。
 *
 * 始终置顶的紧凑列表：今天起近 15 日待办按日分组，可直接勾选完成 / 删除 /
 * 回车快速添加；到点提醒以横幅展示。标题栏可拖动窗口（data-tauri-drag-region），
 * 收起按钮隐藏窗口（取消固定）。
 */
import { useTodoBoard } from './useTodoBoard'
import { todoColorHex } from '@/types/todoSchedule'

const {
  t,
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
  activeReminders,
  reminderDone,
  reminderSnooze,
  reminderClose,
  dotStyle
} = useTodoBoard()
</script>

<template>
  <div class="board">
    <!-- 标题栏：可拖动窗口 -->
    <header
      class="board__bar"
      data-tauri-drag-region
    >
      <span
        class="board__title"
        data-tauri-drag-region
      >{{ t('ui.todo.boardTitle') }}</span>
      <span class="board__count">{{ pendingCount }}</span>
      <span class="board__bar-spacer" data-tauri-drag-region />
      <button
        type="button"
        class="board__bar-btn"
        :title="t('ui.todo.boardOpenCalendar')"
        @click="openCalendar"
      >📅</button>
      <button
        type="button"
        class="board__bar-btn"
        :title="t('ui.todo.boardHide')"
        @click="hideBoard"
      >—</button>
    </header>

    <div class="board__body">
      <!-- 提醒横幅 -->
      <div
        v-for="item in activeReminders"
        :key="item.key"
        class="board__reminder"
        :style="{ '--reminder-color': todoColorHex(item.color) }"
      >
        <div class="board__reminder-title">{{ item.title }}</div>
        <div class="board__reminder-meta">{{ item.date }} {{ item.time }}</div>
        <div class="board__reminder-actions">
          <button
            type="button"
            class="board__mini board__mini--primary"
            @click="reminderDone(item.key)"
          >{{ t('ui.todo.reminderDone') }}</button>
          <button
            type="button"
            class="board__mini"
            @click="reminderSnooze(item.key)"
          >{{ t('ui.todo.reminderSnooze') }}</button>
          <button
            type="button"
            class="board__mini board__mini--ghost"
            @click="reminderClose(item.key)"
          >{{ t('ui.todo.reminderClose') }}</button>
        </div>
      </div>

      <!-- 快速添加 -->
      <input
        v-model="quickTitle"
        type="text"
        class="board__quick"
        :placeholder="t('ui.todo.quickAddPlaceholder')"
        @keyup.enter="handleQuickAdd"
      >

      <!-- 待办列表（按日分组） -->
      <div class="board__list">
        <div
          v-if="dayGroups.length === 0"
          class="board__empty"
        >{{ t('ui.todo.boardEmpty') }}</div>
        <template
          v-for="group in dayGroups"
          :key="group.date"
        >
          <div class="board__date">{{ group.label }}</div>
          <div
            v-for="task in group.tasks"
            :key="task.id"
            class="board__item"
            :style="{ '--task-color': todoColorHex(task.color) }"
          >
            <button
              type="button"
              class="board__check"
              @click="toggleDone(task, group.date)"
            >✓</button>
            <div class="board__item-main">
              <span class="board__item-title">{{ task.title }}</span>
              <span
                v-if="task.repeatDaily"
                class="board__daily"
              >{{ t('ui.todo.daily') }}</span>
              <span
                v-if="task.reminders.some((r) => r.enabled)"
                class="board__bell"
              >⏰</span>
            </div>
            <span class="board__item-dot">
              <i :style="dotStyle(task.color)" />
            </span>
            <button
              type="button"
              class="board__del"
              @click="removeTask(task)"
            >×</button>
          </div>
        </template>
      </div>

      <!-- 已完成（近 7 日，可折叠） -->
      <button
        v-if="recentDone.length > 0"
        type="button"
        class="board__done-toggle"
        @click="showDone = !showDone"
      >
        <span class="board__done-chevron">{{ showDone ? '▾' : '▸' }}</span>
        {{ t('ui.todo.boardDone') }} · {{ recentDone.length }}
      </button>
      <div
        v-if="showDone"
        class="board__list board__list--done"
      >
        <div
          v-for="({ task, date }) in recentDone"
          :key="`${task.id}:${date}`"
          class="board__item board__item--done"
          :style="{ '--task-color': todoColorHex(task.color) }"
        >
          <button
            type="button"
            class="board__check board__check--on"
            @click="toggleDone(task, date)"
          >✓</button>
          <div class="board__item-main">
            <span class="board__item-title">{{ task.title }}</span>
            <span class="board__item-date">{{ date.slice(5) }}</span>
          </div>
          <button
            type="button"
            class="board__del"
            @click="removeTask(task)"
          >×</button>
        </div>
      </div>
    </div>
  </div>
</template>
<style scoped src="./TodoBoard.css"></style>
