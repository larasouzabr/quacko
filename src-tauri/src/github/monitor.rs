use std::collections::HashMap;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::sync::watch;

use super::api::*;
use super::card::*;
use super::gh_cli::*;
use super::model::*;
use super::reviews::{fetch_texts, ReviewWatcher};
use super::runs::{RunChange, RunWatcher};
use crate::model::{IdGen, Notification};

const POLL_PENDING: Duration = Duration::from_secs(30);
const POLL_IDLE: Duration = Duration::from_secs(180);
const BACKOFF_START: Duration = Duration::from_secs(30);
const BACKOFF_MAX: Duration = Duration::from_secs(600);

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
        let mut runs = RunWatcher::new(now_secs());
        let mut reviews = ReviewWatcher::new();
        let mut token: Option<String> = None;
        let mut announced = false;
        let mut retried_auth = false;
        let mut from_store = false;
        let mut backoff = BACKOFF_START;

        while *enabled.borrow() {
            if token.is_none() {
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
                    let run_changes = match runs
                        .poll(
                            &client,
                            token.as_deref().unwrap_or_default(),
                            &login,
                            now_secs(),
                        )
                        .await
                    {
                        Ok(changes) => changes,
                        Err(e) => {
                            tracing::warn!(?e, "falha ao consultar os workflows");
                            Vec::new()
                        }
                    };
                    let candidates = reviews.diff(&prs, &login);
                    let review_events = if candidates.is_empty() {
                        Vec::new()
                    } else {
                        let ids: Vec<String> =
                            candidates.iter().map(|c| c.activity.id.clone()).collect();
                        let texts =
                            fetch_texts(&client, token.as_deref().unwrap_or_default(), &ids)
                                .await
                                .unwrap_or_else(|e| {
                                    tracing::warn!(?e, "falha ao ler os comentários");
                                    HashMap::new()
                                });
                        reviews.resolve(candidates, &texts)
                    };
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
                    deliver(&app, &finished, &run_changes);
                    deliver_reviews(&app, &review_events);
                    if any_running(&tracked) || runs.any_running() {
                        POLL_PENDING
                    } else {
                        POLL_IDLE
                    }
                }
                Err(ApiError::Unauthorized) if !retried_auth => {
                    retried_auth = true;
                    token = None;
                    Duration::from_secs(1)
                }
                Err(ApiError::Unauthorized) if from_store => {
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

            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                changed = enabled.changed() => if changed.is_err() { return; },
            }
        }
        tracing::info!("monitor de PRs desligado");
    }
}

fn deliver(app: &AppHandle, prs: &[Change], runs: &[RunChange]) {
    let settings = app.state::<crate::settings::Settings>();
    if (prs.is_empty() && runs.is_empty()) || !settings.snapshot().notify_ci {
        return;
    }
    let dnd = settings.snapshot().dnd;
    let lang = settings.lang();
    let ids = app.state::<IdGen>();
    let cards: Vec<(Notification, bool)> = prs
        .iter()
        .map(|c| {
            let final_state = !matches!(c, Change::Progress(_));
            (notification(ids.next(), c, lang), final_state)
        })
        .chain(runs.iter().map(|c| {
            let final_state = !matches!(c, RunChange::Progress(_));
            (run_notification(ids.next(), c, lang), final_state)
        }))
        .collect();
    for (n, final_state) in cards {
        if final_state {
            crate::history::remember(app, &n, dnd);
        }
        if dnd {
            continue;
        }
        if final_state {
            crate::sound::play(
                &app.state::<crate::settings::Settings>().snapshot(),
                n.status,
            );
        }
        tracing::info!(id = n.id, status = ?n.status, "CI mudou");
        crate::overlay::present(app, n);
    }
}

fn deliver_reviews(app: &AppHandle, events: &[super::reviews::Event]) {
    let settings = app.state::<crate::settings::Settings>();
    let config = settings.snapshot();
    if events.is_empty() || !config.notify_reviews {
        return;
    }
    let lang = settings.lang();
    let ids = app.state::<IdGen>();
    for event in events {
        let n = super::reviews::notification(ids.next(), event, lang);
        crate::history::remember(app, &n, config.dnd);
        if config.dnd {
            continue;
        }
        crate::sound::play(&config, n.status);
        tracing::info!(id = n.id, status = ?n.status, "review ou comentário novo");
        crate::overlay::present(app, n);
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn stored_token_present() -> bool {
    use crate::github::auth::TokenStore;
    crate::github::auth::KeychainStore.load().is_some()
}

pub const BUILT_IN_CLIENT_ID: Option<&str> = option_env!("QUACKO_BUILD_GITHUB_CLIENT_ID");

pub fn device_flow_client_id(config: &crate::config::Config) -> Option<String> {
    resolve_client_id(
        std::env::var("QUACKO_GITHUB_CLIENT_ID").ok().as_deref(),
        config.github_client_id.as_deref(),
        BUILT_IN_CLIENT_ID,
    )
}

pub fn resolve_client_id(
    env: Option<&str>,
    config: Option<&str>,
    built_in: Option<&str>,
) -> Option<String> {
    [env, config, built_in]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|v| crate::github::auth::valid_client_id(v))
        .map(String::from)
}
