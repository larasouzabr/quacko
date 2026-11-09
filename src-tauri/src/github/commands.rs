//! Comandos do login por código (device flow) chamados pela tela de boas-vindas.

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::settings::Settings;

/// Código em andamento do login por código (entre "começar" e "esperar").
#[derive(Default)]
pub struct PendingDevice(std::sync::Mutex<Option<crate::github::auth::DeviceCode>>);

#[derive(Debug, Clone, Serialize)]
pub struct DevicePrompt {
    pub user_code: String,
    pub verification_uri: String,
}

fn auth_client(app: &AppHandle) -> Result<crate::github::auth::Client, String> {
    let id = crate::github::device_flow_client_id(&app.state::<Settings>().snapshot())
        .ok_or("login por código desligado: falta o Client ID do GitHub App")?;
    crate::github::auth::Client::new(crate::github::auth::GITHUB_WEB, &id)
        .map_err(|e| format!("{e:?}"))
}

/// Pede o código ao GitHub, copia para a área de transferência e abre a página do GitHub.
#[tauri::command]
pub async fn github_device_start(app: AppHandle) -> Result<DevicePrompt, String> {
    let code = auth_client(&app)?
        .start()
        .await
        .map_err(|e| format!("{e:?}"))?;
    crate::clipboard::copy_to_clipboard(&code.user_code);
    if let Some(url) = crate::model::clean_url(&code.verification_uri) {
        use tauri_plugin_opener::OpenerExt;
        let _ = app.opener().open_url(url, None::<&str>);
    }
    let prompt = DevicePrompt {
        user_code: code.user_code.clone(),
        verification_uri: code.verification_uri.clone(),
    };
    *app.state::<PendingDevice>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(code);
    Ok(prompt)
}

/// Espera a pessoa autorizar. "ok", "denied", "expired" ou mensagem de erro.
#[tauri::command]
pub async fn github_device_wait(app: AppHandle) -> Result<String, String> {
    use crate::github::auth::{AuthError, KeychainStore, TokenStore};
    let code = app
        .state::<PendingDevice>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .ok_or("nenhum login em andamento")?;
    match auth_client(&app)?.wait_for_token(&code).await {
        Ok(token) => {
            KeychainStore.save(&token)?;
            app.state::<Settings>().update(|c| c.github = true);
            let monitor = app.state::<crate::github::Monitor>();
            // Reinicia o monitor para ele pegar o login novo.
            monitor.set_enabled(false);
            monitor.set_enabled(true);
            crate::tray::refresh(&app);
            Ok("ok".into())
        }
        Err(AuthError::Denied) => Ok("denied".into()),
        Err(AuthError::Expired) => Ok("expired".into()),
        Err(e) => Err(format!("{e:?}")),
    }
}
