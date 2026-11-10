use std::fs;
use std::io::{self};
use std::path::PathBuf;

use super::files::install_script;
use super::*;

pub(super) const NOTIFY_SH: &str = include_str!("../../../scripts/notify.sh");
pub(super) const PRWATCH: &str = include_str!("../../../scripts/prwatch");
pub(super) const QUACKO_ZSH: &str = include_str!("../../../scripts/quacko.zsh");
pub(super) const QUACKO_BASH: &str = include_str!("../../../scripts/quacko.bash");
pub(super) const QUACKO_PS1: &str = include_str!("../../../scripts/quacko.ps1");

pub(super) const OURS_MARKER: &str = "QUACKO_";
pub(super) const LEGACY_MARKER: &str = "MIAU_";

pub fn sync_installed_scripts(paths: &Paths) -> Vec<String> {
    let mut updated = Vec::new();
    for (path, embedded) in [
        (paths.notify_sh(), NOTIFY_SH),
        (paths.prwatch(), PRWATCH),
        (paths.quacko_zsh(), QUACKO_ZSH),
        (paths.hooks_dir().join("quacko.bash"), QUACKO_BASH),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Zsh,
    Bash,
    Pwsh,
}

impl Shell {
    pub fn detect(windows: bool, login_shell: Option<&str>) -> Self {
        if windows {
            Shell::Pwsh
        } else if login_shell.is_some_and(|s| s.trim_end().ends_with("bash")) {
            Shell::Bash
        } else {
            Shell::Zsh
        }
    }

    pub fn file_name(self) -> &'static str {
        match self {
            Shell::Zsh => "quacko.zsh",
            Shell::Bash => "quacko.bash",
            Shell::Pwsh => "quacko.ps1",
        }
    }

    pub fn line(self) -> &'static str {
        match self {
            Shell::Zsh => "source ~/.claude/hooks/quacko.zsh",
            Shell::Bash => "source ~/.claude/hooks/quacko.bash",
            Shell::Pwsh => r#". "$HOME\.claude\hooks\quacko.ps1""#,
        }
    }

    pub fn rc_name(self) -> &'static str {
        match self {
            Shell::Zsh => "~/.zshrc",
            Shell::Bash => "~/.bash_profile",
            Shell::Pwsh => "$PROFILE",
        }
    }

    fn rc_files(self, home: &std::path::Path) -> Vec<PathBuf> {
        match self {
            Shell::Zsh => vec![home.join(".zshrc")],
            Shell::Bash => vec![home.join(".bash_profile"), home.join(".bashrc")],
            Shell::Pwsh => ["PowerShell", "WindowsPowerShell"]
                .iter()
                .map(|dir| {
                    home.join("Documents")
                        .join(dir)
                        .join("Microsoft.PowerShell_profile.ps1")
                })
                .collect(),
        }
    }
}

pub fn render_powershell(hook_exe: &std::path::Path) -> String {
    QUACKO_PS1.replace(
        "__QUACKO_HOOK__",
        &hook_exe.to_string_lossy().replace('\'', "''"),
    )
}

pub enum ShellOutcome {
    Copied { line: String, nix: bool },
    AlreadySourced,
}

pub fn install_shell(
    paths: &Paths,
    shell: Shell,
    hook_exe: Option<&std::path::Path>,
) -> io::Result<ShellOutcome> {
    let target = paths.hooks_dir().join(shell.file_name());
    match shell {
        Shell::Zsh => install_script(&target, QUACKO_ZSH)?,
        Shell::Bash => install_script(&target, QUACKO_BASH)?,
        Shell::Pwsh => {
            let exe = hook_exe.ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "quacko-hook.exe não encontrado")
            })?;
            install_script(&target, &render_powershell(exe))?
        }
    }
    if shell != Shell::Pwsh {
        install_script(&paths.notify_sh(), NOTIFY_SH)?;
    }
    let rc_files = shell.rc_files(&paths.home);
    let sourced = rc_files.iter().any(|file| {
        fs::read_to_string(file).is_ok_and(|text| {
            text.lines()
                .map(str::trim_start)
                .filter(|line| !line.starts_with('#'))
                .any(|line| {
                    line.replace('\\', "/")
                        .contains(&format!(".claude/hooks/{}", shell.file_name()))
                })
        })
    });
    Ok(if sourced {
        ShellOutcome::AlreadySourced
    } else {
        ShellOutcome::Copied {
            line: shell.line().to_string(),
            nix: fs::read_link(&rc_files[0]).is_ok_and(|dest| dest.starts_with("/nix/store")),
        }
    })
}

pub fn sync_powershell(paths: &Paths, hook_exe: &std::path::Path) -> bool {
    let path = paths.hooks_dir().join(Shell::Pwsh.file_name());
    let Ok(current) = fs::read_to_string(&path) else {
        return false;
    };
    let wanted = render_powershell(hook_exe);
    current != wanted && current.contains(OURS_MARKER) && install_script(&path, &wanted).is_ok()
}
