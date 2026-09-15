/**
 * 喂养 / 亲密度（宠物养成）的类型与数值配置。
 *
 * 数值刻度统一为 0-100：
 *   - hunger 饱食度：随真实时间衰减（每 3 分钟 -1，见 petCare store）；
 *     喂食恢复，吃饱（≥ 满食阈值）拒绝继续投喂。
 *   - bond 亲密度：喂食 / 抚摸累积，等级制展示；饿肚子（≤ 饿肚子阈值）时缓慢流失。
 */

/** 单次喂食的食物定义。hunger/bond 为恢复/增加的点数。 */
export interface PetFood {
  id: 'apple' | 'fish' | 'cake'
  icon: string
  /** i18n 文案 key（ui.care.food.*）。 */
  labelKey: string
  hunger: number
  bond: number
}

/** 可投喂的食物（右键菜单喂食区展示顺序）。 */
export const PET_FOODS: readonly PetFood[] = [
  { id: 'apple', icon: '🍎', labelKey: 'ui.care.food.apple', hunger: 15, bond: 3 },
  { id: 'fish', icon: '🐟', labelKey: 'ui.care.food.fish', hunger: 30, bond: 6 },
  { id: 'cake', icon: '🍰', labelKey: 'ui.care.food.cake', hunger: 60, bond: 10 }
]

/** 单只宠物的养成状态（浮点存储，展示时取整）。 */
export interface PetCareEntry {
  /** 饱食度 0-100。 */
  hunger: number
  /** 亲密度 0-100。 */
  bond: number
  /** 累计喂食次数。 */
  fedCount: number
  /** 累计抚摸次数。 */
  strokeCount: number
  /** 最近一次喂食时间戳（ms），null 表示从未喂过。 */
  lastFedAt: number | null
  /** 最近一次抚摸时间戳（ms），0 表示从未抚摸。 */
  lastStrokeAt: number
  /** 上次衰减结算时间戳（ms）。懒结算基准：读取/定时器触发时按与现在的差值折算。 */
  lastDecayAt: number
}

/** 全部宠物的养成状态，按宠物 id 索引。 */
export interface PetCareMap {
  [petId: string]: PetCareEntry
}

/** 喂食结果。 */
export interface FeedResult {
  ok: boolean
  /** 本次实际恢复的饱食度 / 增加的亲密度（失败时为 0）。 */
  hungerGain: number
  bondGain: number
}

/** 亲密度等级（从高到低），min 为进入该等级的最低亲密度。 */
const BOND_LEVELS: ReadonlyArray<{ min: number; key: string }> = [
  { min: 80, key: 'ui.care.levelSoul' },
  { min: 60, key: 'ui.care.levelBest' },
  { min: 40, key: 'ui.care.levelFond' },
  { min: 20, key: 'ui.care.levelKnown' },
  { min: 0, key: 'ui.care.levelStranger' }
]

/** 亲密度对应的等级文案 key（ui.care.level*）。 */
export function bondLevelKey(bond: number): string {
  return (
    BOND_LEVELS.find((l) => bond >= l.min)?.key ?? BOND_LEVELS[BOND_LEVELS.length - 1].key
  )
}
