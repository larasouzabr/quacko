use super::model::*;
use super::runs::{Run, RunChange};
use crate::i18n::Lang;
use crate::model::{Notification, Progress, Source, Status};

pub const MAX_PR_TITLE_CHARS: usize = 120;

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
        title: Some(clip(&title, MAX_PR_TITLE_CHARS)),
        message: Some(checks_text(lang, change, pr.checks)),
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
        sticky: !matches!(change, Change::Progress(_)),
    }
}

const MAX_JOB_NAMES: usize = 2;

fn job_names(names: &[String], lang: Lang) -> String {
    let shown = names[..names.len().min(MAX_JOB_NAMES)].join(", ");
    match names.len().saturating_sub(MAX_JOB_NAMES) {
        0 => shown,
        rest => match lang {
            Lang::PtBr => format!("{shown} e mais {rest}"),
            Lang::En => format!("{shown} and {rest} more"),
        },
    }
}

fn run_text(lang: Lang, change: &RunChange) -> String {
    let run = match change {
        RunChange::Progress(r) | RunChange::Passed(r) | RunChange::Failed(r) => r,
    };
    let Checks {
        done,
        total,
        failed,
    } = run.jobs;
    match (lang, change) {
        (Lang::PtBr, RunChange::Progress(_)) => {
            let tail = match failed {
                0 => String::new(),
                1 => " · 1 falhou".into(),
                n => format!(" · {n} falharam"),
            };
            format!("{done} de {total} jobs concluídos{tail}")
        }
        (Lang::PtBr, RunChange::Passed(_)) if total == 0 => "Concluído".into(),
        (Lang::PtBr, RunChange::Passed(_)) if total == 1 => "1 job passou".into(),
        (Lang::PtBr, RunChange::Passed(_)) => format!("{total} jobs passaram"),
        (Lang::PtBr, RunChange::Failed(r)) if r.conclusion == "cancelled" => "Cancelado".into(),
        (Lang::PtBr, RunChange::Failed(r)) if r.failed_jobs.is_empty() => "Falhou".into(),
        (Lang::PtBr, RunChange::Failed(r)) => {
            format!("Falhou: {}", job_names(&r.failed_jobs, lang))
        }
        (Lang::En, RunChange::Progress(_)) => {
            let tail = if failed > 0 {
                format!(" · {failed} failed")
            } else {
                String::new()
            };
            format!("{done} of {total} jobs done{tail}")
        }
        (Lang::En, RunChange::Passed(_)) if total == 0 => "Done".into(),
        (Lang::En, RunChange::Passed(_)) if total == 1 => "1 job passed".into(),
        (Lang::En, RunChange::Passed(_)) => format!("All {total} jobs passed"),
        (Lang::En, RunChange::Failed(r)) if r.conclusion == "cancelled" => "Cancelled".into(),
        (Lang::En, RunChange::Failed(r)) if r.failed_jobs.is_empty() => "Failed".into(),
        (Lang::En, RunChange::Failed(r)) => format!("Failed: {}", job_names(&r.failed_jobs, lang)),
    }
}

pub fn run_notification(id: u64, change: &RunChange, lang: Lang) -> Notification {
    let (status, run, ttl_ms): (Status, &Run, u32) = match change {
        RunChange::Progress(r) if r.jobs.failed > 0 => (Status::Fail, r, 6_000),
        RunChange::Progress(r) => (Status::Info, r, 6_000),
        RunChange::Passed(r) => (Status::Ok, r, 8_000),
        RunChange::Failed(r) => (Status::Fail, r, 15_000),
    };
    let clip = |s: &str, max: usize| -> String { s.chars().take(max).collect() };
    let title = if run.number > 0 {
        format!("{} #{}", run.name.trim(), run.number)
    } else {
        run.name.trim().to_string()
    };
    let project = if run.branch.is_empty() {
        run.repo.clone()
    } else {
        format!("{} · {}", run.repo, run.branch)
    };
    Notification {
        id,
        source: Source::Ci,
        status,
        title: Some(clip(&title, MAX_PR_TITLE_CHARS)),
        message: Some(run_text(lang, change)),
        project: Some(clip(&project, crate::model::MAX_PROJECT_CHARS)),
        url: crate::model::clean_url(&run.url),
        ttl_ms,
        progress: (run.jobs.total > 0).then_some(Progress {
            done: run.jobs.done.min(run.jobs.total),
            total: run.jobs.total,
            failed: run.jobs.failed.min(run.jobs.total),
        }),
        sticky: !matches!(change, RunChange::Progress(_)),
    }
}
