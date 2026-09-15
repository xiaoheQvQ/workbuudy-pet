<script setup lang="ts">
/**
 * TodoPanel — 待办日历面板（管理窗口「待办日历」标签页）。
 *
 * 深色日历布局（参考滴答清单式 UI）：
 *   - 左侧栏：统计卡（全部待办 / 已完成 / 今日 / 提醒）+ 选中日任务列表 + 快速添加
 *     + 宠物日程展示范围 / 固定到桌面；
 *   - 右侧：大月历（周一起始、两位日期数、格内任务胶囊、单击选中、双击新建）；
 *   - 到点提醒以彩色横幅置顶展示；任务编辑器为深色弹层。
 *
 * 本面板为自包含深色面（color-scheme: dark），控件全部自定义，不依赖 naive-ui，
 * 避免浅色组件混入破坏观感。
 */
import { useTodoPanel } from './useTodoPanel'
import { todoColorHex, type TodoColorKey } from '@/types/todoSchedule'

const {
  t,
  monthTitle,
  weekdayLabels,
  calendarCells,
  selectedDate,
  prevMonth,
  nextMonth,
  goToday,
  chipsByDate,
  openEditById,
  allPendingCount,
  todayPendingCount,
  doneRecentCount,
  reminderTaskCount,
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
  handlePetScheduleMode
} = useTodoPanel()

/** 无任务日期的空胶囊组（Map miss 兜底）。 */
const EMPTY_CHIPS = { visible: [], more: 0, total: 0 }

/** 色点 / 胶囊颜色。 */
function hex(key: TodoColorKey): string {
  return todoColorHex(key)
}
</script>

<template>
  <section class="todo-app">
    <!-- 到点提醒横幅 -->
    <div
      v-for="item in activeReminders"
      :key="item.key"
      class="talert"
      :style="{ '--c': hex(item.color) }"
    >
      <svg
        class="talert__icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <path d="M18 8a6 6 0 0 0-12 0c0 7-3 9-3 9h18s-3-2-3-9" />
        <path d="M13.7 21a2 2 0 0 1-3.4 0" />
      </svg>
      <div class="talert__body">
        <span class="talert__title">{{ item.title }}</span>
        <span class="talert__meta">{{ item.date }} {{ item.time }}</span>
      </div>
      <div class="talert__actions">
        <button
          type="button"
          class="tbtn tbtn--tiny tbtn--accent"
          @click="handleReminderDone(item.key)"
        >{{ t('ui.todo.reminderDone') }}</button>
        <button
          type="button"
          class="tbtn tbtn--tiny"
          @click="handleReminderSnooze(item.key)"
        >{{ t('ui.todo.reminderSnooze') }}</button>
        <button
          type="button"
          class="tbtn tbtn--tiny tbtn--ghost"
          @click="handleReminderClose(item.key)"
        >{{ t('ui.todo.reminderClose') }}</button>
      </div>
    </div>

    <div class="todo-main">
      <!-- ============ 左侧栏 ============ -->
      <aside class="tside">
        <!-- 统计卡 2x2 -->
        <div class="tstats">
          <div class="tstat">
            <span class="tstat__icon tstat__icon--blue">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01" />
              </svg>
            </span>
            <b class="tstat__num">{{ allPendingCount }}</b>
            <span class="tstat__label">{{ t('ui.todo.status.pending') }}</span>
          </div>
          <div class="tstat">
            <span class="tstat__icon tstat__icon--green">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="10" />
                <path d="m8.5 12.2 2.4 2.4 4.6-5.2" />
              </svg>
            </span>
            <b class="tstat__num">{{ doneRecentCount }}</b>
            <span class="tstat__label">{{ t('ui.todo.status.done') }}</span>
          </div>
          <div class="tstat">
            <span class="tstat__icon tstat__icon--amber">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="3" y="4" width="18" height="18" rx="2" />
                <path d="M16 2v4M8 2v4M3 10h18" />
              </svg>
            </span>
            <b class="tstat__num">{{ todayPendingCount }}</b>
            <span class="tstat__label">{{ t('ui.todo.today') }}</span>
          </div>
          <div class="tstat">
            <span class="tstat__icon tstat__icon--red">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M18 8a6 6 0 0 0-12 0c0 7-3 9-3 9h18s-3-2-3-9" />
                <path d="M13.7 21a2 2 0 0 1-3.4 0" />
              </svg>
            </span>
            <b class="tstat__num">{{ reminderTaskCount }}</b>
            <span class="tstat__label">{{ t('ui.todo.remindersLabel') }}</span>
          </div>
        </div>

        <!-- 选中日任务 -->
        <div class="tday">
          <div class="tday__head">
            <span class="tday__date">{{ selectedDate }}</span>
            <span
              v-if="selectedPendingCount > 0"
              class="tday__count"
            >{{ selectedPendingCount }}</span>
          </div>

          <input
            v-model="quickTitle"
            type="text"
            class="tinput tday__quick"
            :placeholder="t('ui.todo.quickAddPlaceholder')"
            @keyup.enter="handleQuickAdd"
          >

          <div class="tday__list">
            <div
              v-if="selectedTasks.length === 0"
              class="tday__empty"
            >{{ t('ui.todo.dayEmpty') }}</div>
            <div
              v-for="task in selectedTasks"
              :key="task.id"
              class="trow"
              :class="{ 'trow--done': task.repeatDaily ? task.completedDates.includes(selectedDate) : task.status === 'done' }"
            >
              <button
                type="button"
                class="trow__check"
                :title="t('ui.todo.status.done')"
                @click="toggleDone(task)"
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
                  <path d="m5 12.5 4.5 4.5L19 7.5" />
                </svg>
              </button>
              <div
                class="trow__body"
                @click="openEdit(task)"
              >
                <span class="trow__title">
                  {{ task.title }}
                  <i
                    v-if="task.repeatDaily"
                    class="trow__tag"
                  >{{ t('ui.todo.daily') }}</i>
                </span>
                <span
                  v-if="task.note"
                  class="trow__note"
                >{{ task.note }}</span>
              </div>
              <span
                v-if="task.reminders.some((r) => r.enabled)"
                class="trow__bell"
                title="reminder"
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M18 8a6 6 0 0 0-12 0c0 7-3 9-3 9h18s-3-2-3-9" />
                  <path d="M13.7 21a2 2 0 0 1-3.4 0" />
                </svg>
              </span>
              <button
                type="button"
                class="trow__del"
                :title="t('ui.todo.delete')"
                @click.stop="removeTask(task)"
              >×</button>
            </div>
          </div>
        </div>

        <!-- 侧栏底部：宠物日程 + 固定到桌面 -->
        <div class="tside__foot">
          <label class="tfoot-row">
            <span class="tfoot-row__label">{{ t('ui.todo.petScheduleMode') }}</span>
            <select
              class="tselect"
              :value="petScheduleMode"
              @change="handlePetScheduleMode(($event.target as HTMLSelectElement).value as never)"
            >
              <option
                v-for="opt in petScheduleOptions"
                :key="opt.value"
                :value="opt.value"
              >{{ opt.label }}</option>
            </select>
          </label>
          <button
            type="button"
            class="tbtn tbtn--pin"
            :class="{ 'tbtn--accent': boardVisible }"
            @click="handleToggleBoard"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12 17v5M9 3h6l1 7 3 3H5l3-3z" />
            </svg>
            {{ boardVisible ? t('ui.todo.unpinFromDesktop') : t('ui.todo.pinToDesktop') }}
          </button>
          <p class="tfoot-hint">{{ t('ui.todo.petScheduleHint') }}</p>
        </div>
      </aside>

      <!-- ============ 右侧月历 ============ -->
      <div class="tcal">
        <div class="tcal__bar">
          <h2 class="tcal__month">{{ monthTitle }}</h2>
          <div class="tcal__nav">
            <button
              type="button"
              class="tbtn"
              @click="prevMonth"
            >{{ t('ui.todo.prevMonth') }}</button>
            <button
              type="button"
              class="tbtn tbtn--accent"
              @click="goToday"
            >{{ t('ui.todo.today') }}</button>
            <button
              type="button"
              class="tbtn"
              @click="nextMonth"
            >{{ t('ui.todo.nextMonth') }}</button>
          </div>
        </div>

        <div class="tcal__weekdays">
          <span
            v-for="label in weekdayLabels"
            :key="label"
            class="tcal__weekday"
          >{{ label }}</span>
        </div>

        <div class="tgrid">
          <div
            v-for="cell in calendarCells"
            :key="cell.date"
            class="tcell"
            :class="{
              'tcell--out': !cell.inMonth,
              'tcell--selected': cell.isSelected
            }"
            @click="selectedDate = cell.date"
            @dblclick="openCreate(cell.date)"
          >
            <span
              class="tcell__day"
              :class="{ 'tcell__day--today': cell.isToday }"
            >{{ cell.dayLabel }}</span>
            <span class="tcell__chips">
              <span
                v-for="chip in (chipsByDate.get(cell.date) ?? EMPTY_CHIPS).visible"
                :key="chip.id"
                class="tchip"
                :class="{ 'tchip--done': chip.done }"
                :style="{ '--c': hex(chip.color) }"
                :title="chip.title"
                @click.stop="openEditById(chip.id)"
              >
                <i class="tchip__dot" />
                <span class="tchip__text">{{ chip.title }}</span>
                <svg
                  v-if="chip.hasReminder"
                  class="tchip__bell"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                >
                  <path d="M18 8a6 6 0 0 0-12 0c0 7-3 9-3 9h18s-3-2-3-9" />
                </svg>
              </span>
              <span
                v-if="(chipsByDate.get(cell.date) ?? EMPTY_CHIPS).more > 0"
                class="tchip tchip--more"
              >+{{ (chipsByDate.get(cell.date) ?? EMPTY_CHIPS).more }}</span>
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- ============ 任务编辑器弹层 ============ -->
    <Teleport to="body">
      <div
        v-if="editorVisible"
        class="tmask"
        @click.self="closeEditor"
      >
        <div class="tdialog">
          <div class="tdialog__head">
            <h3 class="tdialog__title">
              {{ editingId ? t('ui.todo.editTask') : t('ui.todo.addTask') }}
            </h3>
            <button
              type="button"
              class="tdialog__close"
              @click="closeEditor"
            >×</button>
          </div>

          <div class="tdialog__body">
            <label class="tfield">
              <span class="tfield__label">{{ t('ui.todo.titleLabel') }}</span>
              <input
                v-model="draft.title"
                type="text"
                class="tinput"
                :placeholder="t('ui.todo.titlePlaceholder')"
                @keyup.enter="saveEditor"
              >
            </label>

            <label class="tfield">
              <span class="tfield__label">{{ t('ui.todo.noteLabel') }}</span>
              <textarea
                v-model="draft.note"
                class="tinput ttextarea"
                rows="2"
                :placeholder="t('ui.todo.notePlaceholder')"
              />
            </label>

            <div class="tfield-grid">
              <label class="tfield">
                <span class="tfield__label">{{ t('ui.todo.dateLabel') }}</span>
                <input
                  v-model="draft.date"
                  type="date"
                  class="tinput"
                >
              </label>
              <div class="tfield">
                <span class="tfield__label">{{ t('ui.todo.statusLabel') }}</span>
                <div class="tseg">
                  <button
                    type="button"
                    class="tseg__btn"
                    :class="{ 'tseg__btn--active': draft.status === 'pending' }"
                    @click="setDraftStatus('pending')"
                  >{{ t('ui.todo.status.pending') }}</button>
                  <button
                    type="button"
                    class="tseg__btn"
                    :class="{ 'tseg__btn--active': draft.status === 'done' }"
                    @click="setDraftStatus('done')"
                  >{{ t('ui.todo.status.done') }}</button>
                </div>
              </div>
            </div>

            <div class="tfield">
              <span class="tfield__label">{{ t('ui.todo.colorLabel') }}</span>
              <div class="tcolors">
                <button
                  v-for="c in colorOptions"
                  :key="c.key"
                  type="button"
                  class="tcolor"
                  :class="{ 'tcolor--active': draft.color === c.key }"
                  :style="{ background: todoColorHex(c.key) }"
                  :title="colorLabel(c.key)"
                  @click="draft.color = c.key"
                />
              </div>
            </div>

            <label class="tfield tfield--switch">
              <span class="tfield__label">{{ t('ui.todo.repeatDaily') }}</span>
              <span
                class="tswitch"
                :class="{ 'tswitch--on': draft.repeatDaily }"
                role="switch"
                :aria-checked="draft.repeatDaily"
                @click="draft.repeatDaily = !draft.repeatDaily"
              ><i /></span>
            </label>
            <p class="thint">{{ t('ui.todo.repeatDailyHint') }}</p>

            <div class="tfield">
              <span class="tfield__label">{{ t('ui.todo.remindersLabel') }}</span>
              <div class="treminders">
                <div
                  v-for="(r, i) in draft.reminders"
                  :key="r.id"
                  class="treminder"
                >
                  <input
                    v-model="r.time"
                    type="time"
                    class="tinput treminder__time"
                  >
                  <span
                    class="tswitch tswitch--sm"
                    :class="{ 'tswitch--on': r.enabled }"
                    role="switch"
                    :aria-checked="r.enabled"
                    @click="r.enabled = !r.enabled"
                  ><i /></span>
                  <button
                    type="button"
                    class="treminder__del"
                    @click="removeReminderRow(i)"
                  >×</button>
                </div>
                <button
                  type="button"
                  class="treminder__add"
                  @click="addReminderRow"
                >{{ t('ui.todo.reminderAdd') }}</button>
              </div>
              <p class="thint">{{ t('ui.todo.remindersHint') }}</p>
            </div>
          </div>

          <div class="tdialog__foot">
            <button
              type="button"
              class="tbtn"
              @click="closeEditor"
            >{{ t('ui.common.cancel') }}</button>
            <button
              type="button"
              class="tbtn tbtn--accent"
              :disabled="!draft.title.trim()"
              @click="saveEditor"
            >{{ t('ui.todo.save') }}</button>
          </div>
        </div>
      </div>
    </Teleport>
  </section>
</template>
<style scoped src="./TodoPanel.css"></style>
