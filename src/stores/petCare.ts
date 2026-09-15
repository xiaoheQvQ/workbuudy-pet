/**
 * 宠物养成 store —— 喂养（饱食度）与亲密度。
 *
 * 数据按宠物 id 存在 localStorage（key: workbuddy-pet-care），与 petSettings 同款模式：
 * persist + watch 自动落盘 + storage 事件跨窗口同步（管理窗 / 宠物窗各自一份 Pinia）。
 *
 * 衰减采用「懒结算」：不在后台逐帧倒扣，而是在喂食/抚摸/定时器触发时按
 * lastDecayAt 与当前时间的差值一次性折算 —— 应用离线多天再打开也能算清，
 * 且多窗口同时结算不会叠加多扣（lastDecayAt 推进到当前时刻）。
 *
 * 规则：
 *   - 饱食度每 3 分钟 -1；喂食恢复并 + 亲密度；饱食度 ≥ 满食阈值时拒绝投喂。
 *   - 饱食度 ≤ 饿肚子阈值时，亲密度每 10 分钟 -1（饿肚子会伤心）。
 *   - 左键单击抚摸：+1 亲密度，30s 冷却。
 */
import { defineStore } from 'pinia'
import { reactive, watch } from 'vue'

import type { FeedResult, PetCareEntry, PetCareMap } from '@/types/petCare'

const STORAGE_KEY = 'workbuddy-pet-care'

// --- 衰减 / 规则参数 -----------------------------------------------------
/** 饱食度衰减速率：每 3 分钟 -1 点。 */
const HUNGER_DECAY_PER_MS = 1 / (3 * 60 * 1000)
/** 饿肚子时亲密度衰减速率：每 10 分钟 -1 点。 */
const BOND_HUNGRY_DECAY_PER_MS = 1 / (10 * 60 * 1000)
/** 饿肚子阈值：饱食度低于该值时亲密度开始流失。 */
const HUNGRY_THRESHOLD = 10
/** 满食阈值：饱食度达到该值后拒绝投喂。 */
const FULL_HUNGER_LIMIT = 97
/** 抚摸冷却（ms）。 */
const STROKE_COOLDOWN_MS = 30_000
/** 结算定时器间隔（ms）：周期性懒结算并触发持久化。 */
const SETTLE_INTERVAL_MS = 30_000

/** 新宠物的初始养成状态。 */
function createDefaultEntry(now = Date.now()): PetCareEntry {
  return {
    hunger: 65,
    bond: 15,
    fedCount: 0,
    strokeCount: 0,
    lastFedAt: null,
    lastStrokeAt: 0,
    lastDecayAt: now
  }
}

/** 懒结算：按距上次的时长折算饱食度（以及饿肚子时的亲密度）衰减。原地修改。 */
function settleEntry(entry: PetCareEntry, now = Date.now()): void {
  const elapsed = now - entry.lastDecayAt
  if (elapsed <= 0) return
  entry.lastDecayAt = now
  entry.hunger = Math.max(0, entry.hunger - elapsed * HUNGER_DECAY_PER_MS)
  if (entry.hunger <= HUNGRY_THRESHOLD) {
    entry.bond = Math.max(0, entry.bond - elapsed * BOND_HUNGRY_DECAY_PER_MS)
  }
}

/** 条目「活跃度」：跨窗口合并时用于挑出较新的一份。 */
function activityOf(entry: PetCareEntry): number {
  return Math.max(entry.lastFedAt ?? 0, entry.lastStrokeAt ?? 0, entry.lastDecayAt)
}

/** 从 localStorage 读取原始数据（容错：格式错误则返回空表）。 */
function loadFromStorage(): PetCareMap {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw) as Partial<PetCareMap>
    const map: PetCareMap = {}
    for (const [id, entry] of Object.entries(parsed)) {
      if (entry && typeof entry.hunger === 'number' && typeof entry.bond === 'number') {
        map[id] = {
          ...createDefaultEntry(),
          ...entry
        }
      }
    }
    return map
  } catch {
    return {}
  }
}

export const usePetCareStore = defineStore('petCare', () => {
  // 初始化时先对持久化数据做一次懒结算，再转为响应式（避免渲染期读取触发修改）。
  const entries = reactive<PetCareMap>(loadFromStorage())
  const bootNow = Date.now()
  for (const entry of Object.values(entries)) {
    settleEntry(entry, bootNow)
  }

  /**
   * 跨窗口同步标志（与 petSettings 相同）：persist 写入时置 true，
   * storage 事件据此跳过自身触发的回环。
   */
  let writing = false

  /** 持久化到 localStorage。 */
  function persist(): void {
    try {
      writing = true
      localStorage.setItem(STORAGE_KEY, JSON.stringify(entries))
    } catch (e) {
      console.error('[petCare] persist failed:', e)
    } finally {
      writing = false
    }
  }

  /** 取某只宠物的养成状态；不存在时用默认值初始化（首次见到的宠物从初始值起步）。 */
  function snapshot(petId: string): PetCareEntry {
    let entry = entries[petId]
    if (!entry) {
      entry = createDefaultEntry()
      entries[petId] = entry
    }
    return entry
  }

  /** 喂食：结算衰减 → 校验饱食度 → 恢复饱食度 + 亲密度。 */
  function feed(petId: string, hungerGain: number, bondGain: number): FeedResult {
    const entry = snapshot(petId)
    settleEntry(entry)
    if (entry.hunger >= FULL_HUNGER_LIMIT) {
      return { ok: false, hungerGain: 0, bondGain: 0 }
    }
    const now = Date.now()
    entry.hunger = Math.min(100, entry.hunger + hungerGain)
    entry.bond = Math.min(100, entry.bond + bondGain)
    entry.fedCount += 1
    entry.lastFedAt = now
    return { ok: true, hungerGain, bondGain }
  }

  /** 抚摸：+1 亲密度，带冷却。返回本次是否生效。 */
  function stroke(petId: string): boolean {
    const entry = snapshot(petId)
    settleEntry(entry)
    const now = Date.now()
    if (now - entry.lastStrokeAt < STROKE_COOLDOWN_MS) return false
    entry.bond = Math.min(100, entry.bond + 1)
    entry.strokeCount += 1
    entry.lastStrokeAt = now
    return true
  }

  if (typeof window !== 'undefined') {
    // 跨窗口同步：合并另一窗口的写入。逐宠物取「活跃度」较新的一份，
    // 严格大于才替换 —— 两窗写回相同值时不会互相触发持久化，避免事件回环。
    window.addEventListener('storage', (e) => {
      if (e.key !== STORAGE_KEY || writing) return
      const incoming = loadFromStorage()
      for (const [id, inc] of Object.entries(incoming)) {
        const cur = entries[id]
        if (!cur || activityOf(inc) > activityOf(cur)) {
          entries[id] = inc
        }
      }
      for (const id of Object.keys(entries)) {
        if (!(id in incoming)) delete entries[id]
      }
    })

    // 周期性懒结算：让状态条/气泡即使不喂食也能随时间衰减，watch 自动落盘。
    setInterval(() => {
      const now = Date.now()
      for (const entry of Object.values(entries)) {
        settleEntry(entry, now)
      }
    }, SETTLE_INTERVAL_MS)
  }

  // 任意条目变化自动持久化。
  watch(entries, persist, { deep: true })

  return {
    entries,
    snapshot,
    feed,
    stroke,
    persist
  }
})
