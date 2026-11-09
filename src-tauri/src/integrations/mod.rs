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
use hooks::{apply, commands_in, is_connected, snippet};
use scripts::NOTIFY_SH;
use serde_json::Value;

pub use hooks::Hook;
pub use scripts::{install_prwatch, sync_installed_scripts, PrwatchOutcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Claude,
    Cursor,
}

impl Tool {
    pub const ALL: [Tool; 2] = [Tool::Claude, Tool::Cursor];

    pub fn flag_name(self) -> &'static str {
        match self {
            Tool::Claude => "claude",
            Tool::Cursor => "cursor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualReason {
    Managed,
    ReadOnly,
    Invalid,
}

#[derive(Debug)]
pub enum Outcome {
    Connected,
    Disconnected,
    Manual {
        reason: ManualReason,
        file: PathBuf,
        snippet: String,
    },
}

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
    pub fn cursor_present(&self) -> bool {
        self.home.join(".cursor").is_dir()
    }
}

pub fn hook_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.with_file_name(format!(
        "{}{}",
        hooks::HOOK_EXE_NAME,
        std::env::consts::EXE_SUFFIX
    )))
}

pub fn default_hook(paths: &Paths) -> Option<Hook> {
    if cfg!(target_os = "macos") {
        Some(Hook::Script(paths.notify_sh()))
    } else {
        hook_exe().map(Hook::Exe)
    }
}

pub fn status(paths: &Paths, tool: Tool) -> bool {
    match prepare(&paths.config_file(tool)) {
        Ok((_, doc, _)) => is_connected(tool, &doc),
        Err(_) => fs::read_to_string(paths.config_file(tool))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .is_some_and(|doc| is_connected(tool, &doc)),
    }
}

pub fn set_connected(paths: &Paths, tool: Tool, connect: bool) -> io::Result<Outcome> {
    let hook = default_hook(paths)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "executável do app"))?;
    connect_with(paths, tool, connect, &hook)
}

pub fn connect_with(paths: &Paths, tool: Tool, connect: bool, hook: &Hook) -> io::Result<Outcome> {
    if connect {
        match hook {
            Hook::Script(path) => install_script(path, NOTIFY_SH)?,
            Hook::Exe(path) if !path.is_file() => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("{} não encontrado", path.display()),
                ));
            }
            Hook::Exe(_) => {}
        }
    }
    let file = paths.config_file(tool);
    let (target, mut doc, existed) = match prepare(&file) {
        Ok(found) => found,
        Err(reason) => {
            return Ok(Outcome::Manual {
                reason,
                file,
                snippet: snippet(tool, hook),
            });
        }
    };
    if let Err(reason) = apply(tool, &mut doc, hook, connect) {
        return Ok(Outcome::Manual {
            reason,
            file,
            snippet: snippet(tool, hook),
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

pub fn refresh_exe_hooks(paths: &Paths, exe: &std::path::Path) -> Vec<Tool> {
    let current = exe.to_string_lossy();
    Tool::ALL
        .into_iter()
        .filter(|tool| status(paths, *tool))
        .filter(|tool| {
            let commands: Vec<String> = fs::read_to_string(paths.config_file(*tool))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|doc| doc.get("hooks").and_then(Value::as_object).cloned())
                .map(|hooks| {
                    hooks
                        .values()
                        .filter_map(Value::as_array)
                        .flat_map(|entries| commands_in(*tool, entries))
                        .collect()
                })
                .unwrap_or_default();
            let ours: Vec<&String> = commands.iter().filter(|c| hooks::is_quacko(c)).collect();
            !ours.is_empty() && ours.iter().any(|c| !c.contains(current.as_ref()))
        })
        .filter(|tool| {
            matches!(
                connect_with(paths, *tool, true, &Hook::Exe(exe.to_path_buf())),
                Ok(Outcome::Connected)
            )
        })
        .collect()
}
