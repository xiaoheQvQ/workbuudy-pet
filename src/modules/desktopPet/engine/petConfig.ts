// 默认宠物配置（对应 192x208 的精灵图单元尺寸）。
//
// 从 pixi-pet-demo/src/assets/pixelCat.ts 移植。idle 窗口有意足够长（≥6s），始终超过
// 环境池的休息时长（2.6–5.2s），使停泊的宠物可靠地进入环境状态（waving/jumping/failed/…）。

import type { PetConfig } from './types'

export const defaultPetConfig: PetConfig = {
  scale: 0.75,
  walkSpeed: 76,
  // 漫游频率：平均约 15 分钟走动一次（idle 停留 12-18 分钟，walkChance=1 到点必走）。
  // idle 窗口远大于环境池休息时长（2.6-5.2s），停泊的宠物可可靠进入环境状态
  // （waving/jumping/failed/…）。
  idleDurationRange: [12 * 60 * 1000, 18 * 60 * 1000],
  walkChance: 1,
  reactionDuration: 700,
  particleCount: 18,
}
