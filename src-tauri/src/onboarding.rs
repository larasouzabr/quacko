use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::integrations::{self, Outcome, Paths, Tool};
use crate::settings::Settings;

pub const WELCOME_LABEL: &str = "welcome";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GithubState {
    On,
    Ready,
    GhLoggedOut,
    NoLogin,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub claude_connected: bool,
    pub cursor_present: bool,
    pub cursor_connected: bool,
    pub github: GithubState,
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
        nowplaying: !cfg!(target_os = "macos")
            || crate::media::find_nowplaying_cli(&home).is_some(),
    }
}

#[tauri::command]
pub async fn onboarding_status(app: AppHandle) -> Result<Status, String> {
    tauri::async_runtime::spawn_blocking(move || collect(&app))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn onboarding_connect_claude(app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let paths = Paths::from_home().ok_or("pasta pessoal não encontrada")?;
        match integrations::set_connected(&paths, Tool::Claude, true).map_err(|e| e.to_string())? {
            Outcome::Manual { snippet, .. } => {
                crate::clipboard::copy_to_clipboard(&snippet);
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

#[tauri::command]
pub fn onboarding_test(app: AppHandle) {
    crate::test_sequence::run_test_sequence(&app);
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

pub fn open(app: &AppHandle) {
    let title = match app.state::<Settings>().lang() {
        crate::i18n::Lang::PtBr => "Bem-vindo ao Quacko",
        crate::i18n::Lang::En => "Welcome to Quacko",
    };
    crate::windows::open_or_focus(app, WELCOME_LABEL, "welcome", title, (560.0, 700.0));
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
