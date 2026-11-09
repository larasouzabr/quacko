use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager};

use super::*;

#[tauri::command]
pub fn overlay_empty(app: AppHandle, last_seen_id: u64) {
    let state = app.state::<OverlayState>();
    let _guard = state.lock.lock().unwrap_or_else(|e| e.into_inner());
    if state.last_delivered.load(Ordering::SeqCst) > last_seen_id {
        return;
    }
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.set_ignore_cursor_events(true);
        let _ = window.hide();
    }
    state.visible.send_replace(false);
    crate::tray::icon::set_alert(&app, false);
}

#[tauri::command]
pub fn tray_alert_cleared(app: AppHandle) {
    crate::tray::icon::set_alert(&app, false);
}

#[tauri::command]
pub fn set_overlay_size(app: AppHandle, width: f64, height: f64) {
    if !width.is_finite() || !height.is_finite() {
        return;
    }
    let width = width.clamp(MIN_SIZE.0, MAX_SIZE.0);
    let height = height.clamp(MIN_SIZE.1, MAX_SIZE.1);
    let state = app.state::<OverlayState>();
    let _guard = state.lock.lock().unwrap_or_else(|e| e.into_inner());
    *state.size.lock().unwrap_or_else(|e| e.into_inner()) = (width, height);

    let Some(window) = app.get_webview_window(OVERLAY_LABEL) else {
        return;
    };
    if let Err(e) = window.set_size(tauri::LogicalSize::new(width, height)) {
        tracing::warn!(%e, "não foi possível redimensionar o overlay");
        return;
    }
    if window.is_visible().unwrap_or(false) {
        if let Ok(Some(monitor)) = window.current_monitor() {
            let _ = super::position::place_top_right(&window, &monitor, width);
        }
    }
}

#[tauri::command]
pub fn set_interactive(app: AppHandle, interactive: bool) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.set_ignore_cursor_events(!interactive);
    }
}

#[tauri::command]
pub fn card_action(app: AppHandle, url: Option<String>) {
    let Some(url) = url.as_deref().and_then(crate::model::clean_url) else {
        #[cfg(target_os = "macos")]
        let _ = app.run_on_main_thread(crate::macos::restore_frontmost_app);
        return;
    };
    use tauri_plugin_opener::OpenerExt;
    if let Err(e) = app.opener().open_url(url, None::<&str>) {
        tracing::warn!(%e, "não foi possível abrir o link");
    }
}
