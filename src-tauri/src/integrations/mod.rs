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

pub mod actions;
pub mod files;
pub mod hooks;
pub mod scripts;

#[cfg(test)]
mod tests;

use std::fs;
use std::io::{self};
use std::path::PathBuf;

use files::{install_script, prepare, timestamp, write_atomic};
use hooks::{apply, is_connected, snippet};
use scripts::NOTIFY_SH;
use serde_json::Value;

pub use scripts::{install_prwatch, sync_installed_scripts, PrwatchOutcome};

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
