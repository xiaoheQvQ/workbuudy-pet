// 在线宠物市场（petdex.dev）相关的前端类型定义。
// 镜像后端 src-tauri/src/commands/pet_market.rs 的结构（统一 camelCase 序列化）。
//
// 数据与官网画廊同源（petdex.dev/api/pets/search），排序键与官网一致。

/** 官网支持的排序键（官网画廊默认 installed，搜索时自动切 curated）。 */
export type MarketSortKey = 'curated' | 'popular' | 'installed' | 'alpha' | 'recent'

/** 市场清单中的一只宠物。 */
export interface MarketPet {
  /** 宠物标识（小写字母/数字/连字符），也是本地安装目录名。 */
  slug: string
  displayName: string
  /** 分类：character / creature / object（可能为空串）。 */
  kind: string
  /** 投稿者昵称（可能为 null）。 */
  submittedBy: string | null
  /** 精灵图远程地址（https，可直接用作 PetThumb 背景图）。 */
  spritesheetUrl: string
  /** 图集版本：1 = 标准 9 行（1536×1872），2 = 扩展 11 行（1536×2288）。 */
  spriteVersionNumber: number
  /** 描述（可能为 null）。 */
  description: string | null
  /** 标签（官网 tags + vibes 合并）。 */
  tags: string[]
  /** 图鉴编号（按审核通过顺序，官网卡片上的 No.xxx）。 */
  dexNumber: number | null
  /** 官网精选标记。 */
  featured: boolean
  /** 安装量（官网「最多安装」排序依据，可能为 null）。 */
  installCount: number | null
  /** 点赞数（可能为 null）。 */
  likeCount: number | null
}

/** 分类计数（官网 facets.kinds，按数量降序）。 */
export interface MarketKindCount {
  kind: string
  count: number
}

/** 市场分页结果。 */
export interface MarketPage {
  items: MarketPet[]
  /** 过滤后的总条数。 */
  total: number
  /** 当前页码（从 1 开始）。 */
  page: number
  /** 每页条数（后端已钳制）。 */
  pageSize: number
  /** 全部可选分类及数量（筛选下拉用，不随关键词变化）。 */
  kinds: MarketKindCount[]
}

/** 市场查询参数（字段为空则不过滤）。 */
export interface MarketQuery {
  /** 关键词（走官网搜索：名称/描述/标签匹配，语义化查询自动走 vibe 搜索）。 */
  query?: string
  /** 分类过滤。 */
  kind?: string
  /** 排序键（缺省为官网默认 installed）。 */
  sort?: MarketSortKey
  /** 页码（从 1 开始）。 */
  page?: number
  /** 每页条数。 */
  pageSize?: number
}
