/**
 * 简体中文文案（zh-CN）。
 *
 * 采用「扁平点号 key」结构（如 `notif.tool.start`），配合 main.ts 里
 * createI18n 的 `flatJson: true` 解析。这样 `notif.tool.start` 与
 * `notif.tool.start.file` 这类「同前缀的叶节点」可共存而不冲突。
 *
 * key 分组：
 *   - notif.*  ：桌面宠物通知气泡文案（占位符 {tool}/{file}/{error}/{line}/{name}）
 *   - ui.app.* ：管理窗口标题/副标题
 *   - ui.pet.* ：宠物管理（列表/卡片/空态/导入/标签）
 *   - ui.detail.*：详情弹窗
 *   - ui.workbuddy.*：WorkBuddy 联动开关
 *   - ui.settings.*：设置项（语言等）
 *   - ui.tray.* ：系统托盘菜单
 *   - ui.common.*：通用按钮
 *   - ui.msg.*  ：操作反馈（toast）
 *
 * 其他集成代理（PetManager / notifications）会按这些 key 调用 t()。
 */
export default {
  // --- 通知气泡（notif.*）------------------------------------------------
  'notif.session.greet': '嗨！我是你的编程伙伴 👋',
  'notif.user.thinking': '思考中…',

  'notif.tool.start': '正在 {tool}…',
  'notif.tool.start.file': '正在 {tool}：{file}',
  'notif.tool.read': '📖 读取 {file}',
  'notif.tool.write': '✏️ 写入 {file}',
  'notif.tool.edit': '✏️ 编辑 {file}',
  'notif.tool.bash': '⚙️ 执行：{command}',
  'notif.tool.search': '🔍 搜索：{pattern}',
  'notif.tool.subagent': '🤖 子代理：{desc}',
  'notif.tool.subagent.generic': '🤖 调用子代理…',
  'notif.tool.webfetch': '🌐 抓取：{url}',
  'notif.tool.webfetch.generic': '🌐 抓取网页…',
  'notif.tool.websearch': '🌐 搜索：{query}',
  'notif.tool.websearch.generic': '🌐 搜索网络…',
  'notif.tool.done': '✅ {tool} 完成',
  'notif.tool.done.file': '✅ {tool} 完成：{file}',
  'notif.tool.failed': '❌ {tool} 失败：{error}',

  'notif.perm.need': '⚠️ 需要确认：{tool}',

  'notif.stop.done': '✅ 本轮完成',
  'notif.stop.done.line': '✅ 本轮完成：{line}',
  'notif.stop.empty': '⚠️ 本轮结束（无输出）',

  // --- 管理窗口标题（ui.app.*）------------------------------------------
  'ui.app.title': 'WorkBuddy-PET 桌面宠物',
  'ui.app.subtitle': '选一只陪伴你写代码的小家伙',

  // --- 宠物管理（ui.pet.*）----------------------------------------------
  'ui.pet.enable': '启用桌面宠物',
  'ui.pet.alwaysOnTop': '始终置顶',
  'ui.pet.scale': '宠物大小',
  'ui.pet.movementMode': '漫游模式',
  'ui.pet.movementFree': '自由漫游',
  'ui.pet.movementFixed': '固定位置',
  'ui.pet.myPets': '我的宠物',

  // 状态条 / 引导
  'ui.pet.status.state': '状态',
  'ui.pet.status.running': '运行中',
  'ui.pet.status.closed': '已关闭',
  'ui.pet.status.current': '当前宠物',
  'ui.pet.status.unselected': '未选择',
  'ui.pet.guide.disabled': '宠物已关闭，打开右上角开关即可在桌面显示。先选一只喜欢的吧。',

  // 空态 / 加载
  'ui.pet.loading': '加载中…',

  // 卡片标签
  'ui.pet.tag.builtin': '内置',
  'ui.pet.tag.inUse': '使用中',
  'ui.pet.tag.market': '市场',

  // 操作按钮
  'ui.pet.use': '使用',
  'ui.pet.delete': '删除',
  'ui.pet.detail': '详情',
  'ui.pet.actions': '动作',
  'ui.pet.preview': '预览',

  // --- 详情弹窗（ui.detail.*）-------------------------------------------
  'ui.detail.setActive': '设为当前',
  'ui.detail.close': '关闭',
  'ui.detail.animTitle': '动画状态（点击预览）',
  'ui.detail.previewUnavailable': '预览不可用',

  // --- WorkBuddy 联动（ui.workbuddy.*）------------------------------------------
  'ui.workbuddy.link': '启用 WorkBuddy 联动',
  'ui.workbuddy.linkHint': '开启后，WorkBuddy 的 AI 活动会驱动宠物反应（需重启 WorkBuddy 生效）',
  'ui.workbuddy.snapshotHint': 'WorkBuddy 在启动时对 hooks 打快照，修改配置后需重启才生效',
  'ui.workbuddy.linked': '已联动',
  'ui.workbuddy.unlinked': '未联动',
  'ui.workbuddy.relinkHint': '已更新配置，请重启 WorkBuddy 生效',
  'ui.workbuddy.nodeMissing': '未检测到 Node.js，联动功能需要先安装 Node.js（https://nodejs.org）',
  'ui.workbuddy.nodeOk': '检测到 Node.js {version}，可以开启联动',

  // --- Token 使用量统计（ui.stats.*）-------------------------------------
  'ui.stats.dataDir': 'WorkBuddy 数据目录',
  'ui.stats.autoDetected': '已自动检测',
  'ui.stats.notDetected': '未检测到',
  'ui.stats.dataDirPlaceholder': '留空则自动检测，或填写 ~/.workbuddy 所在目录',
  'ui.stats.dataDirApply': '应用',
  'ui.stats.dataDirOk': '数据目录已生效',
  'ui.stats.dataDirNotFound': '该目录下未找到 WorkBuddy 数据库，请检查路径',
  'ui.stats.dataDirError': '设置失败：{error}',
  'ui.stats.today': '今日',
  'ui.stats.calls': '次调用',
  'ui.stats.noData': '今天还没有 AI 活动哦~',

  // --- 本地导入（ui.pet.import*）-----------------------------------------
  'ui.pet.import': '导入宠物',
  'ui.pet.importSuccess': '已导入「{name}」并设为当前宠物',
  'ui.pet.importFailed': '导入失败：{error}',
  'ui.pet.tag.uploaded': '已上传',

  // --- 删除（ui.pet.delete*）---------------------------------------------
  'ui.pet.deleteConfirm': '确定要删除「{name}」吗？',
  'ui.pet.deleteSuccess': '已删除宠物',
  'ui.pet.deleteBuiltin': '内置宠物不能删除',

  // --- 设置项（ui.settings.*）-------------------------------------------
  'ui.settings.title': '设置',
  'ui.settings.language': '语言',

  // --- AI 搭话（ui.ai.*）-------------------------------------------------
  'ui.ai.title': 'AI 搭话',
  'ui.ai.hint': '复用 WorkBuddy 里配置的自定义（Custom）模型生成台词；任何失败都会自动回退到内置语录',
  'ui.ai.model': '模型',
  'ui.ai.modelDefault': '默认',
  'ui.ai.modelDefaultNamed': '默认（{name}）',
  'ui.ai.modelEmpty': '未在 WorkBuddy 中找到自定义（Custom）模型，请先在 WorkBuddy 里添加一个',
  'ui.ai.modelLoadFailed': '读取模型列表失败：{error}',
  'ui.ai.topic': '话题',
  'ui.ai.topic.chat': '随口聊',
  'ui.ai.topic.news': '今日播报',
  'ui.ai.interval': '主动搭话',
  'ui.ai.intervalOff': '不主动',
  'ui.ai.intervalMinutes': '每 {minutes} 分钟',
  'ui.ai.newsHint': '「今日播报」由模型基于自身知识讲述近期热点，不联网检索实时新闻',
  'ui.ai.menuOn': 'AI 搭话：已开启',
  'ui.ai.menuOff': 'AI 搭话：已关闭',
  'ui.ai.askNow': '让 AI 说一句',
  'ui.ai.contextTodos': '今天还有 {count} 条待办没做完',

  // --- 系统托盘（ui.tray.*）---------------------------------------------
  'ui.tray.toggle': '显示/隐藏宠物',
  'ui.tray.openManager': '打开管理窗口',
  'ui.tray.alwaysOnTop': '始终置顶',
  'ui.tray.quit': '退出',

  // --- 通用按钮（ui.common.*）-------------------------------------------
  'ui.common.confirm': '确认',
  'ui.common.cancel': '取消',

  // --- 应用更新（ui.update.*）------------------------------------------
  'ui.update.available': '发现新版本 {version}',
  'ui.update.download': '下载更新',
  'ui.update.installing': '正在安装…',
  'ui.update.downloadFailed': '更新失败：{error}',
  'ui.update.tooltip': '点击下载并安装 {version}（安装后自动重启）',

  // --- 喂养与亲密度（ui.care.*）------------------------------------------
  'ui.care.hunger': '饱食',
  'ui.care.bond': '亲密',
  'ui.care.food.apple': '苹果',
  'ui.care.food.fish': '小鱼干',
  'ui.care.food.cake': '小蛋糕',
  'ui.care.feedResult': '谢谢投喂！{food}下肚，饱食 +{hunger}，亲密度 +{bond} 🎉',
  'ui.care.tooFull': '唔…我吃太饱啦，先消消食吧~',
  'ui.care.levelStranger': '初识',
  'ui.care.levelKnown': '熟悉',
  'ui.care.levelFond': '喜欢你',
  'ui.care.levelBest': '挚友',
  'ui.care.levelSoul': '心有灵犀',

  // --- 在线市场（ui.market.*）--------------------------------------------
  'ui.market.title': '在线市场',
  'ui.market.searchPlaceholder': '搜索宠物（名称 / 描述 / 标签）',
  'ui.market.allKinds': '全部分类',
  'ui.market.sort.curated': '精选',
  'ui.market.sort.recent': '最新',
  'ui.market.sort.popular': '最多喜欢',
  'ui.market.sort.installed': '最多安装',
  'ui.market.sort.alpha': '字母序',
  'ui.market.refresh': '刷新',
  'ui.market.retry': '重试',
  'ui.market.empty': '没有找到匹配的宠物，换个关键词试试',
  'ui.market.error': '加载失败：{error}',
  'ui.market.install': '安装',
  'ui.market.installed': '已安装',
  'ui.market.installSuccess': '已安装「{name}」并设为当前宠物',
  'ui.market.installFailed': '安装失败：{error}',

  // --- 待办与日历（ui.todo.*）--------------------------------------------
  'ui.todo.tab': '待办日历',
  'ui.todo.title': '待办日历',
  'ui.todo.pinToDesktop': '固定到桌面',
  'ui.todo.unpinFromDesktop': '已固定到桌面',
  'ui.todo.prevMonth': '上个月',
  'ui.todo.nextMonth': '下个月',
  'ui.todo.today': '今天',
  'ui.todo.weekday.sun': '日',
  'ui.todo.weekday.mon': '一',
  'ui.todo.weekday.tue': '二',
  'ui.todo.weekday.wed': '三',
  'ui.todo.weekday.thu': '四',
  'ui.todo.weekday.fri': '五',
  'ui.todo.weekday.sat': '六',
  'ui.todo.addTask': '新建任务',
  'ui.todo.editTask': '编辑任务',
  'ui.todo.titleLabel': '标题',
  'ui.todo.titlePlaceholder': '要做什么？',
  'ui.todo.noteLabel': '备注',
  'ui.todo.notePlaceholder': '备注（可选）',
  'ui.todo.dateLabel': '日期',
  'ui.todo.colorLabel': '颜色标记',
  'ui.todo.remindersLabel': '定时提醒',
  'ui.todo.remindersHint': '精确到时分，可添加多次提醒',
  'ui.todo.reminderAdd': '+ 添加提醒',
  'ui.todo.repeatDaily': '每日任务',
  'ui.todo.repeatDailyHint': '开启后每天都会出现在待办中，完成后次日自动重置',
  'ui.todo.statusLabel': '状态',
  'ui.todo.status.pending': '待办',
  'ui.todo.status.done': '已完成',
  'ui.todo.delete': '删除',
  'ui.todo.save': '保存',
  'ui.todo.dayTasks': '当日任务',
  'ui.todo.dayEmpty': '这一天还没有任务，点击「新建任务」添加',
  'ui.todo.totalPending': '{count} 条待办',
  'ui.todo.countDone': '{count} 条已完成',
  'ui.todo.quickAddPlaceholder': '输入任务，回车快速添加到选中日期',
  'ui.todo.daily': '每日',
  'ui.todo.reminderTitle': '任务提醒',
  'ui.todo.reminderDone': '标记完成',
  'ui.todo.reminderSnooze': '稍后提醒',
  'ui.todo.reminderClose': '知道了',
  'ui.todo.petScheduleMode': '宠物上方展示日程',
  'ui.todo.petSchedule.off': '不展示',
  'ui.todo.petSchedule.7d': '近七日',
  'ui.todo.petSchedule.15d': '近半月',
  'ui.todo.petScheduleHint': '打开软件后，宠物形象上方会展示所选范围内的日程',
  'ui.todo.boardTitle': '桌面待办',
  'ui.todo.boardEmpty': '暂无待办，去待办日历添加吧',
  'ui.todo.boardOpenCalendar': '打开日历',
  'ui.todo.boardHide': '收起',
  'ui.todo.boardDone': '已完成（近 7 日）',
  'ui.todo.todayLabel': '今天 · {date}',
  'ui.todo.tomorrow': '明天 · {date}',
  'ui.todo.menuSchedule': '日程展示（宠物上方）',
  'ui.todo.color.red': '红',
  'ui.todo.color.orange': '橙',
  'ui.todo.color.amber': '黄',
  'ui.todo.color.green': '绿',
  'ui.todo.color.teal': '青',
  'ui.todo.color.blue': '蓝',
  'ui.todo.color.purple': '紫',
  'ui.todo.color.pink': '粉',
  'ui.todo.color.gray': '灰'
} as const
