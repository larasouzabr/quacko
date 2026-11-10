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

mod workflow_runs {
    use super::super::card::run_notification;
    use super::super::runs::*;
    use super::*;

    const START: i64 = 1_794_263_088;

    fn run(
        id: u64,
        completed: bool,
        conclusion: &str,
        created_at: i64,
        done: u32,
        total: u32,
    ) -> Run {
        Run {
            id,
            name: "Release".into(),
            number: 2,
            url: format!("https://github.com/larasouzabr/quacko/actions/runs/{id}"),
            repo: "larasouzabr/quacko".into(),
            branch: "main".into(),
            completed,
            conclusion: conclusion.into(),
            created_at,
            jobs: Checks {
                done,
                total,
                failed: 0,
            },
            failed_jobs: Vec::new(),
        }
    }

    #[test]
    fn datas_do_github_em_segundos() {
        assert_eq!(parse_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_utc("2026-11-09T22:24:48Z"), Some(START));
        assert_eq!(parse_utc("2024-02-29T12:00:00Z"), Some(1_709_208_000));
        assert_eq!(parse_utc("ontem"), None);
    }

    #[test]
    fn so_repositorios_com_push_recente_e_no_maximo_cinco() {
        let recent = "2026-11-09T20:00:00Z";
        let mut repos: Vec<Value> = (0..7)
            .map(|i| json!({"full_name": format!("ana/r{i}"), "pushed_at": recent, "archived": false}))
            .collect();
        repos.insert(
            0,
            json!({"full_name": "ana/velho", "pushed_at": "2026-11-01T00:00:00Z"}),
        );
        repos.insert(
            0,
            json!({"full_name": "ana/arquivado", "pushed_at": recent, "archived": true}),
        );
        let found = recent_repos(&Value::Array(repos), START);
        assert_eq!(found, ["ana/r0", "ana/r1", "ana/r2", "ana/r3", "ana/r4"]);
    }

    #[test]
    fn execucoes_de_pr_ficam_com_o_monitor_de_prs() {
        let body = json!({"workflow_runs": [
            {"id": 1, "name": "Release", "run_number": 3, "html_url": "https://github.com/a/b/actions/runs/1",
             "status": "in_progress", "conclusion": null, "event": "workflow_dispatch", "head_branch": "main",
             "created_at": "2026-11-09T22:30:00Z", "pull_requests": []},
            {"id": 2, "name": "CI", "html_url": "https://github.com/a/b/actions/runs/2", "status": "completed",
             "event": "pull_request", "pull_requests": []},
            {"id": 3, "name": "CI", "html_url": "https://github.com/a/b/actions/runs/3", "status": "completed",
             "event": "push", "pull_requests": [{"number": 7}]}
        ]});
        let runs = parse_runs(&body, "a/b");
        assert_eq!(runs.len(), 1);
        assert_eq!(
            (
                runs[0].id,
                runs[0].number,
                runs[0].completed,
                runs[0].branch.as_str(),
                runs[0].created_at
            ),
            (1, 3, false, "main", START + 312)
        );
    }

    #[test]
    fn jobs_contam_progresso_e_nomes_que_falharam() {
        let body = json!({"jobs": [
            {"name": "macos", "status": "completed", "conclusion": "success"},
            {"name": "windows", "status": "completed", "conclusion": "failure"},
            {"name": "draft", "status": "completed", "conclusion": "skipped"},
            {"name": "linux", "status": "in_progress", "conclusion": null}
        ]});
        let (checks, failed) = parse_jobs(&body);
        assert_eq!((checks.done, checks.total, checks.failed), (3, 4, 1));
        assert_eq!(failed, ["windows"]);
    }

    #[test]
    fn avisa_quando_termina_e_so_uma_vez() {
        let empty = HashMap::new();
        let (state, changes) = diff_runs(&empty, &[run(1, false, "", START + 10, 0, 2)], START);
        assert!(changes.is_empty(), "começou: só acompanha");
        let (state, changes) = diff_runs(&state, &[run(1, false, "", START + 10, 1, 2)], START);
        assert!(matches!(changes.as_slice(), [RunChange::Progress(_)]));
        let mut done = run(1, true, "failure", START + 10, 2, 2);
        done.failed_jobs = vec!["windows".into()];
        let (state, changes) = diff_runs(&state, std::slice::from_ref(&done), START);
        assert!(matches!(changes.as_slice(), [RunChange::Failed(_)]));
        let (_, changes) = diff_runs(&state, &[done], START);
        assert!(changes.is_empty(), "não repete");
    }

    #[test]
    fn execucao_antiga_nao_avisa_e_rapida_avisa() {
        let empty = HashMap::new();
        let old = run(1, true, "success", START - 3_600, 2, 2);
        assert!(diff_runs(&empty, std::slice::from_ref(&old), START)
            .1
            .is_empty());
        assert!(!needs_jobs(&empty, &old, START));
        let quick = run(2, true, "success", START + 20, 1, 1);
        assert!(needs_jobs(&empty, &quick, START));
        assert!(matches!(
            diff_runs(&empty, &[quick], START).1.as_slice(),
            [RunChange::Passed(_)]
        ));
    }

    #[test]
    fn cartao_do_workflow() {
        let mut failed = run(9, true, "failure", START, 2, 2);
        failed.jobs.failed = 1;
        failed.failed_jobs = vec!["windows".into()];
        let n = run_notification(1, &RunChange::Failed(failed.clone()), Lang::PtBr);
        assert_eq!(n.status, Status::Fail);
        assert_eq!(n.title.as_deref(), Some("Release #2"));
        assert_eq!(n.message.as_deref(), Some("Falhou: windows"));
        assert_eq!(n.project.as_deref(), Some("larasouzabr/quacko · main"));
        assert_eq!(
            n.url.as_deref(),
            Some("https://github.com/larasouzabr/quacko/actions/runs/9")
        );
        assert!(n.sticky);
        assert_eq!(n.source, Source::Ci);

        failed.failed_jobs = vec!["a".into(), "b".into(), "c".into()];
        let n = run_notification(2, &RunChange::Failed(failed), Lang::En);
        assert_eq!(n.message.as_deref(), Some("Failed: a, b and 1 more"));

        let progress = run(9, false, "", START, 1, 2);
        let n = run_notification(3, &RunChange::Progress(progress), Lang::PtBr);
        assert_eq!(n.message.as_deref(), Some("1 de 2 jobs concluídos"));
        assert!(!n.sticky);
        assert_eq!(n.status, Status::Info);

        let passed = run(9, true, "success", START, 2, 2);
        let n = run_notification(4, &RunChange::Passed(passed), Lang::En);
        assert_eq!(
            (n.status, n.message.as_deref()),
            (Status::Ok, Some("All 2 jobs passed"))
        );

        let cancelled = run(9, true, "cancelled", START, 0, 2);
        let n = run_notification(5, &RunChange::Failed(cancelled), Lang::PtBr);
        assert_eq!(n.message.as_deref(), Some("Cancelado"));
    }
}

#[test]
fn client_id_da_variavel_da_config_ou_embutido_no_build() {
    use super::monitor::resolve_client_id;
    assert_eq!(
        resolve_client_id(Some(" Iv23env "), Some("Iv23cfg"), Some("Iv23build")),
        Some("Iv23env".to_string())
    );
    assert_eq!(
        resolve_client_id(Some("tem espaço"), Some("Iv23cfg"), Some("Iv23build")),
        Some("Iv23cfg".to_string())
    );
    assert_eq!(
        resolve_client_id(None, None, Some("Iv23build")),
        Some("Iv23build".to_string())
    );
    assert_eq!(resolve_client_id(None, None, Some("")), None);
    assert_eq!(resolve_client_id(None, None, None), None);
}
