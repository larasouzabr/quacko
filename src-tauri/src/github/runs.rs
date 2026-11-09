use std::collections::HashMap;

use serde_json::Value;

use super::api::{get_rest, ApiError};
use super::model::Checks;

pub const MAX_REPOS: usize = 5;
pub const RECENT_PUSH_SECS: i64 = 24 * 60 * 60;
pub const REPOS_REFRESH_SECS: i64 = 10 * 60;
pub const START_SLACK_SECS: i64 = 60;

const FAILED_CONCLUSIONS: [&str; 5] = [
    "failure",
    "timed_out",
    "cancelled",
    "startup_failure",
    "action_required",
];

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Run {
    pub id: u64,
    pub name: String,
    pub number: u64,
    pub url: String,
    pub repo: String,
    pub branch: String,
    pub completed: bool,
    pub conclusion: String,
    pub created_at: i64,
    pub jobs: Checks,
    pub failed_jobs: Vec<String>,
}

impl Run {
    pub fn passed(&self) -> bool {
        matches!(self.conclusion.as_str(), "success" | "neutral" | "skipped")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunChange {
    Progress(Run),
    Passed(Run),
    Failed(Run),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackedRun {
    completed: bool,
    jobs_done: u32,
}

pub fn parse_utc(text: &str) -> Option<i64> {
    let num = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, s) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let year = if mo <= 2 { y - 1 } else { y };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let doy = (153 * ((mo + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3_600 + mi * 60 + s)
}

pub fn recent_repos(body: &Value, now: i64) -> Vec<String> {
    body.as_array()
        .into_iter()
        .flatten()
        .filter(|r| !r["archived"].as_bool().unwrap_or(false))
        .filter(|r| {
            r["pushed_at"]
                .as_str()
                .and_then(parse_utc)
                .is_some_and(|t| now - t <= RECENT_PUSH_SECS)
        })
        .filter_map(|r| r["full_name"].as_str().map(String::from))
        .take(MAX_REPOS)
        .collect()
}

pub fn parse_runs(body: &Value, repo: &str) -> Vec<Run> {
    body["workflow_runs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| {
            !r["event"]
                .as_str()
                .unwrap_or_default()
                .starts_with("pull_request")
        })
        .filter(|r| r["pull_requests"].as_array().is_none_or(Vec::is_empty))
        .filter_map(|r| {
            Some(Run {
                id: r["id"].as_u64()?,
                name: r["name"].as_str().unwrap_or("Workflow").to_string(),
                number: r["run_number"].as_u64().unwrap_or_default(),
                url: r["html_url"].as_str()?.to_string(),
                repo: repo.to_string(),
                branch: r["head_branch"].as_str().unwrap_or_default().to_string(),
                completed: r["status"].as_str() == Some("completed"),
                conclusion: r["conclusion"].as_str().unwrap_or_default().to_string(),
                created_at: r["created_at"].as_str().and_then(parse_utc).unwrap_or(0),
                jobs: Checks::default(),
                failed_jobs: Vec::new(),
            })
        })
        .collect()
}

pub fn parse_jobs(body: &Value) -> (Checks, Vec<String>) {
    let mut checks = Checks::default();
    let mut failed = Vec::new();
    for job in body["jobs"].as_array().into_iter().flatten() {
        checks.total += 1;
        if job["status"].as_str() == Some("completed") {
            checks.done += 1;
        }
        if FAILED_CONCLUSIONS.contains(&job["conclusion"].as_str().unwrap_or_default()) {
            checks.failed += 1;
            failed.push(job["name"].as_str().unwrap_or("job").to_string());
        }
    }
    (checks, failed)
}

fn finishes_now(prev: &HashMap<u64, TrackedRun>, run: &Run, started_at: i64) -> bool {
    run.completed
        && match prev.get(&run.id) {
            Some(old) => !old.completed,
            None => run.created_at >= started_at - START_SLACK_SECS,
        }
}

pub fn needs_jobs(prev: &HashMap<u64, TrackedRun>, run: &Run, started_at: i64) -> bool {
    !run.completed || finishes_now(prev, run, started_at)
}

pub fn diff_runs(
    prev: &HashMap<u64, TrackedRun>,
    now: &[Run],
    started_at: i64,
) -> (HashMap<u64, TrackedRun>, Vec<RunChange>) {
    let mut next = HashMap::new();
    let mut changes = Vec::new();
    for run in now {
        if finishes_now(prev, run, started_at) {
            changes.push(if run.passed() {
                RunChange::Passed(run.clone())
            } else {
                RunChange::Failed(run.clone())
            });
        } else if let Some(old) = prev.get(&run.id) {
            if !run.completed && run.jobs.done > old.jobs_done {
                changes.push(RunChange::Progress(run.clone()));
            }
        }
        next.insert(
            run.id,
            TrackedRun {
                completed: run.completed,
                jobs_done: run.jobs.done,
            },
        );
    }
    (next, changes)
}

pub struct RunWatcher {
    started_at: i64,
    tracked: HashMap<u64, TrackedRun>,
    repos: Vec<String>,
    repos_checked_at: Option<i64>,
}

impl RunWatcher {
    pub fn new(started_at: i64) -> Self {
        Self {
            started_at,
            tracked: HashMap::new(),
            repos: Vec::new(),
            repos_checked_at: None,
        }
    }

    pub fn any_running(&self) -> bool {
        self.tracked.values().any(|t| !t.completed)
    }

    pub async fn poll(
        &mut self,
        client: &reqwest::Client,
        token: &str,
        login: &str,
        now: i64,
    ) -> Result<Vec<RunChange>, ApiError> {
        if self
            .repos_checked_at
            .is_none_or(|t| now - t >= REPOS_REFRESH_SECS)
        {
            let body = get_rest(client, token, "/user/repos?sort=pushed&per_page=20").await?;
            self.repos = recent_repos(&body, now);
            self.repos_checked_at = Some(now);
        }
        let mut runs = Vec::new();
        for repo in &self.repos {
            let path = format!("/repos/{repo}/actions/runs?actor={login}&per_page=10");
            match get_rest(client, token, &path).await {
                Ok(body) => runs.extend(parse_runs(&body, repo)),
                Err(ApiError::Other(e)) => tracing::debug!(%e, repo, "workflows indisponíveis"),
                Err(e) => return Err(e),
            }
        }
        for run in runs
            .iter_mut()
            .filter(|r| needs_jobs(&self.tracked, r, self.started_at))
        {
            let path = format!(
                "/repos/{}/actions/runs/{}/jobs?per_page=100",
                run.repo, run.id
            );
            if let Ok(body) = get_rest(client, token, &path).await {
                (run.jobs, run.failed_jobs) = parse_jobs(&body);
            }
        }
        let (next, changes) = diff_runs(&self.tracked, &runs, self.started_at);
        self.tracked = next;
        Ok(changes)
    }
}
