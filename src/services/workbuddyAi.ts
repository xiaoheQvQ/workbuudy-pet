// WorkBuddy 免费模型 AI 台词服务。
//
// 复用 WorkBuddy 自身配置的模型（`~/.workbuddy/models.json`，含免费额度模型）生成宠物台词，
// 用户无需在本应用里另填任何 API Key。命令定义在
// src-tauri/src/commands/desktop_pet.rs（list_workbuddy_models / generate_pet_line）。
//
// 设计原则：AI 台词是「锦上添花」，任何失败（未配置模型 / 网络异常 / 超时）
// 都不向上抛错，由调用方回退到内置固定语录。

import { invoke } from '@tauri-apps/api/core'
import type { AiTopic } from '@/stores/petSettings'

/** 透出给前端的模型信息（后端已脱敏，不含 apiKey）。 */
export interface WorkBuddyModelInfo {
  id: string
  name: string
  vendor?: string
  /** 是否为「未指定模型」时后端实际会用的默认模型（下拉首项文案据此生成）。 */
  isDefault: boolean
}

/**
 * AI 台词生成的超时（ms）。比后端 30s 请求超时略宽，
 * 保证后端能先返回明确错误，而不是前端先静默丢弃。
 */
const AI_TIMEOUT_MS = 35_000

/** 列出 WorkBuddy 已配置的模型（供设置页下拉选择）。 */
export async function listWorkBuddyModels(): Promise<WorkBuddyModelInfo[]> {
  return invoke<WorkBuddyModelInfo[]>('list_workbuddy_models')
}

/** AI 台词生成入参。 */
export interface AiLineParams {
  /** 模型 id；null / undefined = 用后端默认模型（后端按渠道偏好挑选）。 */
  modelId?: string | null
  /** 话题：'chat' 随口聊 / 'news' 今日播报。 */
  topic?: AiTopic
  /** 台词语言（'zh-CN' / 'en-US'）。 */
  locale?: string
  /** 现场上下文（今日待办等），会拼进提示词。 */
  context?: string | null
}

/** 调后端生成一句宠物台词。失败时抛错（由 {@link requestAiPetLine} 兜底）。 */
export async function generatePetLine(params: AiLineParams = {}): Promise<string> {
  return invoke<string>('generate_pet_line', {
    modelId: params.modelId ?? null,
    topic: params.topic ?? 'chat',
    locale: params.locale ?? 'zh-CN',
    context: params.context ?? null
  })
}

/**
 * 请求一句 AI 台词（带超时 + 全异常兜底）。
 *
 * 任何失败都返回 `null`，绝不抛错：调用方据此回退内置固定语录，
 * 保证宠物在任何情况下都「有话可说」。
 */
export async function requestAiPetLine(
  params: AiLineParams = {},
  timeoutMs = AI_TIMEOUT_MS
): Promise<string | null> {
  let timer: ReturnType<typeof setTimeout> | undefined
  const timeout = new Promise<null>((resolve) => {
    timer = setTimeout(() => resolve(null), timeoutMs)
  })

  try {
    const line = await Promise.race([generatePetLine(params), timeout])
    return line || null
  } catch (error) {
    console.warn('[workbuddyAi] generate pet line failed:', error)
    return null
  } finally {
    if (timer) clearTimeout(timer)
  }
}
