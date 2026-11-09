//! Monitor de PRs: consulta periódica, origem do token e entrega dos avisos.

use std::collections::HashMap;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::sync::watch;

use super::api::*;
use super::card::*;
use super::gh_cli::*;
use super::model::*;
use crate::model::IdGen;

const POLL_PENDING: Duration = Duration::from_secs(30);
const POLL_IDLE: Duration = Duration::from_secs(180);
const BACKOFF_START: Duration = Duration::from_secs(30);
const BACKOFF_MAX: Duration = Duration::from_secs(600);

// ---------- monitor ----------

/// Liga/desliga o monitor (estado vem da config `github`).
pub struct Monitor {
    enabled: watch::Sender<bool>,
}

impl Monitor {
    pub fn set_enabled(&self, on: bool) {
        self.enabled.send_replace(on);
    }
}

pub fn spawn(app: &AppHandle, enabled: bool) {
    let (tx, rx) = watch::channel(enabled);
    app.manage(Monitor { enabled: tx });
    tauri::async_runtime::spawn(run(app.clone(), rx));
}

/// Avisos do monitor para a interface (o tray traduz e mostra no cartão).
pub enum Report {
    Connected { login: String, open_prs: usize },
    NoGh,
    LoginExpired,
}

async fn run(app: AppHandle, mut enabled: watch::Receiver<bool>) {
    let client = match reqwest::Client::builder()
        .user_agent(concat!(
            "Quacko/",
            env!("CARGO_PKG_VERSION"),
            " (desktop notifier)"
        ))
        .timeout(Duration::from_secs(20))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(%e, "não foi possível criar o cliente HTTP");
            return;
        }
    };

    loop {
        if enabled.wait_for(|on| *on).await.is_err() {
            return;
        }
        tracing::info!("monitor de PRs ligado");
        let mut tracked: HashMap<String, Tracked> = HashMap::new();
        let mut token: Option<String> = None;
        let mut announced = false;
        let mut retried_auth = false;
        // O token atual veio do login por código (Keychain)? Se for recusado, apaga.
        let mut from_store = false;
        let mut backoff = BACKOFF_START;

        while *enabled.borrow() {
            if token.is_none() {
                // 1º login por código (renovado se estiver vencendo); 2º gh.
                let config = app.state::<crate::settings::Settings>().snapshot();
                let auth = device_flow_client_id(&config).and_then(|id| {
                    crate::github::auth::Client::new(crate::github::auth::GITHUB_WEB, &id).ok()
                });
                token = crate::github::auth::usable_token(
                    auth.as_ref(),
                    &crate::github::auth::KeychainStore,
                )
                .await;
                from_store = token.is_some();
            }
            if token.is_none() {
                token = tauri::async_runtime::spawn_blocking(|| {
                    let home = dirs::home_dir()?;
                    gh_token(&find_gh(&home)?)
                })
                .await
                .ok()
                .flatten();
                if token.is_none() {
                    crate::github::ui::github_report(&app, Report::NoGh);
                    break;
                }
            }

            let wait = match fetch(&client, token.as_deref().unwrap_or_default()).await {
                Ok((login, prs)) => {
                    retried_auth = false;
                    backoff = BACKOFF_START;
                    let (next, finished) = diff(&tracked, &prs);
                    tracked = next;
                    if !announced {
                        announced = true;
                        tracing::info!(
                            open_prs = prs.len(),
                            running = any_running(&tracked),
                            "GitHub conectado"
                        );
                        crate::github::ui::github_report(
                            &app,
                            Report::Connected {
                                login,
                                open_prs: prs.len(),
                            },
                        );
                    }
                    deliver(&app, &finished);
                    if any_running(&tracked) {
                        POLL_PENDING
                    } else {
                        POLL_IDLE
                    }
                }
                Err(ApiError::Unauthorized) if !retried_auth => {
                    // Token trocado pelo gh (novo login)? Lê de novo e tenta uma vez.
                    retried_auth = true;
                    token = None;
                    Duration::from_secs(1)
                }
                Err(ApiError::Unauthorized) if from_store => {
                    // Login por código recusado de novo (revogado): apaga e tenta o gh.
                    use crate::github::auth::TokenStore;
                    crate::github::auth::KeychainStore.clear();
                    from_store = false;
                    retried_auth = false;
                    token = None;
                    Duration::from_secs(1)
                }
                Err(ApiError::Unauthorized) => {
                    crate::github::ui::github_report(&app, Report::LoginExpired);
                    break;
                }
                Err(ApiError::RateLimited(wait)) => {
                    tracing::warn!(?wait, "limite de taxa do GitHub");
                    wait
                }
                Err(ApiError::Other(e)) => {
                    tracing::warn!(%e, ?backoff, "falha ao consultar o GitHub");
                    let wait = backoff;
                    backoff = (backoff * 2).min(BACKOFF_MAX);
                    wait
                }
            };

            // Espera, mas acorda na hora se a pessoa desligar o monitor.
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                changed = enabled.changed() => if changed.is_err() { return; },
            }
        }
        tracing::info!("monitor de PRs desligado");
    }
}

/// Mostra no cartão o que mudou nos PRs (respeitando o "Não perturbar"). Só o cartão:
/// sem som e sem pausar música.
fn deliver(app: &AppHandle, changes: &[Change]) {
    let settings = app.state::<crate::settings::Settings>();
    if changes.is_empty() || settings.snapshot().dnd {
        return;
    }
    let lang = settings.lang();
    for change in changes {
        let n = notification(app.state::<IdGen>().next(), change, lang);
        // Resultado final toca o som; o progresso é só o cartão.
        if !matches!(change, Change::Progress(_)) {
            crate::sound::play(n.status);
        }
        tracing::info!(id = n.id, status = ?n.status, "PR mudou");
        crate::overlay::present(app, n);
    }
}

// ---------- login por código (passo 6) ----------

/// Há token do login por código guardado no Keychain?
pub fn stored_token_present() -> bool {
    use crate::github::auth::TokenStore;
    crate::github::auth::KeychainStore.load().is_some()
}

/// Client ID do GitHub App: `QUACKO_GITHUB_CLIENT_ID` vence a config. Sem ele, o login
/// por código fica desligado (só o `gh`).
pub fn device_flow_client_id(config: &crate::config::Config) -> Option<String> {
    std::env::var("QUACKO_GITHUB_CLIENT_ID")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| crate::github::auth::valid_client_id(v))
        .or_else(|| config.github_client_id.clone())
}
