//! Cartão de um PR: título, contagem dos checks e barra de progresso.

use super::model::*;
use crate::i18n::Lang;
use crate::model::{Notification, Progress, Source, Status};

/// O título do PR é o que mais ajuda a reconhecê-lo: vai além do limite da API local (60).
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
