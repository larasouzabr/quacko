//! Token do GitHub CLI (`gh`), achado mesmo sem o PATH do terminal.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// ---------- token via gh ----------

/// Procura o `gh`: lugares comuns, o shell de login da pessoa e, por último, o Nix.
pub fn find_gh(home: &Path) -> Option<PathBuf> {
    if let Some(custom) = std::env::var_os("QUACKO_GH").map(PathBuf::from) {
        if custom.is_file() {
            return Some(custom);
        }
    }
    let user = std::env::var("USER").unwrap_or_default();
    let candidates = [
        PathBuf::from("/opt/homebrew/bin/gh"),
        PathBuf::from("/usr/local/bin/gh"),
        home.join(".nix-profile/bin/gh"),
        PathBuf::from(format!("/etc/profiles/per-user/{user}/bin/gh")),
        PathBuf::from("/run/current-system/sw/bin/gh"),
        home.join(".local/bin/gh"),
    ];
    if let Some(found) = candidates.into_iter().find(|p| p.is_file()) {
        return Some(found);
    }
    from_login_shell().or_else(newest_in_nix_store)
}

fn from_login_shell() -> Option<PathBuf> {
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/zsh".into());
    let out = run_with_timeout(
        Command::new(shell).args(["-l", "-i", "-c", "command -v gh"]),
        Duration::from_secs(4),
    )?;
    out.lines()
        .rev()
        .map(str::trim)
        .find(|l| l.starts_with('/'))
        .map(PathBuf::from)
        .filter(|p| p.is_file())
}

/// `/nix/store/<hash>-gh-<versão>/bin/gh` com a maior versão.
fn newest_in_nix_store() -> Option<PathBuf> {
    let entries = std::fs::read_dir("/nix/store").ok()?;
    entries
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|name| {
            let version = name.split_once("-gh-")?.1;
            let parts: Vec<u64> = version
                .split('.')
                .map(|p| p.parse().ok())
                .collect::<Option<_>>()?;
            let bin = Path::new("/nix/store").join(&name).join("bin/gh");
            bin.is_file().then_some((parts, bin))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, bin)| bin)
}

pub fn gh_token(gh: &Path) -> Option<String> {
    let out = run_with_timeout(
        Command::new(gh).args(["auth", "token", "--hostname", "github.com"]),
        Duration::from_secs(5),
    )?;
    let token = out.trim().to_string();
    (!token.is_empty() && !token.contains(char::is_whitespace)).then_some(token)
}

/// Roda um comando com tempo máximo; devolve o stdout se deu certo.
fn run_with_timeout(cmd: &mut Command, limit: Duration) -> Option<String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if start.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
        }
    }
    let mut out = String::new();
    use std::io::Read;
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    Some(out)
}
