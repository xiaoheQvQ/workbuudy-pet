// 待办事项「固定到桌面」窗口（todo-board）命令层。
//
// 职责：创建 / 显示 / 隐藏 / 切换一个始终置顶的小型待办列表窗口，
// 把整个待办列表钉在桌面最上层，便于随时查看任务进展。
//
// 窗口特性（与宠物窗口同款思路，但不透明、可交互）：
//   - 无边框 + 置顶 + 跳过任务栏，不抢焦点
//   - 关闭（CloseRequested）由 lib.rs 统一 prevent_close + hide，窗口常驻可复用
//   - 显示时定位到当前显示器右下角（考虑任务栏预留边距）

use std::time::Duration;

use tauri::{AppHandle, Listener, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

/// todo-board 窗口 label。
pub const TODO_BOARD_LABEL: &str = "todo-board";

/// 窗口逻辑尺寸（与前端 TodoBoard 视图的布局匹配）。
const BOARD_WIDTH: f64 = 320.0;
const BOARD_HEIGHT: f64 = 520.0;
/// 距屏幕右 / 下边距（逻辑 px，给任务栏 / 桌面边缘留空隙）。
const RIGHT_MARGIN: f64 = 16.0;
const BOTTOM_MARGIN: f64 = 16.0;

/// 确保 todo-board 窗口存在（已存在则直接返回）。
///
/// 创建时保持隐藏；首次显示由 show_todo_board_window 定位并展示。
fn ensure_todo_board_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(TODO_BOARD_LABEL) {
        return Ok(window);
    }

    WebviewWindowBuilder::new(
        app,
        TODO_BOARD_LABEL,
        WebviewUrl::App("/todo-board".into()),
    )
    .title("Desktop Todos")
    .inner_size(BOARD_WIDTH, BOARD_HEIGHT)
    .min_inner_size(260.0, 320.0)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(true)
    .shadow(true)
    .visible(false)
    .focused(false)
    .build()
    .map_err(|e| format!("创建桌面待办窗口失败: {}", e))?;

    app.get_webview_window(TODO_BOARD_LABEL)
        .ok_or_else(|| "桌面待办窗口创建后无法获取".to_string())
}

/// 把 todo-board 窗口定位到当前显示器右下角（逻辑坐标）。
fn position_bottom_right(window: &tauri::WebviewWindow) -> Result<(), String> {
    let monitor = window
        .current_monitor()
        .map_err(|e| format!("获取当前显示器失败: {}", e))?
        .or_else(|| {
            window
                .primary_monitor()
                .map_err(|e| format!("获取主显示器失败: {}", e))
                .ok()
                .flatten()
        })
        .ok_or_else(|| "未找到可用显示器".to_string())?;

    let scale = monitor.scale_factor();
    let mon_origin_x = monitor.position().x as f64 / scale;
    let mon_origin_y = monitor.position().y as f64 / scale;
    let mon_logical_w = monitor.size().width as f64 / scale;
    let mon_logical_h = monitor.size().height as f64 / scale;

    // 用户可能拖大窗口：按窗口当前实际尺寸定位。
    let size = window
        .inner_size()
        .map_err(|e| format!("获取窗口尺寸失败: {}", e))?;
    let w = (size.width as f64 / scale).min(mon_logical_w - 16.0);
    let h = (size.height as f64 / scale).min(mon_logical_h - 16.0);

    let x = mon_origin_x + mon_logical_w - w - RIGHT_MARGIN;
    let y = mon_origin_y + mon_logical_h - h - BOTTOM_MARGIN;

    window
        .set_position(LogicalPosition::new(x.max(mon_origin_x + 8.0), y.max(mon_origin_y + 8.0)))
        .map_err(|e| format!("定位桌面待办窗口失败: {}", e))?;
    Ok(())
}

/// 等待前端 emit `todo-board-ready`（带 800ms 超时兜底）。
///
/// 与 pet 窗口的 wait_pet_window_ready 同理：窗口以不透明深色主题渲染，
/// webview 加载 HTML 期间会闪默认白底，等前端首帧就绪后再 show 消除闪烁。
async fn wait_todo_board_ready(app: &AppHandle) {
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
    let tx_clone = tx.clone();
    let _listener = app.listen("todo-board-ready", move |_event| {
        if let Some(sender) = tx_clone.lock().ok().and_then(|mut g| g.take()) {
            let _ = sender.send(());
        }
    });
    let _ = tokio::time::timeout(Duration::from_millis(800), rx).await;
}

/// 显示 todo-board 窗口（定位右下角 + 等待首帧就绪 + 显示 + 抢焦点便于立即操作）。
#[tauri::command]
pub async fn show_todo_board_window(app: AppHandle) -> Result<(), String> {
    let window = ensure_todo_board_window(&app)?;
    position_bottom_right(&window)?;
    wait_todo_board_ready(&app).await;
    window
        .show()
        .map_err(|e| format!("显示桌面待办窗口失败: {}", e))?;
    let _ = window.set_focus();
    Ok(())
}

/// 隐藏 todo-board 窗口（取消「固定到桌面」）。
#[tauri::command]
pub fn hide_todo_board_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(TODO_BOARD_LABEL) {
        window
            .hide()
            .map_err(|e| format!("隐藏桌面待办窗口失败: {}", e))?;
    }
    Ok(())
}

/// 切换 todo-board 窗口显隐，返回切换后是否可见。
#[tauri::command]
pub async fn toggle_todo_board_window(app: AppHandle) -> Result<bool, String> {
    let window = ensure_todo_board_window(&app)?;
    let visible = window
        .is_visible()
        .map_err(|e| format!("查询窗口可见性失败: {}", e))?;
    if visible {
        window
            .hide()
            .map_err(|e| format!("隐藏桌面待办窗口失败: {}", e))?;
        Ok(false)
    } else {
        position_bottom_right(&window)?;
        wait_todo_board_ready(&app).await;
        window
            .show()
            .map_err(|e| format!("显示桌面待办窗口失败: {}", e))?;
        let _ = window.set_focus();
        Ok(true)
    }
}

/// 查询 todo-board 窗口当前是否可见（供前端同步固定状态）。
#[tauri::command]
pub fn is_todo_board_visible(app: AppHandle) -> Result<bool, String> {
    match app.get_webview_window(TODO_BOARD_LABEL) {
        Some(w) => w
            .is_visible()
            .map_err(|e| format!("查询窗口可见性失败: {}", e)),
        None => Ok(false),
    }
}
