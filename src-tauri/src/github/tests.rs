use crate::i18n::Lang;
use crate::model::{Progress, Source, Status};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use super::api::*;
use super::card::*;
use super::model::*;

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

#[test]
fn gh_no_windows_nos_lugares_dos_instaladores() {
    let home = Path::new(r"C:\Users\Ana");
    let env = |key: &str| match key {
        "ProgramFiles" => Some(r"C:\Program Files".into()),
        "LOCALAPPDATA" => Some(r"C:\Users\Ana\AppData\Local".into()),
        _ => None,
    };
    let found: Vec<String> = super::gh_cli::windows_candidates(home, env)
        .iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    assert_eq!(
        found,
        [
            "C:/Program Files/GitHub CLI/gh.exe",
            "C:/Users/Ana/AppData/Local/Programs/GitHub CLI/gh.exe",
            "C:/Users/Ana/scoop/shims/gh.exe",
        ]
    );
}

#[test]
fn gh_pelo_path() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("gh.exe"), "").unwrap();
    let path = std::env::join_paths([dir.path().join("vazio"), bin.clone()]).unwrap();
    assert_eq!(
        super::gh_cli::in_path(path, "gh.exe"),
        Some(bin.join("gh.exe"))
    );
    assert_eq!(
        super::gh_cli::in_path(std::env::join_paths([dir.path()]).unwrap(), "gh.exe"),
        None
    );
}
