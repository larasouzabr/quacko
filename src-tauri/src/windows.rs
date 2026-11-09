//! Janelas secundárias (boas-vindas, Personalizar): abre uma vez e depois só traz para a frente.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub fn open_or_focus(app: &AppHandle, label: &str, route: &str, title: &str, size: (f64, f64)) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App(format!("index.html#{route}").into()),
    )
    .title(title)
    .inner_size(size.0, size.1)
    .resizable(false)
    .center()
    .focused(true)
    .build();
    match built {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(e) => tracing::warn!(%e, label, "não foi possível abrir a janela"),
    }
}
