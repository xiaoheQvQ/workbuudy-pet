/**
 * English (US) messages (en-US).
 *
 * Mirrors the key structure of `./zh-CN` exactly (flat dotted keys).
 * Placeholders use the same {tool}/{file}/{error}/{line}/{name} tokens.
 * Loaded via createI18n with `flatJson: true` in main.ts.
 */
export default {
  // --- Notification bubbles (notif.*) -----------------------------------
  'notif.session.greet': "Hi! I'm your coding buddy 👋",
  'notif.user.thinking': 'Thinking…',

  'notif.tool.start': 'Running {tool}…',
  'notif.tool.start.file': 'Running {tool}: {file}',
  'notif.tool.read': '📖 Reading {file}',
  'notif.tool.write': '✏️ Writing {file}',
  'notif.tool.edit': '✏️ Editing {file}',
  'notif.tool.bash': '⚙️ Running: {command}',
  'notif.tool.search': '🔍 Searching: {pattern}',
  'notif.tool.subagent': '🤖 Subagent: {desc}',
  'notif.tool.subagent.generic': '🤖 Calling subagent…',
  'notif.tool.webfetch': '🌐 Fetching: {url}',
  'notif.tool.webfetch.generic': '🌐 Fetching web…',
  'notif.tool.websearch': '🌐 Searching: {query}',
  'notif.tool.websearch.generic': '🌐 Searching web…',
  'notif.tool.done': '✅ {tool} done',
  'notif.tool.done.file': '✅ {tool} done: {file}',
  'notif.tool.failed': '❌ {tool} failed: {error}',

  'notif.perm.need': '⚠️ Needs confirmation: {tool}',

  'notif.stop.done': '✅ Turn complete',
  'notif.stop.done.line': '✅ Turn complete: {line}',
  'notif.stop.empty': '⚠️ Turn ended (no output)',

  // --- Manager window title (ui.app.*) ----------------------------------
  'ui.app.title': 'WorkBuddy-PET Desktop Pet',
  'ui.app.subtitle': 'Pick a little buddy to keep you company while you code',

  // --- Pet management (ui.pet.*) ----------------------------------------
  'ui.pet.enable': 'Enable desktop pet',
  'ui.pet.alwaysOnTop': 'Always on top',
  'ui.pet.scale': 'Pet size',
  'ui.pet.movementMode': 'Roam mode',
  'ui.pet.movementFree': 'Free roam',
  'ui.pet.movementFixed': 'Fixed in place',
  'ui.pet.myPets': 'My pets',

  // Status bar / guide
  'ui.pet.status.state': 'Status',
  'ui.pet.status.running': 'Running',
  'ui.pet.status.closed': 'Off',
  'ui.pet.status.current': 'Current pet',
  'ui.pet.status.unselected': 'Not selected',
  'ui.pet.guide.disabled': 'The pet is off. Toggle the switch in the top-right to show it on your desktop. Pick one you like first.',

  // Empty / loading
  'ui.pet.loading': 'Loading…',

  // Card tags
  'ui.pet.tag.builtin': 'Built-in',
  'ui.pet.tag.inUse': 'In use',
  'ui.pet.tag.market': 'Market',

  // Action buttons
  'ui.pet.use': 'Use',
  'ui.pet.delete': 'Delete',
  'ui.pet.detail': 'Details',
  'ui.pet.actions': 'Actions',
  'ui.pet.preview': 'Preview',

  // --- Detail modal (ui.detail.*) ---------------------------------------
  'ui.detail.setActive': 'Set as current',
  'ui.detail.close': 'Close',
  'ui.detail.animTitle': 'Animations (click to preview)',
  'ui.detail.previewUnavailable': 'Preview unavailable',

  // --- WorkBuddy integration (ui.workbuddy.*) -----------------------------------
  'ui.workbuddy.link': 'Enable WorkBuddy link',
  'ui.workbuddy.linkHint': 'When on, WorkBuddy AI activity drives pet reactions (restart WorkBuddy to take effect).',
  'ui.workbuddy.snapshotHint': 'WorkBuddy snapshots hooks at startup, so restart it after changing the config.',
  'ui.workbuddy.linked': 'Linked',
  'ui.workbuddy.unlinked': 'Not linked',
  'ui.workbuddy.relinkHint': 'Config updated — restart WorkBuddy to apply.',
  'ui.workbuddy.nodeMissing': 'Node.js not found. The link feature requires Node.js installed first (https://nodejs.org).',
  'ui.workbuddy.nodeOk': 'Found Node.js {version}, ready to link.',

  // --- Token usage stats (ui.stats.*) -----------------------------------
  'ui.stats.dataDir': 'WorkBuddy data directory',
  'ui.stats.autoDetected': 'Auto-detected',
  'ui.stats.notDetected': 'Not detected',
  'ui.stats.dataDirPlaceholder': 'Leave empty for auto-detect, or enter the dir containing ~/.workbuddy',
  'ui.stats.dataDirApply': 'Apply',
  'ui.stats.dataDirOk': 'Data directory applied',
  'ui.stats.dataDirNotFound': 'WorkBuddy database not found in that directory, please check the path',
  'ui.stats.dataDirError': 'Failed: {error}',
  'ui.stats.today': 'Today',
  'ui.stats.calls': ' calls',
  'ui.stats.noData': 'No AI activity today yet~',

  // --- Local import (ui.pet.import*) -------------------------------------
  'ui.pet.import': 'Import pet',
  'ui.pet.importSuccess': 'Imported "{name}" and set as current pet',
  'ui.pet.importFailed': 'Import failed: {error}',
  'ui.pet.tag.uploaded': 'Uploaded',

  // --- Delete (ui.pet.delete*) -------------------------------------------
  'ui.pet.deleteConfirm': 'Delete "{name}"?',
  'ui.pet.deleteSuccess': 'Pet deleted',
  'ui.pet.deleteBuiltin': 'Built-in pets cannot be deleted',

  // --- Settings (ui.settings.*) -----------------------------------------
  'ui.settings.title': 'Settings',
  'ui.settings.language': 'Language',

  // --- System tray (ui.tray.*) ------------------------------------------
  'ui.tray.toggle': 'Show/hide pet',
  'ui.tray.openManager': 'Open manager',
  'ui.tray.alwaysOnTop': 'Always on top',
  'ui.tray.quit': 'Quit',

  // --- Common buttons (ui.common.*) -------------------------------------
  'ui.common.confirm': 'OK',
  'ui.common.cancel': 'Cancel',

  // --- App update (ui.update.*) ----------------------------------------
  'ui.update.available': 'New version {version} available',
  'ui.update.download': 'Download update',
  'ui.update.installing': 'Installing…',
  'ui.update.downloadFailed': 'Update failed: {error}',
  'ui.update.tooltip': 'Click to download and install {version} (restarts automatically)',

  // --- Online market (ui.market.*) --------------------------------------
  'ui.market.title': 'Online Market',
  'ui.market.searchPlaceholder': 'Search pets (name / description / tags)',
  'ui.market.allKinds': 'All types',
  'ui.market.sort.curated': 'Curated',
  'ui.market.sort.recent': 'Newest',
  'ui.market.sort.popular': 'Most liked',
  'ui.market.sort.installed': 'Most installed',
  'ui.market.sort.alpha': 'Alphabetical',
  'ui.market.refresh': 'Refresh',
  'ui.market.retry': 'Retry',
  'ui.market.empty': 'No pets matched. Try another keyword.',
  'ui.market.error': 'Failed to load: {error}',
  'ui.market.install': 'Install',
  'ui.market.installed': 'Installed',
  'ui.market.installSuccess': 'Installed "{name}" and set as current pet',
  'ui.market.installFailed': 'Install failed: {error}'
} as const
