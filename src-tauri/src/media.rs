use std::path::{Path, PathBuf};

pub fn find_nowplaying_cli(home: &Path) -> Option<PathBuf> {
    let user = std::env::var("USER").unwrap_or_default();
    [
        PathBuf::from("/opt/homebrew/bin/nowplaying-cli"),
        PathBuf::from("/usr/local/bin/nowplaying-cli"),
        home.join(".nix-profile/bin/nowplaying-cli"),
        PathBuf::from(format!("/etc/profiles/per-user/{user}/bin/nowplaying-cli")),
        PathBuf::from("/run/current-system/sw/bin/nowplaying-cli"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

pub fn should_warn(agent_connected: bool, nowplaying_found: bool, already_warned: bool) -> bool {
    agent_connected && !nowplaying_found && !already_warned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avisa_so_quando_faz_diferenca_e_uma_vez() {
        assert!(should_warn(true, false, false));
        assert!(
            !should_warn(false, false, false),
            "sem agente conectado, a pausa não importa"
        );
        assert!(!should_warn(true, true, false), "já instalado");
        assert!(!should_warn(true, false, true), "já avisado");
    }

    #[test]
    fn procura_no_homebrew_e_no_nix() {
        let dir = tempfile::tempdir().unwrap();
        let nix = dir.path().join(".nix-profile/bin");
        std::fs::create_dir_all(&nix).unwrap();
        std::fs::write(nix.join("nowplaying-cli"), "").unwrap();
        assert!(find_nowplaying_cli(dir.path()).is_some());
    }
}
