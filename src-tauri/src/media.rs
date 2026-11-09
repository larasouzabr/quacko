//! Pausa de mídia: o que o computador tem para pausar o player do sistema.
//!
//! Testado no macOS 26.5.2 (Tahoe): o framework privado MediaRemote não responde a apps
//! que não são da Apple desde o macOS 15.4 (`MRMediaRemoteGetNowPlayingInfo` devolve
//! nil para um binário com assinatura ad-hoc, e o mesmo código responde quando roda
//! num binário assinado pela Apple). Por isso a pausa nativa dentro do app não é viável
//! sem o mesmo contorno que o `nowplaying-cli` usa (carregar uma biblioteca dentro de
//! um programa da Apple), que a spec v2 considera arriscado de manter.
//!
//! Decisão: a pausa continua no `notify.sh` (Spotify por AppleScript, que funciona
//! sempre; YouTube e outros players pelo `nowplaying-cli`, opcional). O app só detecta
//! se o `nowplaying-cli` existe e avisa quando ele falta.

use std::path::{Path, PathBuf};

/// Onde o `notify.sh` encontra o `nowplaying-cli` (o PATH dele inclui o Homebrew).
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

/// Avisar que falta o `nowplaying-cli`? Só se a pausa importa (algum agente conectado),
/// se ele não existe e se ainda não avisamos.
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
