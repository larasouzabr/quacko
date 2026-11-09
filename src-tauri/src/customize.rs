//! Janela "Personalizar": personagem e acessórios (laço), com pré-visualização ao vivo.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const CUSTOMIZE_LABEL: &str = "customize";

/// Abre (ou traz para a frente) a janela de personalização.
pub fn open(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(CUSTOMIZE_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let title = match app.state::<crate::settings::Settings>().lang() {
        crate::i18n::Lang::PtBr => "Personalizar",
        crate::i18n::Lang::En => "Customize",
    };
    let built = WebviewWindowBuilder::new(
        app,
        CUSTOMIZE_LABEL,
        WebviewUrl::App("index.html#customize".into()),
    )
    .title(title)
    .inner_size(600.0, 640.0)
    .resizable(false)
    .center()
    .focused(true)
    .build();
    match built {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(e) => tracing::warn!(%e, "não foi possível abrir Personalizar"),
    }
}
