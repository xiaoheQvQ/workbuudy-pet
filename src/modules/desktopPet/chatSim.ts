// 桌面宠物对话模拟工具（纯函数）。
//
// 在接入 ACP 真实对话前，用于模拟 SSE 流式输出：把一段完整文案切成小 token，
// 让视图层按节奏逐个 appendChatToken，实现打字机效果。纯逻辑、无副作用，便于单测。

import type { RandomSource } from './engine/types'

/**
 * 把一段文本切成若干"token"，模拟流式增量。
 *
 * 以码点（支持中文）按 maxCharsPerChunk 切分；空串返回空数组。最后一个 chunk 可能短于上限。
 */
export function chunkMessage(text: string, maxCharsPerChunk = 2): string[] {
  if (maxCharsPerChunk < 1) {
    throw new Error('maxCharsPerChunk must be >= 1')
  }
  if (text.length === 0) return []

  // Array.from 正确处理 Unicode 码点（emoji / 扩展汉字）。
  const codepoints = Array.from(text)
  const chunks: string[] = []

  for (let i = 0; i < codepoints.length; i += maxCharsPerChunk) {
    chunks.push(codepoints.slice(i, i + maxCharsPerChunk).join(''))
  }

  return chunks
}

/** 友好中文短句池 —— 模拟宠物"想说的话"。后续接入 ACP 后不再使用。 */
export const CANNED_MESSAGES: readonly string[] = [
  '嗨！很高兴见到你～',
  '今天也要元气满满哦！',
  '有我陪着呢，别闷着啦～',
  '哼哼，偷偷休息被你发现了。',
  '要不要一起出去走走？',
  '喝杯奶茶歇一会儿吧～'
]

// --- 按时间段的俏皮语录（鼠标悬停时随机显示一条） ---
// 每个时间段 10 条，风格轻松幽默，贴合日常生活的真实心境。

/** 凌晨语录（0:00 - 5:59）：熬夜 / 晚安主题。 */
const QUOTES_DAWN: readonly string[] = [
  '都这个点了还不睡？再熬要变熊猫眼啦！',
  '夜深了，世界都安静下来了。',
  '你的黑眼圈比我的精灵图还黑了。',
  '这个点还没睡，是在等谁的晚安吗？',
  '月亮不睡你不睡，小心明天起不来～',
  '这么晚还不睡，明天不用早起吗？',
  '夜深人静，最适合发会儿呆。',
  '别熬太晚哦，我会心疼的。',
  '嘘——天上的星星都困了，你也快睡吧。',
  '深夜的脑洞特别大，但身体要紧呀。'
]

/** 早上语录（6:00 - 10:59）：起床 / 新一天主题。 */
const QUOTES_MORNING: readonly string[] = [
  '早上好呀！今天也是崭新的一天～',
  '早安！昨晚睡得好吗？',
  '新的一天，也要好好照顾自己哦。',
  '元气满满！今天有什么小计划吗？',
  '早餐吃了吗？可别空着肚子哦。',
  '拉开窗帘看看，今天天气怎么样？',
  '出门记得看看天气，别忘带伞～',
  '又是元气满满的一天，冲鸭！',
  '今天的计划：好好吃饭，好好生活。',
  '起这么早？自律达人非你莫属！'
]

/** 中午语录（11:00 - 13:59）：午饭 / 午休主题。 */
const QUOTES_NOON: readonly string[] = [
  '中午啦，该休息一会儿了！',
  '干饭时间到！手头的事先放一放～',
  '午饭后犯困正常，眯一会儿就好。',
  '吃了没？别忙起来就忘了吃饭。',
  '中午不睡，下午崩溃，听我的。',
  '饭点到了，一起干饭吧！',
  '午休一下吧，我会替你看着时间的～',
  '吃饱一点，下午才有力气～',
  '中午的阳光真好，出去晒晒吧。',
  '先去吃饭，下午再战！'
]

/** 下午语录（14:00 - 17:59）：犯困 / 补充能量主题。 */
const QUOTES_AFTERNOON: readonly string[] = [
  '下午好困啊，要不趴会儿？',
  '距离下班还有…我帮你数着呢。',
  '下午茶时间！奶茶还是咖啡？',
  '这个点了，今天的事做完了吗？',
  '下午容易犯困，起来活动活动吧。',
  '困了就站起来伸个懒腰～别学我坐着不动。',
  '喝口水休息一下，别太累了。',
  '下午三点了，有点小困很正常。',
  '再撑一会儿就到晚上了，加油！',
  '来块小饼干，补充点能量吧～'
]

/** 晚上语录（18:00 - 23:59）：放松 / 夜晚主题。 */
const QUOTES_EVENING: readonly string[] = [
  '晚上好！今天辛苦了～',
  '天黑了，该回家啦。',
  '晚饭吃了吗？别亏待自己。',
  '忙了一天，记得犒劳下自己。',
  '晚上适合散步，要不要出去走走？',
  '今天辛苦了，给自己点个夜宵吧。',
  '这么晚还在忙？记得早点休息哦。',
  '夜幕降临，把烦恼都放下吧。',
  '别太累了，回家好好歇歇。',
  '晚上容易饿，夜宵可别吃太多哦。'
]

/** 按当前小时返回对应时间段语录池。 */
function getQuotesByHour(hour: number): readonly string[] {
  if (hour >= 0 && hour < 6) return QUOTES_DAWN
  if (hour >= 6 && hour < 11) return QUOTES_MORNING
  if (hour >= 11 && hour < 14) return QUOTES_NOON
  if (hour >= 14 && hour < 18) return QUOTES_AFTERNOON
  return QUOTES_EVENING
}

/**
 * 从固定短句池随机取一条（用注入的 rng 保证可测）。
 */
export function pickCannedMessage(rng: RandomSource = Math.random): string {
  const index = Math.floor(rng() * CANNED_MESSAGES.length) % CANNED_MESSAGES.length
  return CANNED_MESSAGES[index]
}

/**
 * 按当前时间段从对应语录池随机取一条俏皮消息（鼠标悬停时用）。
 *
 * 根据当前系统时间的小时分段（凌晨/早上/中午/下午/晚上）选择语录池，
 * 每个池 10 条贴合打工人心境的幽默短句，随机返回一条。
 *
 * @param rng 随机源（默认 Math.random，可注入便于测试）
 * @param date 用于判断时间段的日期（默认 new Date()，可注入便于测试）
 */
export function pickTimeBasedMessage(
  rng: RandomSource = Math.random,
  date: Date = new Date()
): string {
  const quotes = getQuotesByHour(date.getHours())
  const index = Math.floor(rng() * quotes.length) % quotes.length
  return quotes[index]
}
