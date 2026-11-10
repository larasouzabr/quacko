use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::hooks::*;
use super::scripts::*;

use super::*;

const NOTIFY: &str = "/Users/ana/.claude/hooks/notify.sh";

fn notify() -> PathBuf {
    PathBuf::from(NOTIFY)
}

fn script() -> Hook {
    Hook::Script(notify())
}

fn set_connected(paths: &Paths, tool: Tool, connect: bool) -> io::Result<Outcome> {
    connect_with(paths, tool, connect, &Hook::Script(paths.notify_sh()))
}

fn connect(tool: Tool, doc: &mut Value) {
    apply(tool, doc, &script(), true).unwrap();
}

fn all_commands(tool: Tool, doc: &Value) -> Vec<String> {
    doc["hooks"]
        .as_object()
        .map(|h| {
            h.values()
                .filter_map(Value::as_array)
                .flat_map(|e| commands_in(tool, e))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn claude_em_arquivo_vazio() {
    let mut doc = json!({});
    connect(Tool::Claude, &mut doc);
    assert!(is_connected(Tool::Claude, &doc));
    assert_eq!(
        doc["hooks"]["Stop"][0]["hooks"][0]["command"],
        format!("{NOTIFY} --from claude --sound /System/Library/Sounds/Glass.aiff")
    );
    assert_eq!(
        doc["hooks"]["UserPromptSubmit"][0]["hooks"][0]["type"],
        "command"
    );
}

#[test]
fn preserva_outras_chaves_hooks_de_terceiros_e_a_ordem() {
    let mut doc: Value = serde_json::from_str(
            r#"{"permissions":{"allow":["Bash(ls)"]},"model":"opus",
                "hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"/opt/lint.sh"}]}],
                         "PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"/opt/guard.sh"}]}]}}"#,
        )
        .unwrap();
    connect(Tool::Claude, &mut doc);
    let keys: Vec<&String> = doc.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["permissions", "model", "hooks"]);
    assert_eq!(doc["permissions"]["allow"][0], "Bash(ls)");
    assert_eq!(
        doc["hooks"]["Stop"][0]["hooks"][0]["command"],
        "/opt/lint.sh"
    );
    assert_eq!(doc["hooks"]["Stop"].as_array().unwrap().len(), 2);
    assert_eq!(
        doc["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
        "/opt/guard.sh"
    );
}

#[test]
fn conectar_duas_vezes_nao_duplica() {
    let mut doc = json!({});
    connect(Tool::Claude, &mut doc);
    let once = doc.clone();
    connect(Tool::Claude, &mut doc);
    assert_eq!(doc, once);
}

#[test]
fn hooks_antigos_do_quacko_sao_atualizados() {
    let mut doc: Value = serde_json::from_str(&format!(
            r#"{{"hooks":{{"Stop":[{{"hooks":[{{"type":"command","command":"{NOTIFY} /System/Library/Sounds/Glass.aiff"}}]}}]}}}}"#
        ))
        .unwrap();
    assert!(!is_connected(Tool::Claude, &doc));
    connect(Tool::Claude, &mut doc);
    assert!(is_connected(Tool::Claude, &doc));
    assert_eq!(
        doc["hooks"]["Stop"].as_array().unwrap().len(),
        1,
        "o antigo foi trocado, não somado"
    );
}

#[test]
fn grupo_misto_perde_so_a_entrada_do_quacko() {
    let mut doc: Value = serde_json::from_str(&format!(
            r#"{{"hooks":{{"Stop":[{{"hooks":[{{"type":"command","command":"/opt/a.sh"}},{{"type":"command","command":"{NOTIFY}"}}]}}]}}}}"#
        ))
        .unwrap();
    apply(Tool::Claude, &mut doc, &script(), false).unwrap();
    assert_eq!(
        doc["hooks"]["Stop"][0]["hooks"].as_array().unwrap().len(),
        1
    );
    assert_eq!(doc["hooks"]["Stop"][0]["hooks"][0]["command"], "/opt/a.sh");
}

#[test]
fn desconectar_tira_so_o_quacko_e_limpa_o_que_ficou_vazio() {
    let mut doc: Value = serde_json::from_str(
            r#"{"model":"opus","hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"/opt/guard.sh"}]}]}}"#,
        )
        .unwrap();
    let original = doc.clone();
    connect(Tool::Claude, &mut doc);
    apply(Tool::Claude, &mut doc, &script(), false).unwrap();
    assert_eq!(doc, original);

    let mut only_quacko = json!({});
    connect(Tool::Claude, &mut only_quacko);
    apply(Tool::Claude, &mut only_quacko, &script(), false).unwrap();
    assert_eq!(only_quacko, json!({}));
}

#[test]
fn desconectar_nao_apaga_lista_vazia_de_outro_evento() {
    let mut doc = json!({"hooks": {"PreCompact": []}});
    connect(Tool::Claude, &mut doc);
    apply(Tool::Claude, &mut doc, &script(), false).unwrap();
    assert_eq!(doc, json!({"hooks": {"PreCompact": []}}));
}

#[test]
fn formato_inesperado_vai_para_o_modo_manual() {
    for bad in [
        json!([]),
        json!({"hooks": []}),
        json!({"hooks": {"Stop": {}}}),
    ] {
        let mut doc = bad.clone();
        assert_eq!(
            apply(Tool::Claude, &mut doc, &script(), true),
            Err(ManualReason::Invalid),
            "{bad}"
        );
        assert_eq!(doc, bad, "nada muda no modo manual");
    }
}

#[test]
fn cursor_cria_versao_e_mescla() {
    let mut doc: Value =
        serde_json::from_str(r#"{"hooks":{"stop":[{"command":"./hooks/audio.sh"}]}}"#).unwrap();
    connect(Tool::Cursor, &mut doc);
    assert_eq!(doc["version"], 1);
    assert!(is_connected(Tool::Cursor, &doc));
    assert_eq!(doc["hooks"]["stop"][0]["command"], "./hooks/audio.sh");
    assert_eq!(
        doc["hooks"]["beforeSubmitPrompt"][0]["command"],
        format!("{NOTIFY} --from cursor")
    );
    apply(Tool::Cursor, &mut doc, &script(), false).unwrap();
    assert_eq!(all_commands(Tool::Cursor, &doc), vec!["./hooks/audio.sh"]);
}

#[test]
fn caminho_com_espaco_vai_entre_aspas() {
    let cmd = command_for(
        Path::new("/Users/Ana Maria/.claude/hooks/notify.sh"),
        "--from claude",
    );
    assert_eq!(
        cmd,
        "'/Users/Ana Maria/.claude/hooks/notify.sh' --from claude"
    );
    assert!(is_quacko(&cmd));
    assert_eq!(command_for(&notify(), "-x"), format!("{NOTIFY} -x"));
}

fn temp_home() -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths {
        home: dir.path().to_path_buf(),
    };
    (dir, paths)
}

#[test]
fn conecta_sem_settings_existente_e_instala_o_script() {
    let (_dir, paths) = temp_home();
    let outcome = set_connected(&paths, Tool::Claude, true).unwrap();
    assert!(matches!(outcome, Outcome::Connected));
    assert!(status(&paths, Tool::Claude));
    let script = fs::read_to_string(paths.notify_sh()).unwrap();
    assert_eq!(script, NOTIFY_SH);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(paths.notify_sh())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }
}

#[test]
fn faz_backup_do_settings_e_do_script_antigo() {
    let (dir, paths) = temp_home();
    let file = paths.config_file(Tool::Claude);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, r#"{"model":"opus"}"#).unwrap();
    fs::create_dir_all(paths.notify_sh().parent().unwrap()).unwrap();
    fs::write(paths.notify_sh(), "#!/bin/bash\necho antigo\n").unwrap();

    set_connected(&paths, Tool::Claude, true).unwrap();

    let claude_dir = dir.path().join(".claude");
    let backups: Vec<String> = fs::read_dir(&claude_dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.starts_with("settings.json.quacko-backup-"))
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(
        fs::read_to_string(claude_dir.join(&backups[0])).unwrap(),
        r#"{"model":"opus"}"#
    );
    assert_eq!(
        fs::read_to_string(paths.hooks_dir().join("notify.sh.bak")).unwrap(),
        "#!/bin/bash\necho antigo\n"
    );
    let saved: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(saved["model"], "opus");
}

#[test]
fn json_com_comentario_fica_intocado_e_vira_modo_manual() {
    let (_dir, paths) = temp_home();
    let file = paths.config_file(Tool::Claude);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let original = "{\n  // meu comentário\n  \"model\": \"opus\"\n}\n";
    fs::write(&file, original).unwrap();
    match set_connected(&paths, Tool::Claude, true).unwrap() {
        Outcome::Manual {
            reason, snippet, ..
        } => {
            assert_eq!(reason, ManualReason::Invalid);
            assert!(snippet.contains("--from claude"));
        }
        other => panic!("esperava modo manual, veio {other:?}"),
    }
    assert_eq!(fs::read_to_string(&file).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn somente_leitura_vira_modo_manual() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, paths) = temp_home();
    let file = paths.config_file(Tool::Claude);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "{}").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o444)).unwrap();
    let outcome = set_connected(&paths, Tool::Claude, true).unwrap();
    assert!(matches!(
        outcome,
        Outcome::Manual {
            reason: ManualReason::ReadOnly,
            ..
        }
    ));
    assert_eq!(fs::read_to_string(&file).unwrap(), "{}");
}

#[cfg(unix)]
#[test]
fn link_simbolico_grava_no_destino_e_mantem_o_link() {
    let (dir, paths) = temp_home();
    let real = dir.path().join("dotfiles").join("claude-settings.json");
    fs::create_dir_all(real.parent().unwrap()).unwrap();
    fs::write(&real, r#"{"model":"opus"}"#).unwrap();
    let file = paths.config_file(Tool::Claude);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&real, &file).unwrap();

    set_connected(&paths, Tool::Claude, true).unwrap();
    assert!(fs::symlink_metadata(&file)
        .unwrap()
        .file_type()
        .is_symlink());
    let saved: Value = serde_json::from_str(&fs::read_to_string(&real).unwrap()).unwrap();
    assert!(is_connected(Tool::Claude, &saved));
}

#[test]
fn desconectar_preserva_hooks_de_terceiros_no_arquivo() {
    let (_dir, paths) = temp_home();
    let file = paths.config_file(Tool::Claude);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(
        &file,
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"/opt/a.sh"}]}]}}"#,
    )
    .unwrap();
    set_connected(&paths, Tool::Claude, true).unwrap();
    assert!(status(&paths, Tool::Claude));
    assert!(matches!(
        set_connected(&paths, Tool::Claude, false).unwrap(),
        Outcome::Disconnected
    ));
    assert!(!status(&paths, Tool::Claude));
    let saved: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(
        saved,
        json!({"hooks":{"Stop":[{"hooks":[{"type":"command","command":"/opt/a.sh"}]}]}})
    );
}

#[cfg(unix)]
#[test]
fn prwatch_cria_o_link_e_nao_atropela_outro() {
    let (_dir, paths) = temp_home();
    assert!(matches!(
        install_prwatch(&paths).unwrap(),
        PrwatchOutcome::Linked
    ));
    assert_eq!(
        fs::read_link(paths.local_bin_prwatch()).unwrap(),
        paths.prwatch()
    );
    assert!(
        matches!(install_prwatch(&paths).unwrap(), PrwatchOutcome::Linked),
        "idempotente"
    );

    let (_dir2, other) = temp_home();
    fs::create_dir_all(other.local_bin_prwatch().parent().unwrap()).unwrap();
    fs::write(other.local_bin_prwatch(), "meu script").unwrap();
    assert!(matches!(
        install_prwatch(&other).unwrap(),
        PrwatchOutcome::Conflict
    ));
    assert_eq!(
        fs::read_to_string(other.local_bin_prwatch()).unwrap(),
        "meu script"
    );
}

#[test]
fn sincroniza_so_scripts_do_quacko_desatualizados() {
    let (_dir, paths) = temp_home();
    assert!(
        sync_installed_scripts(&paths).is_empty(),
        "nada instalado: nada a fazer"
    );

    fs::create_dir_all(paths.hooks_dir()).unwrap();
    fs::write(
        paths.notify_sh(),
        "#!/bin/bash\n# versão antiga\nPORT=\"${QUACKO_PORT:-4747}\"\n",
    )
    .unwrap();
    fs::write(
        paths.prwatch(),
        "#!/bin/bash\necho script de outra pessoa\n",
    )
    .unwrap();

    assert_eq!(
        sync_installed_scripts(&paths),
        vec!["notify.sh".to_string()]
    );
    assert_eq!(fs::read_to_string(paths.notify_sh()).unwrap(), NOTIFY_SH);
    assert!(fs::read_to_string(paths.hooks_dir().join("notify.sh.bak"))
        .unwrap()
        .contains("versão antiga"));
    assert_eq!(
        fs::read_to_string(paths.prwatch()).unwrap(),
        "#!/bin/bash\necho script de outra pessoa\n",
        "script que não é do Quacko fica intocado"
    );
    assert!(
        sync_installed_scripts(&paths).is_empty(),
        "já atualizado: nada a fazer"
    );
}

#[test]
fn scripts_da_versao_com_nome_antigo_sao_atualizados() {
    let (_dir, paths) = temp_home();
    fs::create_dir_all(paths.hooks_dir()).unwrap();
    fs::write(
        paths.notify_sh(),
        "#!/bin/bash\nPORT=\"${MIAU_PORT:-4747}\"\n",
    )
    .unwrap();
    assert_eq!(
        sync_installed_scripts(&paths),
        vec!["notify.sh".to_string()]
    );
    assert_eq!(fs::read_to_string(paths.notify_sh()).unwrap(), NOTIFY_SH);
}

#[test]
fn scripts_embutidos_sao_os_do_repositorio() {
    assert!(NOTIFY_SH.starts_with("#!/bin/bash"));
    assert!(NOTIFY_SH.contains("--from"));
    assert!(PRWATCH.contains("gh pr checks"));
    assert!(QUACKO_ZSH.contains("add-zsh-hook preexec"));
    assert!(
        NOTIFY_SH.contains(OURS_MARKER)
            && PRWATCH.contains(OURS_MARKER)
            && QUACKO_ZSH.contains(OURS_MARKER),
        "a sincronização depende da marca"
    );
}

const EXE: &str = r"C:\Users\Ana Maria\AppData\Local\Quacko\quacko-hook.exe";

fn exe() -> Hook {
    Hook::Exe(PathBuf::from(EXE))
}

#[test]
fn reconhece_o_notify_sh_com_barras_do_windows() {
    assert!(is_quacko(
        r"'C:\Users\Ana\.claude\hooks\notify.sh' --from claude"
    ));
    assert!(!is_quacko(r"C:\tools\lint.exe"));
}

#[test]
fn windows_claude_em_forma_exec_sem_shell() {
    let mut doc = json!({});
    apply(Tool::Claude, &mut doc, &exe(), true).unwrap();
    let stop = &doc["hooks"]["Stop"][0]["hooks"][0];
    assert_eq!(stop["command"], EXE);
    assert_eq!(
        stop["args"],
        json!(["--from", "claude"]),
        "sem --sound do macOS"
    );
    assert!(is_connected(Tool::Claude, &doc));
    apply(Tool::Claude, &mut doc, &exe(), false).unwrap();
    assert_eq!(doc, json!({}));
}

#[test]
fn windows_cursor_pelo_powershell_com_aspas_simples() {
    let mut doc = json!({});
    apply(Tool::Cursor, &mut doc, &exe(), true).unwrap();
    assert_eq!(
        doc["hooks"]["stop"][0]["command"],
        format!("& '{EXE}' --from cursor")
    );
    assert!(is_connected(Tool::Cursor, &doc));
    assert_eq!(
        hooks::powershell_call(Path::new(r"C:\Users\O'Neil\q.exe"), &["-x".into()]),
        r"& 'C:\Users\O''Neil\q.exe' -x"
    );
}

#[test]
fn trocar_o_script_pelo_executavel_nao_duplica() {
    let mut doc = json!({});
    connect(Tool::Claude, &mut doc);
    apply(Tool::Claude, &mut doc, &exe(), true).unwrap();
    let ours: Vec<String> = all_commands(Tool::Claude, &doc);
    assert_eq!(ours.len(), 3);
    assert!(ours.iter().all(|c| c.starts_with(EXE)));
}

#[test]
fn executavel_que_nao_existe_nao_conecta() {
    let (_dir, paths) = temp_home();
    let missing = Hook::Exe(paths.home.join("sumiu").join("quacko-hook.exe"));
    assert!(connect_with(&paths, Tool::Claude, true, &missing).is_err());
    assert!(!paths.config_file(Tool::Claude).exists());
}

#[test]
fn app_movido_atualiza_o_caminho_dos_hooks() {
    let (dir, paths) = temp_home();
    let old = dir.path().join("antigo").join("quacko-hook.exe");
    let new = dir.path().join("novo").join("quacko-hook.exe");
    for p in [&old, &new] {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, "").unwrap();
    }
    connect_with(&paths, Tool::Claude, true, &Hook::Exe(old.clone())).unwrap();
    assert_eq!(
        refresh_exe_hooks(&paths, &old),
        Vec::<Tool>::new(),
        "já aponta para o atual"
    );
    assert_eq!(refresh_exe_hooks(&paths, &new), vec![Tool::Claude]);
    let text = fs::read_to_string(paths.config_file(Tool::Claude)).unwrap();
    assert!(text.contains("novo") && !text.contains("antigo"));
    assert_eq!(refresh_exe_hooks(&paths, &new), Vec::<Tool>::new());
}

#[test]
fn terminal_instala_o_script_e_so_le_o_zshrc() {
    let (_dir, paths) = temp_home();
    let zshrc = paths.home.join(".zshrc");
    fs::write(&zshrc, "# source ~/.claude/hooks/quacko.zsh\nexport A=1\n").unwrap();
    assert!(matches!(
        install_shell(&paths).unwrap(),
        ShellOutcome::Copied { line, nix: false } if line == "source ~/.claude/hooks/quacko.zsh"
    ));
    assert_eq!(fs::read_to_string(paths.quacko_zsh()).unwrap(), QUACKO_ZSH);
    assert_eq!(fs::read_to_string(paths.notify_sh()).unwrap(), NOTIFY_SH);
    assert_eq!(
        fs::read_to_string(&zshrc).unwrap(),
        "# source ~/.claude/hooks/quacko.zsh\nexport A=1\n",
        "o .zshrc nunca é editado"
    );

    fs::write(
        &zshrc,
        "export A=1\n  source \"$HOME/.claude/hooks/quacko.zsh\"\n",
    )
    .unwrap();
    assert!(matches!(
        install_shell(&paths).unwrap(),
        ShellOutcome::AlreadySourced
    ));
}

#[test]
fn terminal_sem_zshrc_copia_a_linha() {
    let (_dir, paths) = temp_home();
    assert!(matches!(
        install_shell(&paths).unwrap(),
        ShellOutcome::Copied { nix: false, .. }
    ));
    assert!(!paths.home.join(".zshrc").exists());
}

#[cfg(unix)]
#[test]
fn terminal_com_zshrc_do_nix_avisa_onde_colocar() {
    let (_dir, paths) = temp_home();
    std::os::unix::fs::symlink(
        "/nix/store/abc-home-manager-files/.zshrc",
        paths.home.join(".zshrc"),
    )
    .unwrap();
    assert!(matches!(
        install_shell(&paths).unwrap(),
        ShellOutcome::Copied { nix: true, .. }
    ));
}
