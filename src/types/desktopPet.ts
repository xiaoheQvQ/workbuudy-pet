// 桌面宠物相关的前端类型定义（镜像后端 src-tauri/src/commands/desktop_pet.rs 的结构）。

/** 本地已安装的宠物信息（后端透出，含绝对路径便于 convertFileSrc）。 */
export interface LocalPetInfo {
  id: string
  displayName: string
  description?: string | null
  kind?: string | null
  tags: string[]
  /** 'builtin'（内置打包）、'downloaded'（历史市场下载）、'uploaded'（本地导入图片）或 'imported'（codex manifest 包）。 */
  source: 'builtin' | 'downloaded' | 'uploaded' | 'imported' | string
  /** 精灵图绝对路径（前端用 convertFileSrc 转成可加载 URL）。 */
  spritesheetPath: string
  posterPath?: string | null
  spritesheetUrl?: string | null
  version?: number | null
  /** 精灵图版本号：1=标准 9 行图集，2=扩展 11 行图集。缺省时由渲染层按图集高度推断。 */
  spriteVersionNumber?: number | null
  installedAt?: string | null
}
