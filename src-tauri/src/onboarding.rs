//! Primeira execução: janela de boas-vindas com 3 passos (Conectar Claude Code,
//! Conectar GitHub, Testar notificação) e o status de cada um. Abre sozinha só na
//! primeira vez (config `onboarded`) e depois pelo menu.

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::integrations::{self, Outcome, Paths, Tool};
use crate::settings::Settings;

pub const WELCOME_LABEL: &str = "welcome";

/// Situação do GitHub, do melhor para o pior caso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GithubState {
    /// Monitor ligado e com login.
    On,
    /// Há login (gh ou código), mas o monitor está desligado.
    Ready,
    /// O gh existe, mas não está logado.
    GhLoggedOut,
    /// Sem gh e sem login por código.
    NoLogin,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub claude_connected: bool,
    pub cursor_present: bool,
    pub cursor_connected: bool,
    pub github: GithubState,
    /// Login por código (GitHub App) disponível: só com Client ID configurado.
    pub github_device_flow: bool,
    pub nowplaying: bool,
}

pub fn github_state(enabled: bool, gh_found: bool, has_token: bool) -> GithubState {
    match (has_token, enabled, gh_found) {
        (true, true, _) => GithubState::On,
        (true, false, _) => GithubState::Ready,
        (false, _, true) => GithubState::GhLoggedOut,
        (false, _, false) => GithubState::NoLogin,
    }
}

fn collect(app: &AppHandle) -> Status {
    let paths = Paths::from_home();
    let connected = |tool| {
        paths
            .as_ref()
            .is_some_and(|p| integrations::status(p, tool))
    };
    let home = paths.as_ref().map(|p| p.home.clone()).unwrap_or_default();
    let gh = crate::github::find_gh(&home);
    let has_token = crate::github::stored_token_present()
        || gh.as_deref().and_then(crate::github::gh_token).is_some();
    let config = app.state::<Settings>().snapshot();
    Status {
        claude_connected: connected(Tool::Claude),
        cursor_present: paths.as_ref().is_some_and(Paths::cursor_present),
        cursor_connected: connected(Tool::Cursor),
        github: github_state(config.github, gh.is_some(), has_token),
        github_device_flow: crate::github::device_flow_client_id(&config).is_some(),
        nowplaying: crate::media::find_nowplaying_cli(&home).is_some(),
    }
}

#[tauri::command]
pub async fn onboarding_status(app: AppHandle) -> Result<Status, String> {
    tauri::async_runtime::spawn_blocking(move || collect(&app))
        .await
        .map_err(|e| e.to_string())
}

/// Conecta o Claude Code. Devolve "ok" ou o motivo do modo manual (o trecho já foi
/// copiado para a área de transferência pelo mesmo caminho do menu).
#[tauri::command]
pub async fn onboarding_connect_claude(app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let paths = Paths::from_home().ok_or("pasta pessoal não encontrada")?;
        match integrations::set_connected(&paths, Tool::Claude, true).map_err(|e| e.to_string())? {
            Outcome::Manual { snippet, .. } => {
                crate::tray::copy_to_clipboard(&snippet);
                Ok("manual".to_string())
            }
            _ => {
                crate::tray::refresh(&app);
                Ok("ok".to_string())
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn onboarding_enable_github(app: AppHandle) {
    app.state::<Settings>().update(|c| c.github = true);
    app.state::<crate::github::Monitor>().set_enabled(true);
    crate::tray::refresh(&app);
}

/// Código em andamento do login por código (entre "começar" e "esperar").
#[derive(Default)]
pub struct PendingDevice(std::sync::Mutex<Option<crate::github_auth::DeviceCode>>);

#[derive(Debug, Clone, Serialize)]
pub struct DevicePrompt {
    pub user_code: String,
    pub verification_uri: String,
}

fn auth_client(app: &AppHandle) -> Result<crate::github_auth::Client, String> {
    let id = crate::github::device_flow_client_id(&app.state::<Settings>().snapshot())
        .ok_or("login por código desligado: falta o Client ID do GitHub App")?;
    crate::github_auth::Client::new(crate::github_auth::GITHUB_WEB, &id)
        .map_err(|e| format!("{e:?}"))
}

/// Pede o código ao GitHub, copia para a área de transferência e abre a página do GitHub.
#[tauri::command]
pub async fn github_device_start(app: AppHandle) -> Result<DevicePrompt, String> {
    let code = auth_client(&app)?
        .start()
        .await
        .map_err(|e| format!("{e:?}"))?;
    crate::tray::copy_to_clipboard(&code.user_code);
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
    use crate::github_auth::{AuthError, KeychainStore, TokenStore};
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

#[tauri::command]
pub fn onboarding_test(app: AppHandle) {
    crate::window::run_test_sequence(&app);
}

#[tauri::command]
pub fn onboarding_finish(app: AppHandle) {
    mark_done(&app);
    if let Some(window) = app.get_webview_window(WELCOME_LABEL) {
        let _ = window.close();
    }
}

pub fn mark_done(app: &AppHandle) {
    let settings = app.state::<Settings>();
    if !settings.snapshot().onboarded {
        settings.update(|c| c.onboarded = true);
    }
}

/// Abre (ou traz para a frente) a janela de boas-vindas.
pub fn open(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WELCOME_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let title = match app.state::<Settings>().lang() {
        crate::i18n::Lang::PtBr => "Bem-vindo ao Quacko",
        crate::i18n::Lang::En => "Welcome to Quacko",
    };
    let built = WebviewWindowBuilder::new(
        app,
        WELCOME_LABEL,
        WebviewUrl::App("index.html#welcome".into()),
    )
    .title(title)
    .inner_size(560.0, 700.0)
    .resizable(false)
    .center()
    .focused(true)
    .build();
    match built {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(e) => tracing::warn!(%e, "não foi possível abrir as boas-vindas"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estado_do_github() {
        assert_eq!(github_state(true, true, true), GithubState::On);
        assert_eq!(
            github_state(true, false, true),
            GithubState::On,
            "login por código, sem gh"
        );
        assert_eq!(github_state(false, true, true), GithubState::Ready);
        assert_eq!(github_state(true, true, false), GithubState::GhLoggedOut);
        assert_eq!(github_state(false, false, false), GithubState::NoLogin);
    }

    #[test]
    fn serializa_como_o_frontend_espera() {
        assert_eq!(
            serde_json::to_value(GithubState::GhLoggedOut).unwrap(),
            "gh_logged_out"
        );
        assert_eq!(
            serde_json::to_value(GithubState::NoLogin).unwrap(),
            "no_login"
        );
    }
}
