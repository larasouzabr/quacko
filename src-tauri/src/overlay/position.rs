use tauri::{PhysicalPosition, Runtime, WebviewWindow};

use super::*;

pub(super) fn move_to_cursor_monitor<R: Runtime>(
    window: &WebviewWindow<R>,
    logical_width: f64,
) -> tauri::Result<()> {
    let cursor = window.cursor_position()?;
    let monitors = window.available_monitors()?;
    let monitor = monitors
        .iter()
        .find(|m| {
            let (pos, size) = (m.position(), m.size());
            cursor.x >= pos.x as f64
                && cursor.x < pos.x as f64 + size.width as f64
                && cursor.y >= pos.y as f64
                && cursor.y < pos.y as f64 + size.height as f64
        })
        .cloned()
        .or(window.primary_monitor()?);
    match monitor {
        Some(monitor) => place_top_right(window, &monitor, logical_width),
        None => Ok(()),
    }
}

pub(super) fn place_top_right<R: Runtime>(
    window: &WebviewWindow<R>,
    monitor: &tauri::Monitor,
    logical_width: f64,
) -> tauri::Result<()> {
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let width = (logical_width * scale).round() as i32;
    let side_margin = (SCREEN_MARGIN * scale).round() as i32;
    let top_margin = (TOP_MARGIN * scale).round() as i32;
    let x = area.position.x + area.size.width as i32 - width - side_margin;
    let y = area.position.y + top_margin;
    window.set_position(PhysicalPosition::new(x, y))
}
