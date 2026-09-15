<script setup lang="ts">
/**
 * TodoPanel — 待办日历面板（管理窗口「待办日历」标签页）。
 *
 * 布局：左侧月历（格子上色点标记任务，点击选中），右侧选中日任务列表
 * （快速添加 / 勾选完成 / 编辑 / 删除）+ 提醒横幅 + 固定到桌面。
 * 任务编辑器为面板内自定义弹层（标题/备注/日期/颜色/状态/多条提醒/每日任务）。
 */
import { useTodoPanel } from './useTodoPanel'
import { todoColorHex } from '@/types/todoSchedule'

const {
  t,
  monthTitle,
  weekdayLabels,
  calendarCells,
  selectedDate,
  prevMonth,
  nextMonth,
  goToday,
  colorDotsOf,
  hasPendingOn,
  selectedTasks,
  selectedPendingCount,
  toggleDone,
  removeTask,
  quickTitle,
  handleQuickAdd,
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
  activeReminders,
  handleReminderDone,
  handleReminderSnooze,
  handleReminderClose,
  boardVisible,
  handleToggleBoard,
  petScheduleMode,
  petScheduleOptions,
  handlePetScheduleMode,
  upcoming7Count
} = useTodoPanel()

/** 日历格子上的色点背景色。 */
function dotStyle(key: string): Record<string, string> {
  return { background: todoColorHex(key as never) }
}
</script>

<template>
  <section class="todo-panel">
    <!-- 面板头部：统计 + 宠物日程范围 + 固定到桌面 -->
    <div class="todo-head">
      <div class="todo-head__stats">
        <span class="todo-head__badge">{{ t('ui.todo.totalPending', { count: upcoming7Count }) }}</span>
        <span class="todo-head__badge todo-head__badge--muted">{{ t('ui.todo.title') }}</span>
      </div>
      <div class="todo-head__actions">
        <label class="todo-head__mode">
          <span class="todo-head__mode-label">{{ t('ui.todo.petScheduleMode') }}</span>
          <n-select
            :value="petScheduleMode"
            :options="petScheduleOptions"
            size="small"
            class="todo-head__mode-select"
            @update:value="handlePetScheduleMode"
          />
        </label>
        <n-button
          size="small"
          :type="boardVisible ? 'primary' : 'default'"
          ghost
          @click="handleToggleBoard"
        >
          {{ boardVisible ? t('ui.todo.unpinFromDesktop') : t('ui.todo.pinToDesktop') }}
        </n-button>
      </div>
    </div>
    <p class="todo-head__hint">{{ t('ui.todo.petScheduleHint') }}</p>

    <!-- 提醒横幅（到点的提醒，各窗口独立展示） -->
    <div
      v-for="item in activeReminders"
      :key="item.key"
      class="todo-reminder"
      :style="{ '--reminder-color': todoColorHex(item.color) }"
    >
      <span class="todo-reminder__dot" />
      <div class="todo-reminder__body">
        <div class="todo-reminder__title">{{ item.title }}</div>
        <div class="todo-reminder__meta">{{ item.date }} {{ item.time }}</div>
      </div>
      <div class="todo-reminder__actions">
        <n-button
          size="tiny"
          type="primary"
          ghost
          @click="handleReminderDone(item.key)"
        >{{ t('ui.todo.reminderDone') }}</n-button>
        <n-button
          size="tiny"
          ghost
          @click="handleReminderSnooze(item.key)"
        >{{ t('ui.todo.reminderSnooze') }}</n-button>
        <n-button
          size="tiny"
          quaternary
          @click="handleReminderClose(item.key)"
        >{{ t('ui.todo.reminderClose') }}</n-button>
      </div>
    </div>

    <div class="todo-layout">
      <!-- 左：月历 -->
      <div class="todo-cal">
        <div class="todo-cal__nav">
          <button
            type="button"
            class="todo-cal__nav-btn"
            :title="t('ui.todo.prevMonth')"
            @click="prevMonth"
          >‹</button>
          <span class="todo-cal__month">{{ monthTitle }}</span>
          <button
            type="button"
            class="todo-cal__nav-btn"
            :title="t('ui.todo.nextMonth')"
            @click="nextMonth"
          >›</button>
          <button
            type="button"
            class="todo-cal__today"
            @click="goToday"
          >{{ t('ui.todo.today') }}</button>
        </div>
        <div class="todo-cal__weekdays">
          <span
            v-for="label in weekdayLabels"
            :key="label"
            class="todo-cal__weekday"
          >{{ label }}</span>
        </div>
        <div class="todo-cal__grid">
          <button
            v-for="cell in calendarCells"
            :key="cell.date"
            type="button"
            class="todo-cal__cell"
            :class="{
              'todo-cal__cell--out': !cell.inMonth,
              'todo-cal__cell--today': cell.isToday,
              'todo-cal__cell--selected': cell.isSelected,
              'todo-cal__cell--pending': hasPendingOn(cell.date)
            }"
            @click="selectedDate = cell.date"
            @dblclick="openCreate(cell.date)"
          >
            <span class="todo-cal__day">{{ cell.day }}</span>
            <span class="todo-cal__dots">
              <i
                v-for="c in colorDotsOf(cell.date)"
                :key="c"
                class="todo-cal__dot"
                :style="dotStyle(c)"
              />
            </span>
          </button>
        </div>
      </div>

      <!-- 右：选中日任务列表 -->
      <div class="todo-day">
        <div class="todo-day__head">
          <span class="todo-day__date">{{ selectedDate }}</span>
          <span
            v-if="selectedPendingCount > 0"
            class="todo-day__count"
          >{{ t('ui.todo.totalPending', { count: selectedPendingCount }) }}</span>
          <n-button
            size="tiny"
            type="primary"
            ghost
            class="todo-day__add"
            @click="openCreate()"
          >+ {{ t('ui.todo.addTask') }}</n-button>
        </div>

        <n-input
          v-model:value="quickTitle"
          size="small"
          :placeholder="t('ui.todo.quickAddPlaceholder')"
          class="todo-day__quick"
          @keyup.enter="handleQuickAdd"
        />

        <div class="todo-day__list">
          <div
            v-if="selectedTasks.length === 0"
            class="todo-day__empty"
          >{{ t('ui.todo.dayEmpty') }}</div>
          <div
            v-for="task in selectedTasks"
            :key="task.id"
            class="todo-item"
            :class="{ 'todo-item--done': task.repeatDaily ? task.completedDates.includes(selectedDate) : task.status === 'done' }"
            :style="{ '--task-color': todoColorHex(task.color) }"
          >
            <button
              type="button"
              class="todo-item__check"
              :title="t('ui.todo.status.done')"
              @click="toggleDone(task)"
            >✓</button>
            <div
              class="todo-item__body"
              @click="openEdit(task)"
            >
              <div class="todo-item__title">
                {{ task.title }}
                <span
                  v-if="task.repeatDaily"
                  class="todo-item__daily"
                >{{ t('ui.todo.daily') }}</span>
                <span
                  v-if="task.reminders.some((r) => r.enabled)"
                  class="todo-item__bell"
                >⏰</span>
              </div>
              <div
                v-if="task.note"
                class="todo-item__note"
              >{{ task.note }}</div>
            </div>
            <button
              type="button"
              class="todo-item__del"
              :title="t('ui.todo.delete')"
              @click.stop="removeTask(task)"
            >×</button>
          </div>
        </div>
      </div>
    </div>

    <!-- 任务编辑器弹层 -->
    <Teleport to="body">
      <div
        v-if="editorVisible"
        class="todo-editor-mask"
        @click.self="closeEditor"
      >
        <div class="todo-editor">
          <div class="todo-editor__head">
            <h3 class="todo-editor__title">
              {{ editingId ? t('ui.todo.editTask') : t('ui.todo.addTask') }}
            </h3>
            <button
              type="button"
              class="todo-editor__close"
              @click="closeEditor"
            >×</button>
          </div>

          <div class="todo-editor__body">
            <label class="todo-editor__field">
              <span class="todo-editor__label">{{ t('ui.todo.titleLabel') }}</span>
              <n-input
                v-model:value="draft.title"
                :placeholder="t('ui.todo.titlePlaceholder')"
                autofocus
                @keyup.enter="saveEditor"
              />
            </label>

            <label class="todo-editor__field">
              <span class="todo-editor__label">{{ t('ui.todo.noteLabel') }}</span>
              <n-input
                v-model:value="draft.note"
                type="textarea"
                :rows="2"
                :placeholder="t('ui.todo.notePlaceholder')"
              />
            </label>

            <div class="todo-editor__row">
              <label class="todo-editor__field todo-editor__field--date">
                <span class="todo-editor__label">{{ t('ui.todo.dateLabel') }}</span>
                <input
                  v-model="draft.date"
                  type="date"
                  class="todo-editor__date"
                >
              </label>
              <div class="todo-editor__field">
                <span class="todo-editor__label">{{ t('ui.todo.statusLabel') }}</span>
                <div class="todo-editor__status">
                  <button
                    type="button"
                    class="todo-editor__status-btn"
                    :class="{ 'todo-editor__status-btn--active': draft.status === 'pending' }"
                    @click="setDraftStatus('pending')"
                  >{{ t('ui.todo.status.pending') }}</button>
                  <button
                    type="button"
                    class="todo-editor__status-btn"
                    :class="{ 'todo-editor__status-btn--active': draft.status === 'done' }"
                    @click="setDraftStatus('done')"
                  >{{ t('ui.todo.status.done') }}</button>
                </div>
              </div>
            </div>

            <div class="todo-editor__field">
              <span class="todo-editor__label">{{ t('ui.todo.colorLabel') }}</span>
              <div class="todo-editor__colors">
                <button
                  v-for="c in colorOptions"
                  :key="c.key"
                  type="button"
                  class="todo-editor__color"
                  :class="{ 'todo-editor__color--active': draft.color === c.key }"
                  :style="{ background: todoColorHex(c.key) }"
                  :title="colorLabel(c.key)"
                  @click="draft.color = c.key"
                />
              </div>
            </div>

            <label class="todo-editor__field todo-editor__field--switch">
              <span class="todo-editor__label">{{ t('ui.todo.repeatDaily') }}</span>
              <n-switch
                v-model:value="draft.repeatDaily"
                size="small"
              />
            </label>
            <p class="todo-editor__hint">{{ t('ui.todo.repeatDailyHint') }}</p>

            <div class="todo-editor__field">
              <span class="todo-editor__label">{{ t('ui.todo.remindersLabel') }}</span>
              <div class="todo-editor__reminders">
                <div
                  v-for="(r, i) in draft.reminders"
                  :key="r.id"
                  class="todo-editor__reminder"
                >
                  <input
                    v-model="r.time"
                    type="time"
                    class="todo-editor__time"
                  >
                  <n-switch
                    v-model:value="r.enabled"
                    size="small"
                  />
                  <button
                    type="button"
                    class="todo-editor__reminder-del"
                    @click="removeReminderRow(i)"
                  >×</button>
                </div>
                <button
                  type="button"
                  class="todo-editor__reminder-add"
                  @click="addReminderRow"
                >{{ t('ui.todo.reminderAdd') }}</button>
              </div>
              <p class="todo-editor__hint">{{ t('ui.todo.remindersHint') }}</p>
            </div>
          </div>

          <div class="todo-editor__foot">
            <n-button
              size="small"
              quaternary
              @click="closeEditor"
            >{{ t('ui.common.cancel') }}</n-button>
            <n-button
              size="small"
              type="primary"
              :disabled="!draft.title.trim()"
              @click="saveEditor"
            >{{ t('ui.todo.save') }}</n-button>
          </div>
        </div>
      </div>
    </Teleport>
  </section>
</template>
<style scoped src="./TodoPanel.css"></style>
