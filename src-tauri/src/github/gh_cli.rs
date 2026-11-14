use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn find_gh(home: &Path) -> Option<PathBuf> {
    if let Some(custom) = std::env::var_os("QUACKO_GH").filter(|v| !v.is_empty()) {
        let custom = PathBuf::from(custom);
        return custom.is_file().then_some(custom);
    }
    if cfg!(windows) {
        return windows_candidates(home, |key| std::env::var_os(key))
            .into_iter()
            .find(|p| p.is_file())
            .or_else(|| in_path(std::env::var_os("PATH")?, "gh.exe"));
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

pub fn windows_candidates(home: &Path, env: impl Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    let dir = |key: &str, parts: &[&str]| {
        env(key).map(|root| {
            parts
                .iter()
                .fold(PathBuf::from(root), |p, part| p.join(part))
        })
    };
    [
        dir("ProgramFiles", &["GitHub CLI", "gh.exe"]),
        dir("ProgramFiles(x86)", &["GitHub CLI", "gh.exe"]),
        dir("LOCALAPPDATA", &["Programs", "GitHub CLI", "gh.exe"]),
        Some(home.join("scoop").join("shims").join("gh.exe")),
        dir("ProgramData", &["chocolatey", "bin", "gh.exe"]),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub fn in_path(path: OsString, name: &str) -> Option<PathBuf> {
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
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

pub(crate) fn run_with_timeout(cmd: &mut Command, limit: Duration) -> Option<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    without_terminal(cmd);
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

#[cfg(unix)]
fn without_terminal(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn quacko_gh_pointing_nowhere_means_no_gh() {
        std::env::set_var("QUACKO_GH", "/nao/existe/gh");
        let found = find_gh(Path::new("/"));
        std::env::remove_var("QUACKO_GH");
        assert_eq!(found, None);
    }

    #[test]
    fn child_runs_in_its_own_session_without_the_terminal() {
        let out = run_with_timeout(
            Command::new("/bin/sh")
                .args(["-c", "echo $$ $(ps -o pgid= -p $$) $(ps -o tty= -p $$)"]),
            Duration::from_secs(5),
        )
        .expect("sh roda");
        let parts: Vec<&str> = out.split_whitespace().collect();
        assert_eq!(parts[0], parts[1], "vira líder do próprio grupo: {out}");
        assert!(parts[2].starts_with('?'), "sem terminal: {out}");
    }
}
