//! Janela overlay: criada uma vez no start, escondida até haver cartão.
//!
//! - Mostra sem pegar foco (`focusable(false)`), no canto superior direito do monitor
//!   onde está o cursor, logo abaixo da barra de menus (área de trabalho do monitor).
//! - Enquanto visível, o Rust acompanha o mouse e manda a posição para o frontend
//!   (evento `overlay-pointer`): o WebKit não entrega hover numa janela fora de foco.
//! - A janela ignora cliques (eles passam para o app de baixo) até o mouse estar
//!   sobre um cartão; o frontend avisa isso com `set_interactive`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
use tokio::sync::watch;

use crate::model::{IdGen, Notification, Source, Status};

pub const OVERLAY_LABEL: &str = "overlay";
pub const OVERLAY_WIDTH: f64 = 380.0;
pub const OVERLAY_HEIGHT: f64 = 330.0;
const SCREEN_MARGIN: f64 = 24.0;
/// Distância da barra de menus (a pilha ainda tem 8 px de respiro interno).
const TOP_MARGIN: f64 = 8.0;
const POINTER_POLL: Duration = Duration::from_millis(80);

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
    if crate::tray_icon::is_alert(&notification) {
        crate::tray_icon::set_alert(app, true);
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

/// Canto superior direito da área útil do monitor onde está o cursor.
fn move_to_cursor_monitor<R: Runtime>(
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

fn place_top_right<R: Runtime>(
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

/// Enquanto o overlay está visível, manda a posição do mouse (px CSS relativos à janela).
async fn pointer_loop(app: AppHandle, mut visible: watch::Receiver<bool>) {
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

fn pointer_in_overlay(app: &AppHandle) -> Option<(i32, i32)> {
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

/// "Testar notificação": um aviso de cada estado, 2,5 s um do outro.
/// Aparece mesmo com "Não perturbar" ligado, porque é um pedido explícito.
pub fn run_test_sequence(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let title =
            crate::i18n::tray_texts(app.state::<crate::settings::Settings>().lang()).test_title;
        for (i, status) in Status::ALL.into_iter().enumerate() {
            if i > 0 {
                tokio::time::sleep(Duration::from_millis(2500)).await;
            }
            let id = app.state::<IdGen>().next();
            crate::sound::play(status);
            present(
                &app,
                Notification {
                    id,
                    source: Source::Custom,
                    status,
                    // Sem mensagem: mostra o texto padrão do idioma atual.
                    message: None,
                    title: Some(format!("{title} ({})", status.as_str())),
                    project: Some(format!("teste-{}", status.as_str())),
                    url: None,
                    ttl_ms: 7000,
                    progress: None,
                    sticky: false,
                },
            );
        }
    });
}

// ---------- comandos chamados pelo frontend ----------

/// A fila esvaziou. `last_seen_id` evita esconder a janela se um aviso novo já chegou.
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
    crate::tray_icon::set_alert(&app, false);
}

/// Os avisos de alerta sumiram da tela (dispensados ou expirados): o ícone volta ao normal.
#[tauri::command]
pub fn tray_alert_cleared(app: AppHandle) {
    crate::tray_icon::set_alert(&app, false);
}

/// Mouse sobre um cartão: a janela passa a receber cliques. Fora: cliques atravessam.
/// O frontend pede o tamanho exato para o personagem e o limite de cartões atuais.
/// Com a janela visível, ela continua presa ao canto superior direito do monitor atual.
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
            let _ = place_top_right(&window, &monitor, width);
        }
    }
}

#[tauri::command]
pub fn set_interactive(app: AppHandle, interactive: bool) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.set_ignore_cursor_events(!interactive);
    }
}

/// Clique num cartão. Com `url` http(s) válida, abre no navegador; senão devolve o foco
/// ao app que estava em uso (o clique pode ter ativado o Quacko).
#[tauri::command]
pub fn card_action(app: AppHandle, url: Option<String>) {
    if let Some(url) = url.as_deref().and_then(crate::model::clean_url) {
        use tauri_plugin_opener::OpenerExt;
        if let Err(e) = app.opener().open_url(url, None::<&str>) {
            tracing::warn!(%e, "não foi possível abrir o link");
        }
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let _ = app.run_on_main_thread(crate::macos::restore_frontmost_app);
    }
}
