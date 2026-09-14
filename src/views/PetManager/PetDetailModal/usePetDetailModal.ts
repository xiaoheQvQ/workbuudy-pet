/** usePetDetailModal — 桌面宠物详情弹窗组件的 composable。 */
import { computed, ref, watch } from 'vue'
import { CODEX_ATLAS_ROWS_DEF } from '@/modules/desktopPet/engine'

/**
 * 宠物详情弹窗：大图实时预览 + 动画状态切换 + 使用/删除操作。
 *
 * pet.spritesheetSrc 为本地 convertFileSrc URL，PetPreview 直接加载。
 * 点击某行动画按钮 → 设置 activeAction → 大图实时播放该动画。
 */

/** 详情用统一宠物结构。 */
export interface DetailPet {
  id: string
  displayName: string
  description?: string | null
  kind?: string | null
  tags: string[]
  /** 精灵图源（本地 convertFileSrc）。 */
  spritesheetSrc: string
  /** 是否已本地安装。 */
  installed: boolean
  /** 来源标签：'builtin' | 'downloaded' | 'uploaded' | 'imported' | 'market'。 */
  source: string
  /** 安装时间（本地宠物可选）。 */
  installedAt?: string | null
  /** 投稿者昵称（仅在线市场来源有值）。 */
  submittedBy?: string | null
}

export interface PetDetailModalProps {
  visible: boolean
  pet: DetailPet | null
  /** 是否为当前激活宠物（决定"设为当前"按钮状态）。 */
  isActive?: boolean
}

export interface PetDetailModalEmits {
  (event: 'update:visible', value: boolean): void
  (event: 'use', petId: string): void
  (event: 'delete', petId: string): void
  /** 安装未安装的宠物（在线市场来源）。 */
  (event: 'install', petId: string): void
}

export function useDetailModal(
  props: PetDetailModalProps,
  emit: (
    event: 'update:visible' | 'use' | 'delete' | 'install',
    ...args: unknown[]
  ) => void
) {
  // 当前正在播放的动画行 id（空 = 自动漫游）。
  const activeAction = ref<string>('')

  // 动画行（idle/running-right/.../review）。
  const animRows = CODEX_ATLAS_ROWS_DEF

  const canUse = computed(() => props.pet?.installed && !props.isActive)
  const canDelete = computed(
    () => props.pet?.installed && props.pet.source !== 'builtin'
  )
  /** 在线市场宠物且尚未安装：显示「安装」而非「使用中」。 */
  const canInstall = computed(() => !!props.pet && !props.pet.installed)

  function close(): void {
    emit('update:visible', false)
  }

  function handleUse(): void {
    if (props.pet) {
      emit('use', props.pet.id)
    }
    close()
  }

  function handleDelete(): void {
    if (props.pet) {
      emit('delete', props.pet.id)
    }
    close()
  }

  /** 安装未安装的宠物。不关闭弹窗：安装完成后由外部刷新 pet.installed 状态。 */
  function handleInstall(): void {
    if (props.pet) {
      emit('install', props.pet.id)
    }
  }

  function playAnim(rowId: string): void {
    // 再次点击同一动画 → 取消（回到自动漫游）。
    activeAction.value = activeAction.value === rowId ? '' : rowId
  }

  // 弹窗打开时重置动画选择。
  watch(
    () => props.visible,
    (visible) => {
      if (visible) {
        activeAction.value = ''
      }
    }
  )

  return {
    animRows,
    activeAction,
    canUse,
    canDelete,
    canInstall,
    close,
    handleUse,
    handleDelete,
    handleInstall,
    playAnim
  }
}
