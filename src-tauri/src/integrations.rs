//! "Conectar" pelo menu: instala os scripts embutidos no app e liga os hooks do
//! Claude Code e do Cursor, sem a pessoa precisar do código nem de editar JSON.
//!
//! Regras (spec v2, seção 6.1):
//! - Arquivo de config inexistente é criado. Link simbólico é seguido; se o destino
//!   estiver em `/nix/store`, for somente leitura ou não for JSON válido: **modo manual**
//!   (nada é alterado; o trecho vai para a área de transferência).
//! - Backup antes de alterar (`<arquivo>.quacko-backup-<timestamp>`).
//! - Mescla idempotente: as entradas do Quacko são reconhecidas pelo caminho do
//!   `notify.sh`; as dos outros ficam intactas, na mesma ordem.
//! - Gravação atômica (temporário + rename). "Desconectar" remove só as do Quacko.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};

/// Scripts que vão dentro do app (mesmos arquivos de `scripts/`).
const NOTIFY_SH: &str = include_str!("../../scripts/notify.sh");
const PRWATCH: &str = include_str!("../../scripts/prwatch");

/// Um comando é do Quacko se aponta para o notify.sh instalado por ele (inclui os hooks
/// da v1 no formato antigo) ou para o CLI da v2.
const QUACKO_MARKERS: [&str; 2] = ["/.claude/hooks/notify.sh", "quacko hook"];

const CLAUDE_HOOKS: [(&str, &str); 3] = [
    ("UserPromptSubmit", "--from claude"),
    (
        "Stop",
        "--from claude --sound /System/Library/Sounds/Glass.aiff",
    ),
    (
        "Notification",
        "--from claude --sound /System/Library/Sounds/Ping.aiff",
    ),
];

const CURSOR_HOOKS: [(&str, &str); 2] = [
    ("beforeSubmitPrompt", "--from cursor"),
    (
        "stop",
        "--from cursor --sound /System/Library/Sounds/Glass.aiff",
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Claude,
    Cursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualReason {
    /// Destino em `/nix/store` (home-manager etc.).
    Managed,
    ReadOnly,
    /// Não é JSON válido (ex.: tem comentários) ou tem formato inesperado.
    Invalid,
}

#[derive(Debug)]
pub enum Outcome {
    Connected,
    Disconnected,
    /// Nada foi alterado; `snippet` é o trecho para colar à mão.
    Manual {
        reason: ManualReason,
        file: PathBuf,
        snippet: String,
    },
}

/// Caminhos usados (injetáveis nos testes).
#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
}

impl Paths {
    pub fn from_home() -> Option<Self> {
        dirs::home_dir().map(|home| Self { home })
    }
    fn hooks_dir(&self) -> PathBuf {
        self.home.join(".claude").join("hooks")
    }
    pub fn notify_sh(&self) -> PathBuf {
        self.hooks_dir().join("notify.sh")
    }
    fn prwatch(&self) -> PathBuf {
        self.hooks_dir().join("prwatch")
    }
    pub fn local_bin_prwatch(&self) -> PathBuf {
        self.home.join(".local").join("bin").join("prwatch")
    }
    pub fn config_file(&self, tool: Tool) -> PathBuf {
        match tool {
            Tool::Claude => self.home.join(".claude").join("settings.json"),
            Tool::Cursor => self.home.join(".cursor").join("hooks.json"),
        }
    }
    /// O Cursor só é oferecido se parece instalado.
    pub fn cursor_present(&self) -> bool {
        self.home.join(".cursor").is_dir()
    }
}

// ---------- mescla (funções puras) ----------

fn is_quacko(command: &str) -> bool {
    QUACKO_MARKERS.iter().any(|m| command.contains(m))
}

/// Aspas simples quando o caminho tem caractere que o shell interpretaria.
fn shell_quote(path: &str) -> String {
    let safe = path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c));
    if safe {
        path.to_string()
    } else {
        format!("'{}'", path.replace('\'', r"'\''"))
    }
}

fn command_for(notify: &Path, args: &str) -> String {
    format!("{} {args}", shell_quote(&notify.to_string_lossy()))
}

fn events(tool: Tool) -> &'static [(&'static str, &'static str)] {
    match tool {
        Tool::Claude => &CLAUDE_HOOKS,
        Tool::Cursor => &CURSOR_HOOKS,
    }
}

/// Comandos de um evento, no formato de cada ferramenta.
fn commands_in(tool: Tool, entries: &[Value]) -> Vec<String> {
    let as_str = |v: &Value| v.get("command").and_then(Value::as_str).map(String::from);
    match tool {
        // Claude: [{ "matcher"?, "hooks": [{ "type", "command" }] }]
        Tool::Claude => entries
            .iter()
            .filter_map(|g| g.get("hooks").and_then(Value::as_array))
            .flatten()
            .filter_map(as_str)
            .collect(),
        // Cursor: [{ "command", ... }]
        Tool::Cursor => entries.iter().filter_map(as_str).collect(),
    }
}

/// Está conectado se todos os eventos têm um comando do Quacko no formato novo.
pub fn is_connected(tool: Tool, doc: &Value) -> bool {
    let flag = match tool {
        Tool::Claude => "--from claude",
        Tool::Cursor => "--from cursor",
    };
    events(tool).iter().all(|(event, _)| {
        doc.get("hooks")
            .and_then(|h| h.get(*event))
            .and_then(Value::as_array)
            .is_some_and(|entries| {
                commands_in(tool, entries)
                    .iter()
                    .any(|c| is_quacko(c) && c.contains(flag))
            })
    })
}

/// Tira as entradas do Quacko de um evento; grupos que ficam vazios somem.
fn strip_quacko(tool: Tool, entries: &mut Vec<Value>) {
    match tool {
        Tool::Claude => {
            entries.retain_mut(|group| {
                let Some(hooks) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                    return true;
                };
                let before = hooks.len();
                hooks.retain(|h| {
                    !h.get("command")
                        .and_then(Value::as_str)
                        .is_some_and(is_quacko)
                });
                !(hooks.is_empty() && before > 0)
            });
        }
        Tool::Cursor => entries.retain(|h| {
            !h.get("command")
                .and_then(Value::as_str)
                .is_some_and(is_quacko)
        }),
    }
}

fn new_entry(tool: Tool, command: String) -> Value {
    match tool {
        Tool::Claude => json!({ "hooks": [{ "type": "command", "command": command }] }),
        Tool::Cursor => json!({ "command": command }),
    }
}

/// Aplica a conexão (ou desconexão) num documento. Erro = formato inesperado: modo
/// manual, e o documento fica exatamente como estava (a mudança é feita numa cópia).
pub fn apply(
    tool: Tool,
    doc: &mut Value,
    notify: &Path,
    connect: bool,
) -> Result<(), ManualReason> {
    let mut work = doc.clone();
    apply_in_place(tool, &mut work, notify, connect)?;
    *doc = work;
    Ok(())
}

fn apply_in_place(
    tool: Tool,
    doc: &mut Value,
    notify: &Path,
    connect: bool,
) -> Result<(), ManualReason> {
    let root = doc.as_object_mut().ok_or(ManualReason::Invalid)?;
    if tool == Tool::Cursor && connect && !root.contains_key("version") {
        root.insert("version".into(), json!(1));
    }
    if !root.contains_key("hooks") {
        if !connect {
            return Ok(());
        }
        root.insert("hooks".into(), Value::Object(Map::new()));
    }
    let hooks = root
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or(ManualReason::Invalid)?;

    for (event, args) in events(tool) {
        if !hooks.contains_key(*event) {
            if !connect {
                continue;
            }
            hooks.insert((*event).into(), Value::Array(Vec::new()));
        }
        let entries = hooks
            .get_mut(*event)
            .and_then(Value::as_array_mut)
            .ok_or(ManualReason::Invalid)?;
        strip_quacko(tool, entries);
        if connect {
            entries.push(new_entry(tool, command_for(notify, args)));
        }
    }
    if !connect {
        // Só limpa os eventos que o Quacko mexe; listas vazias de outros eventos ficam.
        let ours: Vec<&str> = events(tool).iter().map(|(e, _)| *e).collect();
        hooks.retain(|k, v| {
            !(ours.contains(&k.as_str()) && v.as_array().is_some_and(Vec::is_empty))
        });
        if hooks.is_empty() {
            root.remove("hooks");
        }
    }
    Ok(())
}

/// Trecho para colar à mão (modo manual).
pub fn snippet(tool: Tool, notify: &Path) -> String {
    let mut doc = json!({});
    let _ = apply(tool, &mut doc, notify, true);
    serde_json::to_string_pretty(&doc).unwrap_or_default()
}

// ---------- arquivos ----------

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Caminho real a gravar e o conteúdo atual; `Err` = modo manual.
fn prepare(path: &Path) -> Result<(PathBuf, Value, bool), ManualReason> {
    if !path.exists() && fs::symlink_metadata(path).is_err() {
        return Ok((path.to_path_buf(), json!({}), false));
    }
    let target = fs::canonicalize(path).map_err(|_| ManualReason::ReadOnly)?;
    if target.starts_with("/nix/store") {
        return Err(ManualReason::Managed);
    }
    let writable = fs::OpenOptions::new().append(true).open(&target).is_ok()
        && target.parent().is_some_and(|dir| tempfile_in(dir).is_ok());
    if !writable {
        return Err(ManualReason::ReadOnly);
    }
    let text = fs::read_to_string(&target).map_err(|_| ManualReason::ReadOnly)?;
    let doc = if text.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&text).map_err(|_| ManualReason::Invalid)?
    };
    Ok((target, doc, true))
}

/// Testa se dá para criar arquivo na pasta (o rename atômico precisa disso).
fn tempfile_in(dir: &Path) -> io::Result<()> {
    let probe = dir.join(format!(".quacko-probe-{}", std::process::id()));
    fs::File::create(&probe)?;
    fs::remove_file(&probe)
}

fn write_atomic(path: &Path, contents: &str) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "caminho sem pasta"))?;
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.quacko-tmp-{}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("arquivo"),
        std::process::id()
    ));
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
    }
    if let Ok(meta) = fs::metadata(path) {
        let _ = fs::set_permissions(&tmp, meta.permissions());
    }
    fs::rename(&tmp, path)
}

/// Instala um script com permissão de execução; guarda backup se havia outro diferente.
fn install_script(path: &Path, contents: &str) -> io::Result<()> {
    if let Ok(current) = fs::read_to_string(path) {
        if current == contents {
            return Ok(());
        }
        // O primeiro backup guarda o original; os seguintes ganham data.
        let mut backup = PathBuf::from(format!("{}.bak", path.display()));
        if backup.exists() {
            backup = PathBuf::from(format!("{}.bak-{}", path.display(), timestamp()));
        }
        fs::copy(path, &backup)?;
    }
    write_atomic(path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// Lê a config e diz se o Quacko já está ligado nela (para a marca do menu).
pub fn status(paths: &Paths, tool: Tool) -> bool {
    match prepare(&paths.config_file(tool)) {
        Ok((_, doc, _)) => is_connected(tool, &doc),
        Err(_) => fs::read_to_string(paths.config_file(tool))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .is_some_and(|doc| is_connected(tool, &doc)),
    }
}

/// Conecta ou desconecta uma ferramenta.
pub fn set_connected(paths: &Paths, tool: Tool, connect: bool) -> io::Result<Outcome> {
    let notify = paths.notify_sh();
    if connect {
        install_script(&notify, NOTIFY_SH)?;
    }
    let file = paths.config_file(tool);
    let (target, mut doc, existed) = match prepare(&file) {
        Ok(found) => found,
        Err(reason) => {
            return Ok(Outcome::Manual {
                reason,
                file,
                snippet: snippet(tool, &notify),
            });
        }
    };
    if let Err(reason) = apply(tool, &mut doc, &notify, connect) {
        return Ok(Outcome::Manual {
            reason,
            file,
            snippet: snippet(tool, &notify),
        });
    }
    if existed {
        let name = target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("config");
        fs::copy(
            &target,
            target.with_file_name(format!("{name}.quacko-backup-{}", timestamp())),
        )?;
    }
    let mut text = serde_json::to_string_pretty(&doc).map_err(io::Error::other)?;
    text.push('\n');
    write_atomic(&target, &text)?;
    Ok(if connect {
        Outcome::Connected
    } else {
        Outcome::Disconnected
    })
}

/// Marca presente nos scripts do Quacko (variáveis QUACKO_*): só esses são atualizados.
const OURS_MARKER: &str = "QUACKO_";
/// Scripts instalados pela versão com o nome antigo (variáveis MIAU_*): também são nossos.
const LEGACY_MARKER: &str = "MIAU_";

/// Ao abrir o app: se o `notify.sh` ou o `prwatch` instalados forem do Quacko mas de outra
/// versão, troca pelos embutidos (com backup). Scripts que não são do Quacko, ou que não
/// existem (a pessoa nunca conectou), ficam como estão. Devolve os nomes atualizados.
pub fn sync_installed_scripts(paths: &Paths) -> Vec<String> {
    let mut updated = Vec::new();
    for (path, embedded) in [(paths.notify_sh(), NOTIFY_SH), (paths.prwatch(), PRWATCH)] {
        let Ok(current) = fs::read_to_string(&path) else {
            continue;
        };
        if current == embedded
            || !(current.contains(OURS_MARKER) || current.contains(LEGACY_MARKER))
        {
            continue;
        }
        match install_script(&path, embedded) {
            Ok(()) => updated.push(
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("script")
                    .to_string(),
            ),
            Err(e) => {
                tracing::warn!(%e, path = %path.display(), "não foi possível atualizar o script")
            }
        }
    }
    updated
}

pub enum PrwatchOutcome {
    /// Link criado (ou já existia) em ~/.local/bin.
    Linked,
    /// Já existe outro `prwatch` em ~/.local/bin que não é nosso: não mexemos.
    Conflict,
}

/// Instala o `prwatch` e cria o link `~/.local/bin/prwatch` (sem pedir administrador).
pub fn install_prwatch(paths: &Paths) -> io::Result<PrwatchOutcome> {
    install_script(&paths.notify_sh(), NOTIFY_SH)?;
    install_script(&paths.prwatch(), PRWATCH)?;
    let link = paths.local_bin_prwatch();
    if let Some(dir) = link.parent() {
        fs::create_dir_all(dir)?;
    }
    match fs::read_link(&link) {
        Ok(dest) if dest == paths.prwatch() => return Ok(PrwatchOutcome::Linked),
        Ok(_) => return Ok(PrwatchOutcome::Conflict),
        Err(_) if link.exists() => return Ok(PrwatchOutcome::Conflict),
        Err(_) => {}
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(paths.prwatch(), &link)?;
    Ok(PrwatchOutcome::Linked)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTIFY: &str = "/Users/ana/.claude/hooks/notify.sh";

    fn notify() -> PathBuf {
        PathBuf::from(NOTIFY)
    }

    fn connect(tool: Tool, doc: &mut Value) {
        apply(tool, doc, &notify(), true).unwrap();
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
        // Formato da v1: notify.sh só com o som, sem --from.
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
        apply(Tool::Claude, &mut doc, &notify(), false).unwrap();
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
        apply(Tool::Claude, &mut doc, &notify(), false).unwrap();
        assert_eq!(doc, original);

        let mut only_quacko = json!({});
        connect(Tool::Claude, &mut only_quacko);
        apply(Tool::Claude, &mut only_quacko, &notify(), false).unwrap();
        assert_eq!(only_quacko, json!({}));
    }

    #[test]
    fn desconectar_nao_apaga_lista_vazia_de_outro_evento() {
        let mut doc = json!({"hooks": {"PreCompact": []}});
        connect(Tool::Claude, &mut doc);
        apply(Tool::Claude, &mut doc, &notify(), false).unwrap();
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
                apply(Tool::Claude, &mut doc, &notify(), true),
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
        apply(Tool::Cursor, &mut doc, &notify(), false).unwrap();
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

    // ---------- com arquivos de verdade ----------

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
        assert!(
            NOTIFY_SH.contains(OURS_MARKER) && PRWATCH.contains(OURS_MARKER),
            "a sincronização depende da marca"
        );
    }
}
