//! Terminal no Windows: a política de scripts do PowerShell e o `$PROFILE`.
//!
//! O Windows PowerShell 5.1 vem com scripts bloqueados (`Restricted`), então o
//! `quacko.ps1` só carrega depois de liberar scripts locais para o usuário.
//! Este módulo pergunta ao PowerShell como ele está configurado, decide o que
//! falta e acrescenta a linha do Quacko ao perfil sem estragar o arquivo.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::files::{timestamp, write_atomic_bytes};

/// Hosts do PowerShell que o Quacko configura: o 5.1, que vem no Windows, e o 7, se estiver instalado.
pub const HOSTS: [&str; 2] = ["powershell.exe", "pwsh.exe"];

/// Linha que carrega o hook no perfil.
pub const LINE: &str = r#". "$HOME\.claude\hooks\quacko.ps1""#;

/// Pergunta a política de cada escopo e o caminho real do perfil (que pode estar no OneDrive).
pub const PROBE: &str = concat!(
    "[Console]::OutputEncoding = [Text.Encoding]::UTF8; ",
    "Get-ExecutionPolicy -List | ForEach-Object { \"policy:$($_.Scope)=$($_.ExecutionPolicy)\" }; ",
    "\"profile:$($PROFILE.CurrentUserCurrentHost)\""
);

/// Libera scripts locais só para o usuário atual. Não precisa de administrador.
pub const UNBLOCK: &str = "Set-ExecutionPolicy -Scope CurrentUser RemoteSigned -Force";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Probe {
    pub scopes: Vec<(String, String)>,
    pub profile: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Policy {
    /// Scripts locais já rodam.
    Allows,
    /// Bloqueado, mas o usuário pode liberar no escopo `CurrentUser`.
    Blocked,
    /// Bloqueado por política de grupo (empresa): não muda daqui.
    Enforced(String),
}

pub fn parse_probe(out: &str) -> Probe {
    let mut probe = Probe::default();
    for line in out.lines().map(|l| l.trim().trim_start_matches('\u{feff}')) {
        if let Some(rest) = line.strip_prefix("policy:") {
            if let Some((scope, value)) = rest.split_once('=') {
                probe
                    .scopes
                    .push((scope.trim().to_string(), value.trim().to_string()));
            }
        } else if let Some(path) = line.strip_prefix("profile:") {
            let path = path.trim();
            if !path.is_empty() {
                probe.profile = Some(PathBuf::from(path));
            }
        }
    }
    probe
}

fn blocks(value: &str) -> bool {
    ["Restricted", "AllSigned", "Default"]
        .iter()
        .any(|v| v.eq_ignore_ascii_case(value))
}

fn undefined(value: &str) -> bool {
    value.is_empty() || value.eq_ignore_ascii_case("Undefined")
}

/// Política efetiva para scripts locais, na ordem de precedência do PowerShell.
/// `None` quando a resposta não trouxe nenhum escopo (o PowerShell não respondeu direito).
pub fn policy(probe: &Probe) -> Option<Policy> {
    if probe.scopes.is_empty() {
        return None;
    }
    let get = |scope: &str| {
        probe
            .scopes
            .iter()
            .find(|(s, _)| s.eq_ignore_ascii_case(scope))
            .map(|(_, v)| v.as_str())
            .unwrap_or("Undefined")
    };
    for scope in ["MachinePolicy", "UserPolicy"] {
        let value = get(scope);
        if !undefined(value) {
            return Some(if blocks(value) {
                Policy::Enforced(value.to_string())
            } else {
                Policy::Allows
            });
        }
    }
    // O escopo Process é o do PowerShell que fez a pergunta, não o do terminal da pessoa.
    for scope in ["CurrentUser", "LocalMachine"] {
        let value = get(scope);
        if !undefined(value) {
            return Some(if blocks(value) {
                Policy::Blocked
            } else {
                Policy::Allows
            });
        }
    }
    // Tudo indefinido: no Windows de usuário o padrão é Restricted.
    Some(Policy::Blocked)
}

/// O texto do perfil já carrega o `quacko.ps1` numa linha que não é comentário?
pub fn sources_quacko(text: &str) -> bool {
    text.lines()
        .map(str::trim_start)
        .filter(|line| !line.starts_with('#'))
        .any(|line| line.replace('\\', "/").contains(".claude/hooks/quacko.ps1"))
}

/// Lê o perfil em qualquer codificação que o PowerShell aceita (UTF-8, ANSI ou UTF-16).
pub fn decode(bytes: &[u8]) -> String {
    let utf16 = |be: bool| {
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| {
                if be {
                    u16::from_be_bytes(pair)
                } else {
                    u16::from_le_bytes(pair)
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, ..] => utf16(false),
        [0xFE, 0xFF, ..] => utf16(true),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// O arquivo do perfil com a linha do Quacko no fim, mantendo a codificação de quem já existe.
pub fn with_line(existing: &[u8]) -> Vec<u8> {
    let block = if existing.is_empty() {
        format!("# Quacko: avisa quando um comando longo termina\r\n{LINE}\r\n")
    } else {
        format!("\r\n# Quacko: avisa quando um comando longo termina\r\n{LINE}\r\n")
    };
    let mut out = existing.to_vec();
    match existing {
        [0xFF, 0xFE, ..] => out.extend(block.encode_utf16().flat_map(u16::to_le_bytes)),
        [0xFE, 0xFF, ..] => out.extend(block.encode_utf16().flat_map(u16::to_be_bytes)),
        // UTF-8 (com ou sem BOM) e ANSI: a linha só tem ASCII, que é igual nos dois.
        _ => out.extend_from_slice(block.as_bytes()),
    }
    out
}

/// Acrescenta a linha ao perfil, criando pasta e arquivo se faltarem e guardando backup do que existia.
/// Não faz nada se a linha já estiver lá.
pub fn add_to_profile(profile: &Path) -> io::Result<bool> {
    let existing = match fs::read(profile) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    if sources_quacko(&decode(&existing)) {
        return Ok(false);
    }
    if !existing.is_empty() {
        let backup = PathBuf::from(format!(
            "{}.quacko-backup-{}",
            profile.display(),
            timestamp()
        ));
        fs::write(backup, &existing)?;
    }
    write_atomic_bytes(profile, &with_line(&existing))?;
    Ok(true)
}

/// Roda um comando no PowerShell indicado, sem abrir janela. `None` se ele não existir ou falhar.
pub fn run(host: &str, script: &str) -> Option<String> {
    crate::github::gh_cli::run_with_timeout(
        Command::new(host).args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ]),
        Duration::from_secs(20),
    )
}

/// Como cada PowerShell instalado está configurado. Só entram os que responderam.
pub fn probe_hosts() -> Vec<(&'static str, Probe)> {
    HOSTS
        .iter()
        .filter_map(|host| {
            let probe = parse_probe(&run(host, PROBE)?);
            policy(&probe).is_some().then_some((*host, probe))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(scopes: &[(&str, &str)]) -> Probe {
        Probe {
            scopes: scopes
                .iter()
                .map(|(s, v)| (s.to_string(), v.to_string()))
                .collect(),
            profile: None,
        }
    }

    #[test]
    fn le_a_resposta_do_powershell() {
        let out = "\u{feff}policy:MachinePolicy=Undefined\r\npolicy:UserPolicy=Undefined\r\npolicy:Process=Undefined\r\npolicy:CurrentUser=Undefined\r\npolicy:LocalMachine=Undefined\r\nprofile:C:\\Users\\joão\\OneDrive\\Documentos\\WindowsPowerShell\\Microsoft.PowerShell_profile.ps1\r\n";
        let p = parse_probe(out);
        assert_eq!(p.scopes.len(), 5);
        assert_eq!(
            p.profile.unwrap().to_string_lossy(),
            "C:\\Users\\joão\\OneDrive\\Documentos\\WindowsPowerShell\\Microsoft.PowerShell_profile.ps1"
        );
        assert_eq!(parse_probe("lixo\n").scopes.len(), 0);
    }

    #[test]
    fn politica_efetiva() {
        let all_undefined = probe(&[
            ("MachinePolicy", "Undefined"),
            ("UserPolicy", "Undefined"),
            ("Process", "Undefined"),
            ("CurrentUser", "Undefined"),
            ("LocalMachine", "Undefined"),
        ]);
        assert_eq!(
            policy(&all_undefined),
            Some(Policy::Blocked),
            "padrão do Windows"
        );
        assert_eq!(
            policy(&probe(&[
                ("CurrentUser", "RemoteSigned"),
                ("LocalMachine", "Restricted")
            ])),
            Some(Policy::Allows),
            "CurrentUser vence LocalMachine"
        );
        assert_eq!(
            policy(&probe(&[
                ("CurrentUser", "Undefined"),
                ("LocalMachine", "RemoteSigned")
            ])),
            Some(Policy::Allows),
            "PowerShell 7 e máquinas já liberadas"
        );
        assert_eq!(
            policy(&probe(&[("CurrentUser", "AllSigned")])),
            Some(Policy::Blocked),
            "AllSigned barra o quacko.ps1, que não é assinado"
        );
        assert_eq!(
            policy(&probe(&[
                ("MachinePolicy", "Restricted"),
                ("CurrentUser", "RemoteSigned")
            ])),
            Some(Policy::Enforced("Restricted".into())),
            "política de grupo vence o usuário"
        );
        assert_eq!(
            policy(&probe(&[
                ("UserPolicy", "RemoteSigned"),
                ("CurrentUser", "Restricted")
            ])),
            Some(Policy::Allows)
        );
        assert_eq!(
            policy(&probe(&[("Process", "Bypass")])),
            Some(Policy::Blocked),
            "o escopo Process é do PowerShell da pergunta, não conta"
        );
        assert_eq!(policy(&Probe::default()), None);
    }

    #[test]
    fn reconhece_a_linha_no_perfil() {
        assert!(sources_quacko(
            "oh-my-posh init\r\n. \"$HOME\\.claude\\hooks\\quacko.ps1\"\r\n"
        ));
        assert!(sources_quacko("  . C:/Users/x/.claude/hooks/quacko.ps1"));
        assert!(!sources_quacko("# . \"$HOME\\.claude\\hooks\\quacko.ps1\""));
        assert!(!sources_quacko(""));
    }

    #[test]
    fn acrescenta_mantendo_a_codificacao() {
        let novo = with_line(b"");
        assert!(String::from_utf8(novo).unwrap().starts_with("# Quacko"));

        let utf8 = with_line(b"\xEF\xBB\xBFoh-my-posh init");
        assert!(utf8.starts_with(b"\xEF\xBB\xBFoh-my-posh init\r\n# Quacko"));
        assert!(sources_quacko(&decode(&utf8)));

        let mut le = vec![0xFF, 0xFE];
        le.extend("Set-Alias g git".encode_utf16().flat_map(u16::to_le_bytes));
        let le = with_line(&le);
        assert_eq!(le.len() % 2, 0);
        let text = decode(&le);
        assert!(text.starts_with("Set-Alias g git\r\n# Quacko"), "{text:?}");
        assert!(sources_quacko(&text));

        let mut be = vec![0xFE, 0xFF];
        be.extend("x".encode_utf16().flat_map(u16::to_be_bytes));
        assert!(sources_quacko(&decode(&with_line(&be))));
    }

    #[test]
    fn edita_o_perfil_com_backup_e_sem_repetir() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir
            .path()
            .join("OneDrive/Documentos/WindowsPowerShell/Microsoft.PowerShell_profile.ps1");

        assert!(add_to_profile(&profile).unwrap(), "cria pasta e arquivo");
        assert!(sources_quacko(&fs::read_to_string(&profile).unwrap()));
        assert!(!add_to_profile(&profile).unwrap(), "não repete a linha");

        fs::write(&profile, "Import-Module posh-git\r\n").unwrap();
        assert!(add_to_profile(&profile).unwrap());
        let text = fs::read_to_string(&profile).unwrap();
        assert!(text.starts_with("Import-Module posh-git\r\n"));
        assert!(sources_quacko(&text));
        let backups: Vec<_> = fs::read_dir(profile.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".quacko-backup-"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            fs::read_to_string(backups[0].path()).unwrap(),
            "Import-Module posh-git\r\n"
        );
    }
}
