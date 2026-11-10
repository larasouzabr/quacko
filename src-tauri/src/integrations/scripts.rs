use std::fs;
use std::io::{self};

use super::files::install_script;
use super::*;

pub(super) const NOTIFY_SH: &str = include_str!("../../../scripts/notify.sh");
pub(super) const PRWATCH: &str = include_str!("../../../scripts/prwatch");
pub(super) const QUACKO_ZSH: &str = include_str!("../../../scripts/quacko.zsh");

pub(super) const OURS_MARKER: &str = "QUACKO_";
pub(super) const LEGACY_MARKER: &str = "MIAU_";

pub fn sync_installed_scripts(paths: &Paths) -> Vec<String> {
    let mut updated = Vec::new();
    for (path, embedded) in [
        (paths.notify_sh(), NOTIFY_SH),
        (paths.prwatch(), PRWATCH),
        (paths.quacko_zsh(), QUACKO_ZSH),
    ] {
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
    Linked,
    Conflict,
}

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

pub enum ShellOutcome {
    Copied { line: String, nix: bool },
    AlreadySourced,
}

pub fn shell_line() -> &'static str {
    "source ~/.claude/hooks/quacko.zsh"
}

pub fn install_shell(paths: &Paths) -> io::Result<ShellOutcome> {
    install_script(&paths.notify_sh(), NOTIFY_SH)?;
    install_script(&paths.quacko_zsh(), QUACKO_ZSH)?;
    let zshrc_path = paths.home.join(".zshrc");
    let zshrc = fs::read_to_string(&zshrc_path).unwrap_or_default();
    let sourced = zshrc
        .lines()
        .map(str::trim_start)
        .filter(|line| !line.starts_with('#'))
        .any(|line| line.contains(".claude/hooks/quacko.zsh"));
    Ok(if sourced {
        ShellOutcome::AlreadySourced
    } else {
        ShellOutcome::Copied {
            line: shell_line().to_string(),
            nix: fs::read_link(&zshrc_path).is_ok_and(|dest| dest.starts_with("/nix/store")),
        }
    })
}
