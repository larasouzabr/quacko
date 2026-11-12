use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Rollup {
    Expected,
    Pending,
    Success,
    Failure,
    Error,
}

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
    pub rollup: Option<Rollup>,
    pub checks: Checks,
    pub activity: Vec<super::reviews::Activity>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tracked {
    head: String,
    rollup: Option<Rollup>,
    checks: Checks,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Progress(Pr),
    Passed(Pr),
    Failed(Pr),
}

fn running(r: Option<Rollup>) -> bool {
    matches!(r, Some(Rollup::Pending | Rollup::Expected))
}

fn is_finished(rollup: Option<Rollup>, checks: Checks) -> bool {
    checks.total > 0 && checks.done >= checks.total && rollup.is_some() && !running(rollup)
}

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

pub fn any_running(state: &HashMap<String, Tracked>) -> bool {
    state
        .values()
        .any(|t| running(t.rollup) || (t.checks.total > 0 && !is_finished(t.rollup, t.checks)))
}
