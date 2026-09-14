/**
 * 本地宠物管理与宠物窗口控制的 Pinia store。
 *
 * - settings store → petSettings store（轻量 localStorage 持久化）
 * - 保留跨窗口 switch 事件机制（管理窗口 → 悬浮窗）
 */
import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  deleteLocalPet,
  getPetSpritesheetUrl,
  hidePetWindow,
  importLocalPet,
  listLocalPets,
  setPetAlwaysOnTop,
  showPetWindow
} from '@/services/desktopPet'
import type { LocalPetInfo } from '@/types/desktopPet'
import { usePetSettingsStore } from './petSettings'

/**
 * 桌面宠物状态：本地已安装宠物、激活宠物。
 *
 * 激活宠物的选择持久化在 petSettings.activeId（localStorage 自动落盘）。
 * 切换激活宠物时，通过 Tauri event `desktop-pet:switch` 通知宠物悬浮窗口重载精灵图。
 */

export type { LocalPetInfo }

let unlistenSwitch: (() => void) | null = null

export const useDesktopPetStore = defineStore('desktopPet', () => {
  const petSettings = usePetSettingsStore()

  // 本地宠物
  const localPets = ref<LocalPetInfo[]>([])
  const localPetsLoaded = ref(false)

  // 激活宠物 id（读写 petSettings，自动持久化）
  const activePetId = computed<string | null>({
    get: () => petSettings.activeId,
    set: (value) => {
      petSettings.activeId = value
    }
  })

  const activePet = computed(() =>
    localPets.value.find((pet) => pet.id === activePetId.value) ?? null
  )

  /** 加载本地已安装宠物列表，并在激活宠物缺失时回退到第一只。 */
  async function loadLocalPets(): Promise<LocalPetInfo[]> {
    const pets = await listLocalPets()
    localPets.value = pets
    localPetsLoaded.value = true

    // 激活宠物不存在或未设置时，默认选第一只。
    const hasActive = pets.some((pet) => pet.id === activePetId.value)
    if (!hasActive && pets.length > 0) {
      activePetId.value = pets[0].id
    }
    return pets
  }

  /**
   * 切换激活宠物：更新 petSettings + 通知宠物窗口重载精灵图。
   * 若宠物窗口尚未存在，由调用方负责 show。
   */
  async function setActivePet(petId: string): Promise<void> {
    if (petId === activePetId.value) return
    const pet = localPets.value.find((item) => item.id === petId)
    if (!pet) return

    activePetId.value = petId

    // 通知宠物窗口（若已打开）切换精灵图。窗口未打开时事件被丢弃，下次打开会用最新激活 id。
    try {
      const src = await getPetSpritesheetUrl(petId)
      const { emit } = await import('@tauri-apps/api/event')
      await emit('desktop-pet:switch', { petId, spritesheetSrc: src })
    } catch (error) {
      console.error('[desktopPet] failed to emit switch event:', error)
    }
  }

  /** 删除本地宠物。删除当前激活宠物时回退到第一只。 */
  async function removePet(petId: string): Promise<void> {
    await deleteLocalPet(petId)
    await loadLocalPets()
    if (activePetId.value === petId) {
      activePetId.value = localPets.value[0]?.id ?? null
    }
  }

  /** 从本地文件导入宠物，成功后刷新本地列表并设为激活。 */
  async function importPet(filePath: string, displayName?: string): Promise<LocalPetInfo> {
    const info = await importLocalPet(filePath, displayName)
    await loadLocalPets()
    await setActivePet(info.id)
    return info
  }

  // --- 窗口控制 ----------------------------------------------------------

  async function showPet(): Promise<void> {
    await showPetWindow()
  }

  async function hidePet(): Promise<void> {
    await hidePetWindow()
  }

  /** 切换始终置顶：更新后端窗口 + 持久化 petSettings。 */
  async function setAlwaysOnTop(alwaysOnTop: boolean): Promise<void> {
    petSettings.alwaysOnTop = alwaysOnTop
    await setPetAlwaysOnTop(alwaysOnTop)
  }

  /**
   * 在宠物窗口内监听切换事件（由本 store 从管理窗口 emit）。
   * 仅在宠物窗口（isPetWindow）内调用。
   */
  async function startPetSwitchListener(
    handler: (payload: { petId: string; spritesheetSrc: string }) => void
  ): Promise<void> {
    if (unlistenSwitch) return
    const currentWindow = getCurrentWindow()
    unlistenSwitch = await currentWindow.listen<{
      petId: string
      spritesheetSrc: string
    }>('desktop-pet:switch', (event) => {
      handler(event.payload)
    })
  }

  function stopPetSwitchListener(): void {
    unlistenSwitch?.()
    unlistenSwitch = null
  }

  return {
    // 本地
    localPets,
    localPetsLoaded,
    activePetId,
    activePet,
    // actions
    loadLocalPets,
    setActivePet,
    removePet,
    importPet,
    showPet,
    hidePet,
    setAlwaysOnTop,
    startPetSwitchListener,
    stopPetSwitchListener
  }
})
