import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { createPetApp } from '@/modules/desktopPet/engine'
import type { PetApp } from '@/modules/desktopPet/engine'

/**
 * 宠物实时预览（PixiJS）。
 *
 * 精灵图来源为本地已安装宠物：convertFileSrc 产出的
 * `http://asset.localhost/.../spritesheet.webp`，可直接交给 PixiJS Assets.load。
 *
 * 行为：
 *   - spritesheetSrc / petId 变化 → switchPet（不重建引擎）。
 *   - scale 变化 → 重建引擎。
 *   - activeAction 变化 → playAction。
 *   - 卸载 → destroy 释放纹理。
 */
export interface PetPreviewProps {
  /** 宠物 id（用于后端缓存目录命名）。 */
  petId: string
  /** 精灵图源（本地 convertFileSrc URL）。 */
  spritesheetSrc: string
  /** 宠物缩放（相对 192x208 单元），默认 0.9。 */
  scale?: number
  /** 当前手动触发的动作 id（如 'waving'）；为空则自动漫游。 */
  activeAction?: string
}

export function usePetPreview(props: PetPreviewProps) {
  const hostRef = ref<HTMLElement | null>(null)
  const petApp = ref<PetApp | null>(null)
  const loadError = ref<string | null>(null)
  const isLoading = ref(true)

  async function bootApp(): Promise<void> {
    if (!hostRef.value) return
    await destroyApp()
    isLoading.value = true
    loadError.value = null
    try {
      petApp.value = await createPetApp(hostRef.value, {
        initialPetId: props.petId,
        initialSpritesheetSrc: props.spritesheetSrc,
        config: { scale: props.scale ?? 0.9 },
        preview: true
      })
      isLoading.value = false
    } catch (error) {
      console.error('[PetPreview] boot failed:', error)
      loadError.value = error instanceof Error ? error.message : String(error)
      isLoading.value = false
    }
  }

  async function destroyApp(): Promise<void> {
    if (petApp.value) {
      try {
        await petApp.value.destroy()
      } catch (error) {
        console.error('[PetPreview] destroy failed:', error)
      }
      petApp.value = null
    }
  }

  onMounted(() => {
    void bootApp()
  })

  // src / petId 变化：切换宠物。
  watch(
    () => [props.spritesheetSrc, props.petId] as const,
    async ([src, id]) => {
      if (!petApp.value) return
      if (id === petApp.value.currentPetId) return
      isLoading.value = true
      try {
        await petApp.value.switchPet(id, src)
        isLoading.value = false
      } catch (error) {
        console.error('[PetPreview] switch failed:', error)
        loadError.value = error instanceof Error ? error.message : String(error)
        isLoading.value = false
      }
    }
  )

  // scale 变化：重建引擎。
  watch(
    () => props.scale ?? 0.9,
    () => {
      void bootApp()
    }
  )

  // activeAction 变化：触发动作。
  watch(
    () => props.activeAction,
    (action) => {
      if (petApp.value && action) {
        petApp.value.playAction(action)
      }
    }
  )

  onBeforeUnmount(() => {
    void destroyApp()
  })

  return {
    hostRef,
    isLoading,
    loadError
  }
}
