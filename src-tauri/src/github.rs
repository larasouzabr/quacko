//! Monitor de PRs do GitHub (spec v2, seção 6.2), passo 1: usa o login do `gh`.
//!
//! - Uma consulta GraphQL traz todos os PRs abertos da pessoa com o resumo dos checks
//!   do último commit (`statusCheckRollup`: check runs + status de commit, o que inclui
//!   o Jenkins), com a contagem por estado. Custa 1 ponto do limite de taxa.
//! - Progresso: quando mais checks terminam, o cartão do PR é atualizado ("3 de 12").
//!   Final: quando todos terminaram e o resumo não está mais pendente. Sempre para o
//!   mesmo commit; PR visto pela primeira vez não avisa; push novo reinicia.
//! - Progresso é só o cartão; o resultado final toca o som (Glass/Basso). Música não é pausada.
//! - Consulta a cada 30 s enquanto algum PR está rodando, a cada 3 min quando tudo
//!   terminou; recua em erro de rede e respeita o limite de taxa.
//! - O token vem de `gh auth token` (o app procura o `gh` sozinho, porque um app aberto
//!   pelo Finder não tem o PATH do terminal). O passo 2 (GitHub App + login por código)
//!   troca só a origem do token.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio::sync::watch;

use crate::i18n::Lang;
use crate::model::{IdGen, Notification, Progress, Source, Status};

const GRAPHQL_URL: &str = "https://api.github.com/graphql";
const SEARCH: &str = "is:pr is:open author:@me archived:false";
const QUERY: &str = r#"query($q: String!) {
  viewer { login }
  search(query: $q, type: ISSUE, first: 50) {
    nodes {
      ... on PullRequest {
        number title url
        repository { nameWithOwner }
        commits(last: 1) { nodes { commit { oid statusCheckRollup {
          state
          contexts(first: 0) {
            checkRunCountsByState { state count }
            statusContextCountsByState { state count }
          }
        } } } }
      }
    }
  }
}"#;

/// O título do PR é o que mais ajuda a reconhecê-lo: vai além do limite da API local (60).
pub const MAX_PR_TITLE_CHARS: usize = 120;

const POLL_PENDING: Duration = Duration::from_secs(30);
const POLL_IDLE: Duration = Duration::from_secs(180);
const BACKOFF_START: Duration = Duration::from_secs(30);
const BACKOFF_MAX: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Rollup {
    Expected,
    Pending,
    Success,
    Failure,
    Error,
}

/// Contagem dos checks do último commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Checks {
    pub done: u32,
    pub total: u32,
    pub failed: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pr {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub repo: String,
    pub head: String,
    /// `None` = PR sem checks.
    pub rollup: Option<Rollup>,
    pub checks: Checks,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tracked {
    head: String,
    rollup: Option<Rollup>,
    checks: Checks,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// Mais checks terminaram, mas ainda falta algum.
    Progress(Pr),
    Passed(Pr),
    Failed(Pr),
}

fn running(r: Option<Rollup>) -> bool {
    matches!(r, Some(Rollup::Pending | Rollup::Expected))
}

/// Todos os checks terminaram e o resumo já tem resultado.
fn is_finished(rollup: Option<Rollup>, checks: Checks) -> bool {
    checks.total > 0 && checks.done >= checks.total && rollup.is_some() && !running(rollup)
}

/// Compara a leitura nova com a anterior. Devolve o novo estado e o que mudou.
pub fn diff(
    prev: &HashMap<String, Tracked>,
    now: &[Pr],
) -> (HashMap<String, Tracked>, Vec<Change>) {
    let mut next = HashMap::new();
    let mut changes = Vec::new();
    for pr in now {
        if let Some(old) = prev.get(&pr.url).filter(|old| old.head == pr.head) {
            let was = is_finished(old.rollup, old.checks);
            let is = is_finished(pr.rollup, pr.checks);
            if is && !was {
                let failed = pr.checks.failed > 0
                    || matches!(pr.rollup, Some(Rollup::Failure | Rollup::Error));
                changes.push(if failed {
                    Change::Failed(pr.clone())
                } else {
                    Change::Passed(pr.clone())
                });
            } else if !is && pr.checks.done > old.checks.done {
                changes.push(Change::Progress(pr.clone()));
            }
        }
        next.insert(
            pr.url.clone(),
            Tracked {
                head: pr.head.clone(),
                rollup: pr.rollup,
                checks: pr.checks,
            },
        );
    }
    (next, changes)
}

/// Algum PR com checks rodando (consulta mais frequente).
pub fn any_running(state: &HashMap<String, Tracked>) -> bool {
    state
        .values()
        .any(|t| running(t.rollup) || (t.checks.total > 0 && !is_finished(t.rollup, t.checks)))
}

#[derive(Debug)]
pub enum ApiError {
    /// Token inválido ou expirado.
    Unauthorized,
    /// Limite de taxa: esperar esse tempo.
    RateLimited(Duration),
    /// Rede, servidor ou resposta inesperada.
    Other(String),
}

/// Lê a resposta da consulta: login de quem está logado e os PRs abertos.
pub fn parse(body: &Value) -> Result<(String, Vec<Pr>), ApiError> {
    if let Some(errors) = body.get("errors").and_then(Value::as_array) {
        let msg = errors
            .iter()
            .filter_map(|e| e.get("message").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(ApiError::Other(format!("GraphQL: {msg}")));
    }
    let data = body
        .get("data")
        .ok_or_else(|| ApiError::Other("resposta sem data".into()))?;
    let login = data["viewer"]["login"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let prs = data["search"]["nodes"]
        .as_array()
        .map(|nodes| nodes.iter().filter_map(parse_pr).collect())
        .unwrap_or_default();
    Ok((login, prs))
}

fn parse_pr(node: &Value) -> Option<Pr> {
    let commit = &node["commits"]["nodes"][0]["commit"];
    let rollup = &commit["statusCheckRollup"];
    Some(Pr {
        number: node["number"].as_u64()?,
        title: node["title"].as_str().unwrap_or_default().to_string(),
        url: node["url"].as_str()?.to_string(),
        repo: node["repository"]["nameWithOwner"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        head: commit["oid"].as_str()?.to_string(),
        rollup: serde_json::from_value(rollup["state"].clone()).ok(),
        checks: count_checks(&rollup["contexts"]),
    })
}

/// Soma as contagens por estado. Terminado = qualquer estado que não seja de espera.
fn count_checks(contexts: &Value) -> Checks {
    let mut c = Checks::default();
    let mut add = |list: &Value, waiting: &[&str], failed: &[&str]| {
        for entry in list.as_array().into_iter().flatten() {
            let n = entry["count"].as_u64().unwrap_or(0) as u32;
            let state = entry["state"].as_str().unwrap_or_default();
            c.total += n;
            if !waiting.contains(&state) {
                c.done += n;
            }
            if failed.contains(&state) {
                c.failed += n;
            }
        }
    };
    add(
        &contexts["checkRunCountsByState"],
        &["QUEUED", "IN_PROGRESS", "WAITING", "PENDING", "REQUESTED"],
        &[
            "FAILURE",
            "TIMED_OUT",
            "STARTUP_FAILURE",
            "CANCELLED",
            "ACTION_REQUIRED",
        ],
    );
    add(
        &contexts["statusContextCountsByState"],
        &["PENDING", "EXPECTED"],
        &["FAILURE", "ERROR"],
    );
    c
}

fn checks_text(lang: Lang, change: &Change, c: Checks) -> String {
    let (done, total, failed) = (c.done, c.total, c.failed);
    match (lang, change) {
        (Lang::PtBr, Change::Progress(_)) => {
            let tail = match failed {
                0 => String::new(),
                1 => " · 1 falhou".into(),
                n => format!(" · {n} falharam"),
            };
            format!("{done} de {total} checks concluídos{tail}")
        }
        (Lang::PtBr, Change::Passed(_)) if total == 1 => "1 check passou".into(),
        (Lang::PtBr, Change::Passed(_)) => format!("{total} checks passaram"),
        (Lang::PtBr, Change::Failed(_)) if failed == 0 => "Os checks falharam".into(),
        (Lang::PtBr, Change::Failed(_)) if failed == 1 => format!("1 de {total} checks falhou"),
        (Lang::PtBr, Change::Failed(_)) => format!("{failed} de {total} checks falharam"),
        (Lang::En, Change::Progress(_)) => {
            let tail = if failed > 0 {
                format!(" · {failed} failed")
            } else {
                String::new()
            };
            format!("{done} of {total} checks done{tail}")
        }
        (Lang::En, Change::Passed(_)) if total == 1 => "1 check passed".into(),
        (Lang::En, Change::Passed(_)) => format!("All {total} checks passed"),
        (Lang::En, Change::Failed(_)) if failed == 0 => "Checks failed".into(),
        (Lang::En, Change::Failed(_)) => format!("{failed} of {total} checks failed"),
    }
}

/// O cartão de um PR: título "PR #N · título", contagem na mensagem e barra de progresso.
pub fn notification(id: u64, change: &Change, lang: Lang) -> Notification {
    let (status, pr, ttl_ms) = match change {
        Change::Progress(pr) if pr.checks.failed > 0 => (Status::Fail, pr, 6_000),
        Change::Progress(pr) => (Status::Info, pr, 6_000),
        Change::Passed(pr) => (Status::Ok, pr, 8_000),
        Change::Failed(pr) => (Status::Fail, pr, 15_000),
    };
    let clip = |s: &str, max: usize| -> String { s.chars().take(max).collect() };
    let title = if pr.title.trim().is_empty() {
        format!("PR #{}", pr.number)
    } else {
        format!("PR #{} · {}", pr.number, pr.title.trim())
    };
    Notification {
        id,
        source: Source::Ci,
        status,
        // Título do PR inteiro (até 120): o cartão mostra 2 linhas e o resto ao passar o mouse.
        title: Some(clip(&title, MAX_PR_TITLE_CHARS)),
        message: Some(checks_text(lang, change, pr.checks)),
        // dono/repo#N: dois PRs do mesmo repo não se substituem; o mesmo PR se atualiza.
        project: Some(clip(
            &format!("{}#{}", pr.repo, pr.number),
            crate::model::MAX_PROJECT_CHARS,
        )),
        url: crate::model::clean_url(&pr.url),
        ttl_ms,
        progress: (pr.checks.total > 0).then_some(Progress {
            done: pr.checks.done.min(pr.checks.total),
            total: pr.checks.total,
            failed: pr.checks.failed.min(pr.checks.total),
        }),
        // O resultado final fica até a pessoa fechar no ×; o progresso some sozinho.
        sticky: !matches!(change, Change::Progress(_)),
    }
}

// ---------- login por código (passo 6) ----------

/// Há token do login por código guardado no Keychain?
pub fn stored_token_present() -> bool {
    use crate::github_auth::TokenStore;
    crate::github_auth::KeychainStore.load().is_some()
}

/// Client ID do GitHub App: `QUACKO_GITHUB_CLIENT_ID` vence a config. Sem ele, o login
/// por código fica desligado (só o `gh`).
pub fn device_flow_client_id(config: &crate::config::Config) -> Option<String> {
    std::env::var("QUACKO_GITHUB_CLIENT_ID")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| crate::github_auth::valid_client_id(v))
        .or_else(|| config.github_client_id.clone())
}

// ---------- token via gh ----------

/// Procura o `gh`: lugares comuns, o shell de login da pessoa e, por último, o Nix.
pub fn find_gh(home: &Path) -> Option<PathBuf> {
    if let Some(custom) = std::env::var_os("QUACKO_GH").map(PathBuf::from) {
        if custom.is_file() {
            return Some(custom);
        }
    }
    let user = std::env::var("USER").unwrap_or_default();
    let candidates = [
        PathBuf::from("/opt/homebrew/bin/gh"),
        PathBuf::from("/usr/local/bin/gh"),
        home.join(".nix-profile/bin/gh"),
        PathBuf::from(format!("/etc/profiles/per-user/{user}/bin/gh")),
        PathBuf::from("/run/current-system/sw/bin/gh"),
        home.join(".local/bin/gh"),
    ];
    if let Some(found) = candidates.into_iter().find(|p| p.is_file()) {
        return Some(found);
    }
    from_login_shell().or_else(newest_in_nix_store)
}

fn from_login_shell() -> Option<PathBuf> {
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/zsh".into());
    let out = run_with_timeout(
        Command::new(shell).args(["-l", "-i", "-c", "command -v gh"]),
        Duration::from_secs(4),
    )?;
    out.lines()
        .rev()
        .map(str::trim)
        .find(|l| l.starts_with('/'))
        .map(PathBuf::from)
        .filter(|p| p.is_file())
}

/// `/nix/store/<hash>-gh-<versão>/bin/gh` com a maior versão.
fn newest_in_nix_store() -> Option<PathBuf> {
    let entries = std::fs::read_dir("/nix/store").ok()?;
    entries
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|name| {
            let version = name.split_once("-gh-")?.1;
            let parts: Vec<u64> = version
                .split('.')
                .map(|p| p.parse().ok())
                .collect::<Option<_>>()?;
            let bin = Path::new("/nix/store").join(&name).join("bin/gh");
            bin.is_file().then_some((parts, bin))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, bin)| bin)
}

pub fn gh_token(gh: &Path) -> Option<String> {
    let out = run_with_timeout(
        Command::new(gh).args(["auth", "token", "--hostname", "github.com"]),
        Duration::from_secs(5),
    )?;
    let token = out.trim().to_string();
    (!token.is_empty() && !token.contains(char::is_whitespace)).then_some(token)
}

/// Roda um comando com tempo máximo; devolve o stdout se deu certo.
fn run_with_timeout(cmd: &mut Command, limit: Duration) -> Option<String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if start.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
        }
    }
    let mut out = String::new();
    use std::io::Read;
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    Some(out)
}

// ---------- rede ----------

async fn fetch(client: &reqwest::Client, token: &str) -> Result<(String, Vec<Pr>), ApiError> {
    let response = client
        .post(GRAPHQL_URL)
        .bearer_auth(token)
        .json(&json!({ "query": QUERY, "variables": { "q": SEARCH } }))
        .send()
        .await
        .map_err(|e| ApiError::Other(e.to_string()))?;
    let status = response.status();
    let retry_after = response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let remaining_zero = response
        .headers()
        .get("x-ratelimit-remaining")
        .is_some_and(|v| v.as_bytes() == b"0");
    match status.as_u16() {
        401 => return Err(ApiError::Unauthorized),
        403 | 429 if retry_after.is_some() || remaining_zero => {
            return Err(ApiError::RateLimited(Duration::from_secs(
                retry_after.unwrap_or(600).max(60),
            )));
        }
        s if !(200..300).contains(&s) => return Err(ApiError::Other(format!("HTTP {s}"))),
        _ => {}
    }
    let body: Value = response
        .json()
        .await
        .map_err(|e| ApiError::Other(e.to_string()))?;
    parse(&body)
}

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
                    crate::github_auth::Client::new(crate::github_auth::GITHUB_WEB, &id).ok()
                });
                token = crate::github_auth::usable_token(
                    auth.as_ref(),
                    &crate::github_auth::KeychainStore,
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
                    crate::tray::github_report(&app, Report::NoGh);
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
                        crate::tray::github_report(
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
                    use crate::github_auth::TokenStore;
                    crate::github_auth::KeychainStore.clear();
                    from_store = false;
                    retried_auth = false;
                    token = None;
                    Duration::from_secs(1)
                }
                Err(ApiError::Unauthorized) => {
                    crate::tray::github_report(&app, Report::LoginExpired);
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
        crate::window::present(app, n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Rollup::*;

    fn pr(n: u64, head: &str, rollup: Option<Rollup>, done: u32, total: u32, failed: u32) -> Pr {
        Pr {
            number: n,
            title: format!("Título {n}"),
            url: format!("https://github.com/o/r/pull/{n}"),
            repo: "o/r".into(),
            head: head.into(),
            rollup,
            checks: Checks {
                done,
                total,
                failed,
            },
        }
    }

    /// Roda as leituras em sequência e devolve as mudanças de cada uma.
    fn run(reads: &[Vec<Pr>]) -> Vec<Vec<Change>> {
        let mut state = HashMap::new();
        reads
            .iter()
            .map(|read| {
                let (next, changes) = diff(&state, read);
                state = next;
                changes
            })
            .collect()
    }

    fn kinds(changes: &[Change]) -> Vec<&'static str> {
        changes
            .iter()
            .map(|c| match c {
                Change::Progress(_) => "progresso",
                Change::Passed(_) => "passou",
                Change::Failed(_) => "falhou",
            })
            .collect()
    }

    #[test]
    fn primeira_leitura_nao_avisa_nem_pr_ja_terminado() {
        let out = run(&[vec![
            pr(1, "a", Some(Success), 3, 3, 0),
            pr(2, "b", Some(Failure), 3, 3, 1),
            pr(3, "c", Some(Pending), 1, 3, 0),
        ]]);
        assert!(out[0].is_empty());
    }

    #[test]
    fn progresso_a_cada_check_e_final_quando_todos_terminam() {
        let out = run(&[
            vec![pr(1, "a", Some(Pending), 0, 12, 0)],
            vec![pr(1, "a", Some(Pending), 3, 12, 0)],
            vec![pr(1, "a", Some(Pending), 3, 12, 0)],
            vec![pr(1, "a", Some(Pending), 11, 12, 0)],
            vec![pr(1, "a", Some(Success), 12, 12, 0)],
            vec![pr(1, "a", Some(Success), 12, 12, 0)],
        ]);
        let got: Vec<Vec<&str>> = out.iter().map(|c| kinds(c)).collect();
        assert_eq!(
            got,
            vec![
                vec![],
                vec!["progresso"],
                vec![],
                vec!["progresso"],
                vec!["passou"],
                vec![]
            ]
        );
    }

    #[test]
    fn falha_no_meio_continua_o_progresso_e_termina_com_falhou() {
        // O resumo vira FAILURE assim que um check falha, mesmo com outros rodando.
        let out = run(&[
            vec![pr(1, "a", Some(Pending), 0, 4, 0)],
            vec![pr(1, "a", Some(Failure), 1, 4, 1)],
            vec![pr(1, "a", Some(Failure), 4, 4, 1)],
        ]);
        assert_eq!(kinds(&out[1]), vec!["progresso"]);
        assert_eq!(kinds(&out[2]), vec!["falhou"]);
    }

    #[test]
    fn todos_terminados_mas_resumo_pendente_ainda_nao_e_final() {
        // Check obrigatório que ainda não reportou (EXPECTED).
        let out = run(&[
            vec![pr(1, "a", Some(Pending), 2, 3, 0)],
            vec![pr(1, "a", Some(Expected), 3, 3, 0)],
        ]);
        assert_eq!(kinds(&out[1]), vec!["progresso"]);
    }

    #[test]
    fn push_novo_reinicia_sem_avisar() {
        let out = run(&[
            vec![pr(1, "a", Some(Pending), 1, 3, 0)],
            vec![pr(1, "b", Some(Success), 3, 3, 0)],
            vec![pr(1, "c", Some(Pending), 0, 3, 0)],
            vec![pr(1, "c", Some(Failure), 3, 3, 2)],
        ]);
        assert!(out[1].is_empty(), "commit novo já terminado não avisa");
        assert!(out[2].is_empty());
        assert_eq!(kinds(&out[3]), vec!["falhou"]);
    }

    #[test]
    fn checks_rodados_de_novo_avisam_o_novo_final() {
        let out = run(&[
            vec![pr(1, "a", Some(Pending), 2, 3, 0)],
            vec![pr(1, "a", Some(Failure), 3, 3, 1)],
            vec![pr(1, "a", Some(Pending), 0, 3, 0)],
            vec![pr(1, "a", Some(Success), 3, 3, 0)],
        ]);
        let got: Vec<Vec<&str>> = out.iter().map(|c| kinds(c)).collect();
        assert_eq!(got, vec![vec![], vec!["falhou"], vec![], vec!["passou"]]);
    }

    #[test]
    fn sem_checks_nao_avisa_e_pr_fechado_sai_do_estado() {
        let mut state = HashMap::new();
        let (s, _) = diff(
            &state,
            &[
                pr(1, "a", None, 0, 0, 0),
                pr(2, "b", Some(Pending), 0, 2, 0),
            ],
        );
        state = s;
        assert!(any_running(&state));
        let (s, c) = diff(&state, &[pr(1, "a", None, 0, 0, 0)]);
        assert!(c.is_empty());
        assert_eq!(s.len(), 1);
        assert!(!any_running(&s));
    }

    #[test]
    fn conta_checks_por_estado() {
        let ctx = json!({
            "checkRunCountsByState": [
                {"state": "SUCCESS", "count": 18}, {"state": "FAILURE", "count": 3},
                {"state": "NEUTRAL", "count": 1}, {"state": "SKIPPED", "count": 1},
                {"state": "IN_PROGRESS", "count": 2}, {"state": "QUEUED", "count": 1}
            ],
            "statusContextCountsByState": [
                {"state": "SUCCESS", "count": 1}, {"state": "PENDING", "count": 1}, {"state": "ERROR", "count": 1}
            ]
        });
        assert_eq!(
            count_checks(&ctx),
            Checks {
                done: 25,
                total: 29,
                failed: 4
            }
        );
        assert_eq!(count_checks(&Value::Null), Checks::default());
    }

    #[test]
    fn le_a_resposta_do_graphql() {
        let body = json!({"data": {
            "viewer": {"login": "larasouzabr"},
            "search": {"nodes": [
                {"number": 445, "title": "Corrige o parser", "url": "https://github.com/pondworks/breadcrumbs/pull/445",
                 "repository": {"nameWithOwner": "pondworks/breadcrumbs"},
                 "commits": {"nodes": [{"commit": {"oid": "273e404", "statusCheckRollup": {
                     "state": "PENDING",
                     "contexts": {"checkRunCountsByState": [{"state": "SUCCESS", "count": 3}, {"state": "QUEUED", "count": 9}],
                                  "statusContextCountsByState": []}}}}]}},
                {"number": 7, "title": "Sem checks", "url": "https://github.com/o/r/pull/7",
                 "repository": {"nameWithOwner": "o/r"},
                 "commits": {"nodes": [{"commit": {"oid": "abc", "statusCheckRollup": null}}]}},
                {}
            ]}
        }});
        let (login, prs) = parse(&body).unwrap();
        assert_eq!(login, "larasouzabr");
        assert_eq!(prs.len(), 2, "nó vazio (não é PR) é ignorado");
        assert_eq!(prs[0].rollup, Some(Pending));
        assert_eq!(
            prs[0].checks,
            Checks {
                done: 3,
                total: 12,
                failed: 0
            }
        );
        assert_eq!(prs[1].rollup, None);
        assert_eq!(prs[1].checks, Checks::default());
    }

    #[test]
    fn erro_do_graphql_vira_erro() {
        let body = json!({"errors": [{"message": "Something went wrong"}]});
        assert!(matches!(parse(&body), Err(ApiError::Other(m)) if m.contains("Something")));
    }

    #[test]
    fn cartao_de_progresso() {
        let n = notification(
            9,
            &Change::Progress(pr(445, "a", Some(Pending), 3, 12, 0)),
            Lang::PtBr,
        );
        assert_eq!(n.status, Status::Info);
        assert_eq!(n.source, Source::Ci);
        assert_eq!(n.title.as_deref(), Some("PR #445 · Título 445"));
        assert_eq!(n.message.as_deref(), Some("3 de 12 checks concluídos"));
        assert_eq!(n.project.as_deref(), Some("o/r#445"));
        assert!(!n.sticky, "progresso some sozinho");
        assert_eq!(
            n.progress,
            Some(Progress {
                done: 3,
                total: 12,
                failed: 0
            })
        );

        let n = notification(
            9,
            &Change::Progress(pr(1, "a", Some(Failure), 4, 12, 1)),
            Lang::PtBr,
        );
        assert_eq!(
            n.status,
            Status::Fail,
            "falha no meio já deixa o gato preocupado"
        );
        assert_eq!(
            n.message.as_deref(),
            Some("4 de 12 checks concluídos · 1 falhou")
        );
        let n = notification(
            9,
            &Change::Progress(pr(1, "a", Some(Failure), 4, 12, 2)),
            Lang::En,
        );
        assert_eq!(n.message.as_deref(), Some("4 of 12 checks done · 2 failed"));
    }

    #[test]
    fn cartao_final() {
        let passed = notification(
            1,
            &Change::Passed(pr(1, "a", Some(Success), 12, 12, 0)),
            Lang::PtBr,
        );
        assert_eq!(
            (passed.status, passed.message.as_deref()),
            (Status::Ok, Some("12 checks passaram"))
        );
        let one = notification(
            1,
            &Change::Passed(pr(1, "a", Some(Success), 1, 1, 0)),
            Lang::PtBr,
        );
        assert_eq!(one.message.as_deref(), Some("1 check passou"));
        let failed = notification(
            1,
            &Change::Failed(pr(1, "a", Some(Failure), 12, 12, 2)),
            Lang::PtBr,
        );
        assert_eq!(
            (failed.status, failed.message.as_deref()),
            (Status::Fail, Some("2 de 12 checks falharam"))
        );
        let en = notification(
            1,
            &Change::Passed(pr(1, "a", Some(Success), 5, 5, 0)),
            Lang::En,
        );
        assert_eq!(en.message.as_deref(), Some("All 5 checks passed"));
        assert_eq!(failed.ttl_ms, 15_000);
        assert!(
            passed.sticky && failed.sticky,
            "resultado final só fecha no ×"
        );
    }

    #[test]
    fn cartao_corta_textos_longos_e_descarta_url_estranha() {
        let mut p = pr(1, "a", Some(Success), 1, 1, 0);
        p.title = "x".repeat(500);
        p.repo = "o".repeat(200);
        p.url = "javascript:alert(1)".into();
        let n = notification(1, &Change::Passed(p), Lang::PtBr);
        assert_eq!(n.title.unwrap().chars().count(), MAX_PR_TITLE_CHARS);
        assert_eq!(
            n.project.unwrap().chars().count(),
            crate::model::MAX_PROJECT_CHARS
        );
        assert_eq!(n.url, None);
    }
}
