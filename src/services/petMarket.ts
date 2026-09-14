// 在线宠物市场（petdex.dev）IPC 封装。所有调用走 invoke，返回 Promise。
//
// 后端命令定义在 src-tauri/src/commands/pet_market.rs：
//   - fetch_market_pets：转发官网画廊的 /api/pets/search，排序/过滤/分页在服务端完成
//   - install_market_pet：下载精灵图并落盘为本地宠物（幂等）
//   - market_pets_dir：返回本地宠物根目录
//
// 排序与官网一致（默认 installed = 最多安装），见 MarketSortKey。

import { invoke } from '@tauri-apps/api/core'
import type { LocalPetInfo } from '@/types/desktopPet'
import type { MarketPage, MarketQuery } from '@/types/petMarket'

/** 拉取市场宠物（后端转发官网搜索接口，完成排序 + 过滤 + 分页）。 */
export async function fetchMarketPets(query: MarketQuery = {}): Promise<MarketPage> {
  return invoke<MarketPage>('fetch_market_pets', {
    query: query.query ?? null,
    kind: query.kind ?? null,
    sort: query.sort ?? null,
    page: query.page ?? null,
    pageSize: query.pageSize ?? null
  })
}

/** 安装一只市场宠物到本地。已安装时后端直接返回既有信息（幂等，不重复下载）。 */
export async function installMarketPet(slug: string): Promise<LocalPetInfo> {
  return invoke<LocalPetInfo>('install_market_pet', { slug })
}

/** 本地宠物根目录（安装位置，用于展示或诊断）。 */
export async function getMarketPetsDir(): Promise<string> {
  return invoke<string>('market_pets_dir')
}
