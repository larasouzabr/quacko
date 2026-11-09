//! "Testar notificação": os 4 estados em sequência.

use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::model::{IdGen, Notification, Source, Status};

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
            crate::overlay::present(
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
