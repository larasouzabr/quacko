//! Janela overlay: criada uma vez no start, escondida até haver cartão.
//!
//! - Mostra sem pegar foco (`focusable(false)`), no canto superior direito do monitor
//!   onde está o cursor, logo abaixo da barra de menus (área de trabalho do monitor).
//! - Enquanto visível, o Rust acompanha o mouse e manda a posição para o frontend
//!   (evento `overlay-pointer`): o WebKit não entrega hover numa janela fora de foco.
//! - A janela ignora cliques (eles passam para o app de baixo) até o mouse estar
//!   sobre um cartão; o frontend avisa isso com `set_interactive`.

pub mod commands;
pub mod pointer;
pub mod position;

use pointer::pointer_loop;
use position::move_to_cursor_monitor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tokio::sync::watch;

use crate::model::Notification;

pub const OVERLAY_LABEL: &str = "overlay";
pub const OVERLAY_WIDTH: f64 = 380.0;
pub const OVERLAY_HEIGHT: f64 = 330.0;
const SCREEN_MARGIN: f64 = 24.0;
/// Distância da barra de menus (a pilha ainda tem 8 px de respiro interno).
const TOP_MARGIN: f64 = 8.0;

/// Limites do tamanho pedido pelo frontend (px lógicos).
const MIN_SIZE: (f64, f64) = (200.0, 150.0);
const MAX_SIZE: (f64, f64) = (1200.0, 1600.0);

pub struct OverlayState {
    /// Serializa "mostrar" e "esconder" para um aviso novo não ser escondido por engano.
    lock: Mutex<()>,
    last_delivered: AtomicU64,
    visible: watch::Sender<bool>,
    /// Tamanho atual (px lógicos): muda com o tamanho do pixel e o limite de cartões.
    size: Mutex<(f64, f64)>,
}

impl OverlayState {
    fn size(&self) -> (f64, f64) {
        *self.size.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub fn create_overlay(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window =
        WebviewWindowBuilder::new(app, OVERLAY_LABEL, WebviewUrl::App("index.html".into()))
            .title("Quacko")
            .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .resizable(false)
            .visible(false)
            // Nunca vira a janela "key": mostrar o overlay não tira o foco do app em uso.
            .focused(false)
            .focusable(false)
            // O primeiro clique já chega ao cartão, mesmo sem a janela ter foco.
            .accept_first_mouse(true)
            .visible_on_all_workspaces(true)
            .build()?;

    window.set_ignore_cursor_events(true)?;
    #[cfg(target_os = "macos")]
    crate::macos::join_all_spaces_and_fullscreen(&window);

    let (visible_tx, visible_rx) = watch::channel(false);
    app.manage(OverlayState {
        lock: Mutex::new(()),
        last_delivered: AtomicU64::new(0),
        visible: visible_tx,
        size: Mutex::new((OVERLAY_WIDTH, OVERLAY_HEIGHT)),
    });
    tauri::async_runtime::spawn(pointer_loop(app.clone(), visible_rx));
    Ok(window)
}

/// Mostra o overlay (se escondido) e entrega o aviso ao frontend.
pub fn present(app: &AppHandle, notification: Notification) {
    let Some(window) = app.get_webview_window(OVERLAY_LABEL) else {
        return;
    };
    let state = app.state::<OverlayState>();
    let _guard = state.lock.lock().unwrap_or_else(|e| e.into_inner());
    state
        .last_delivered
        .store(notification.id, Ordering::SeqCst);
    if crate::tray::icon::is_alert(&notification) {
        crate::tray::icon::set_alert(app, true);
    }

    if !window.is_visible().unwrap_or(false) {
        #[cfg(target_os = "macos")]
        crate::macos::remember_frontmost_app();
        if let Err(e) = move_to_cursor_monitor(&window, state.size().0) {
            tracing::warn!(%e, "não foi possível posicionar o overlay");
        }
        if let Err(e) = window.show() {
            tracing::warn!(%e, "não foi possível mostrar o overlay");
        }
        state.visible.send_replace(true);
    }

    if let Err(e) = app.emit_to(OVERLAY_LABEL, "notify", notification) {
        tracing::warn!(%e, "falha ao enviar aviso ao overlay");
    }
}
