//! Janela "Personalizar": personagem e acessórios (laço), com pré-visualização ao vivo.

use tauri::{AppHandle, Manager};

pub const CUSTOMIZE_LABEL: &str = "customize";

pub fn open(app: &AppHandle) {
    let title = match app.state::<crate::settings::Settings>().lang() {
        crate::i18n::Lang::PtBr => "Personalizar",
        crate::i18n::Lang::En => "Customize",
    };
    crate::windows::open_or_focus(app, CUSTOMIZE_LABEL, "customize", title, (600.0, 640.0));
}
