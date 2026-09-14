import { computed, onMounted, onUnmounted, ref } from 'vue'
import {
  CODEX_ATLAS_COLS,
  CODEX_ATLAS_ROWS,
  CODEX_ATLAS_WIDTH,
  CODEX_CELL_HEIGHT
} from '@/modules/desktopPet/engine'

/**
 * 单格精灵图缩略图。
 *
 * 精灵图是 8 列 × N 行、每格 192×208 的整图。直接整图当缩略图会显示成不可辨认的网格。
 * 这里用 CSS background：`background-size: 800% (N×100)%`（= 8 列宽 × N 行高，即整图按单格放大），
 * 再用 `background-position` 精确切到指定 row/col 的单格，只显示该帧。
 *
 * 行数 N 的确定顺序：显式传入 `rows` > 按图片真实尺寸探测 > 默认 9 行（v1）。
 * 在线市场的宠物混排 v1（9 行）/ v2（11 行），且上游清单标注的版本号实测不可靠，
 * 故默认走真实尺寸探测，避免取帧错位。
 *
 * 本地精灵图（convertFileSrc 产出）与远程 https 精灵图通用，永远只显示一格。
 * `lazy=true` 时通过 IntersectionObserver 在进入视口后才设置背景图，避免市场一次拉满大图。
 */
export interface PetThumbProps {
  /** 精灵图 URL（本地 convertFileSrc 或远程 https）。 */
  src: string
  /** 行索引（0 起），默认 0 = idle 行。 */
  row?: number
  /** 列索引（0 起），默认 0 = 该行首帧。 */
  col?: number
  /**
   * 图集行数：v1 = 9，v2 = 11。不传则按图片真实尺寸自动探测（探测失败回退 9 行）；
   * 已知版本时显式传入可省掉一次探测加载。
   */
  rows?: number
  /** 是否懒加载（进入视口才加载），默认 true。 */
  lazy?: boolean
  /** 是否圆形裁剪（卡片头像用），默认 false。 */
  round?: boolean
}

/** 探测出的行数上限（防御异常图片算出荒谬行数）。 */
const MAX_DETECTED_ROWS = 32

export function usePetThumb(props: PetThumbProps) {
  const elRef = ref<HTMLElement | null>(null)
  const loaded = ref(false)
  /** 由图片真实尺寸探测出的行数（null = 尚未探测出）。 */
  const detectedRows = ref<number | null>(null)
  let observer: IntersectionObserver | null = null

  /** 生效行数：显式指定 > 探测值 > 默认 9 行。 */
  const rows = computed(() => props.rows ?? detectedRows.value ?? CODEX_ATLAS_ROWS)

  // 背景定位百分比：col 占比 = col / (COLS-1) * 100%，row 同理。
  // 当 background-size 是 N 倍时，position 0% 对齐左/上、100% 对齐右/下，故除以 (N-1)。
  const backgroundPosition = computed(() => {
    const cols = CODEX_ATLAS_COLS
    const total = rows.value
    const col = props.col ?? 0
    const row = props.row ?? 0
    const x = cols > 1 ? (col / (cols - 1)) * 100 : 0
    const y = total > 1 ? (row / (total - 1)) * 100 : 0
    return `${x}% ${y}%`
  })

  const thumbStyle = computed(() => {
    if (!loaded.value) {
      return { backgroundColor: 'var(--color-bg-tertiary)' } as Record<string, string>
    }
    return {
      backgroundImage: `url("${props.src}")`,
      backgroundSize: `${CODEX_ATLAS_COLS * 100}% ${rows.value * 100}%`,
      backgroundPosition: backgroundPosition.value,
      backgroundRepeat: 'no-repeat'
    } as Record<string, string>
  })

  function load(): void {
    if (loaded.value) return
    loaded.value = true
    cleanupObserver()
  }

  function cleanupObserver(): void {
    if (observer) {
      observer.disconnect()
      observer = null
    }
  }

  /**
   * 按真实像素尺寸推断图集行数：rows = 高 × 1536 / (宽 × 208)（兼容 2x 等干净缩放）。
   *
   * 图片加载失败或环境无 Image（如 jsdom 单测）时直接放行，让行数回退默认值，
   * 不因探测失败而让缩略图卡在占位态。
   */
  function detectRows(onDone: () => void): void {
    if (typeof Image === 'undefined') {
      onDone()
      return
    }
    const image = new Image()
    image.onload = () => {
      const { naturalWidth, naturalHeight } = image
      if (naturalWidth > 0 && naturalHeight > 0) {
        const value = Math.round(
          (naturalHeight * CODEX_ATLAS_WIDTH) / (naturalWidth * CODEX_CELL_HEIGHT)
        )
        if (value >= 1 && value <= MAX_DETECTED_ROWS) {
          detectedRows.value = value
        }
      }
      onDone()
    }
    image.onerror = () => onDone()
    image.src = props.src
  }

  /** 进入视口（或非懒加载）后开始：显式行数直接上背景图，否则先探测行数。 */
  function startLoad(): void {
    if (loaded.value) return
    if (props.rows !== undefined) {
      load()
      return
    }
    detectRows(load)
  }

  onMounted(() => {
    // 不懒加载：立即加载。
    if (!props.lazy) {
      startLoad()
      return
    }
    // 懒加载：进视口才加载。
    if (!elRef.value || typeof IntersectionObserver === 'undefined') {
      startLoad()
      return
    }
    observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            startLoad()
            break
          }
        }
      },
      { rootMargin: '120px' }
    )
    observer.observe(elRef.value)
  })

  onUnmounted(() => {
    cleanupObserver()
  })

  return {
    elRef,
    loaded,
    thumbStyle
  }
}
