use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::watch;

use super::*;

const POINTER_POLL: Duration = Duration::from_millis(80);

pub(super) async fn pointer_loop(app: AppHandle, mut visible: watch::Receiver<bool>) {
    loop {
        if visible.wait_for(|v| *v).await.is_err() {
            return;
        }
        let mut last: Option<Option<(i32, i32)>> = None;
        let mut tick = tokio::time::interval(POINTER_POLL);
        while *visible.borrow() {
            tick.tick().await;
            let pointer = pointer_in_overlay(&app);
            if last != Some(pointer) {
                let _ = app.emit_to(OVERLAY_LABEL, "overlay-pointer", pointer);
                last = Some(pointer);
            }
            #[cfg(target_os = "macos")]
            crate::macos::remember_frontmost_app();
        }
        let _ = app.emit_to(OVERLAY_LABEL, "overlay-pointer", None::<(i32, i32)>);
    }
}

pub(super) fn pointer_in_overlay(app: &AppHandle) -> Option<(i32, i32)> {
    let window = app.get_webview_window(OVERLAY_LABEL)?;
    let cursor = window.cursor_position().ok()?;
    let pos = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    let scale = window.scale_factor().ok()?;
    let dx = cursor.x - pos.x as f64;
    let dy = cursor.y - pos.y as f64;
    if dx < 0.0 || dy < 0.0 || dx >= size.width as f64 || dy >= size.height as f64 {
        return None;
    }
    Some(((dx / scale).round() as i32, (dy / scale).round() as i32))
}
