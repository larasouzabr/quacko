//! Idioma efetivo e textos do menu da barra.

use serde::Serialize;

use crate::config::LangPref;

/// Idioma efetivo (o que o frontend e o menu usam).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Lang {
    #[serde(rename = "pt-BR")]
    PtBr,
    #[serde(rename = "en")]
    En,
}

/// "Automático": primeiro idioma suportado na lista de preferidos do sistema; senão inglês.
pub fn resolve<I, S>(pref: LangPref, system_locales: I) -> Lang
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    match pref {
        LangPref::PtBr => Lang::PtBr,
        LangPref::En => Lang::En,
        LangPref::Auto => system_locales
            .into_iter()
            .find_map(|l| {
                let l = l.as_ref().to_ascii_lowercase();
                if l == "pt" || l.starts_with("pt-") || l.starts_with("pt_") {
                    Some(Lang::PtBr)
                } else if l == "en" || l.starts_with("en-") || l.starts_with("en_") {
                    Some(Lang::En)
                } else {
                    None
                }
            })
            .unwrap_or(Lang::En),
    }
}

pub fn resolve_from_system(pref: LangPref) -> Lang {
    resolve(pref, sys_locale::get_locales())
}

pub struct TrayTexts {
    pub listening: &'static str,
    pub port_busy: &'static str,
    pub port_unavailable: &'static str,
    pub dnd: &'static str,
    pub test: &'static str,
    pub autostart: &'static str,
    pub language: &'static str,
    pub lang_auto: &'static str,
    pub quit: &'static str,
    pub test_title: &'static str,
    pub connect: &'static str,
    pub not_found: &'static str,
    pub prwatch_item: &'static str,
    pub connected: &'static str,
    pub disconnected: &'static str,
    pub manual_prefix: &'static str,
    pub manual_suffix: &'static str,
    pub reason_managed: &'static str,
    pub reason_readonly: &'static str,
    pub reason_invalid: &'static str,
    pub prwatch_linked: &'static str,
    pub prwatch_conflict: &'static str,
    pub error: &'static str,
    pub github_label: &'static str,
    /// Modelo com {login} e {n}.
    pub github_connected: &'static str,
    pub github_no_gh: &'static str,
    pub github_expired: &'static str,
    pub github_off: &'static str,
    pub scripts_updated: &'static str,
    pub media_missing: &'static str,
    pub welcome: &'static str,
    pub customize: &'static str,
    pub github_sign_in: &'static str,
}

pub fn tray_texts(lang: Lang) -> &'static TrayTexts {
    match lang {
        Lang::PtBr => &PT_BR,
        Lang::En => &EN,
    }
}

static PT_BR: TrayTexts = TrayTexts {
    listening: "Ouvindo na porta",
    port_busy: "ocupada",
    port_unavailable: "indisponível",
    dnd: "Não perturbar",
    test: "Testar notificação",
    autostart: "Iniciar com o login",
    language: "Idioma",
    lang_auto: "Automático (do sistema)",
    quit: "Sair",
    test_title: "Teste",
    connect: "Conectar",
    not_found: "(não encontrado)",
    prwatch_item: "Instalar comando prwatch (PRs)",
    connected: "conectado. Vale a partir da próxima sessão.",
    disconnected: "desconectado.",
    manual_prefix: "Não consegui editar",
    manual_suffix: "Copiei o trecho: cole no arquivo.",
    reason_managed: "gerenciado pelo Nix",
    reason_readonly: "somente leitura",
    reason_invalid: "formato não reconhecido",
    prwatch_linked: "Comando prwatch instalado em ~/.local/bin. Precisa do GitHub CLI (gh).",
    prwatch_conflict: "Já existe um prwatch em ~/.local/bin; não mexi nele.",
    error: "Algo deu errado:",
    github_label: "GitHub (PRs)",
    github_connected: "GitHub conectado como @{login}. PRs abertos acompanhados: {n}.",
    github_no_gh: "Não achei o GitHub CLI logado. Instale o gh e rode “gh auth login”.",
    github_expired: "O login do GitHub expirou. Rode “gh auth login” e conecte de novo.",
    github_off: "GitHub desconectado.",
    scripts_updated: "Atualizei os scripts do Miau ({files}). O backup ficou em ~/.claude/hooks.",
    welcome: "Boas-vindas…",
    customize: "Personalizar…",
    github_sign_in: "Entre no GitHub com um código: abri a tela de boas-vindas.",
    media_missing: "O Spotify já pausa sozinho. Para pausar YouTube e outros players, instale o nowplaying-cli (brew install nowplaying-cli).",
};

static EN: TrayTexts = TrayTexts {
    listening: "Listening on port",
    port_busy: "in use",
    port_unavailable: "unavailable",
    dnd: "Do Not Disturb",
    test: "Test notification",
    autostart: "Open at Login",
    language: "Language",
    lang_auto: "Automatic (system)",
    quit: "Quit",
    test_title: "Test",
    connect: "Connect",
    not_found: "(not found)",
    prwatch_item: "Install prwatch command (PRs)",
    connected: "connected. Takes effect in new sessions.",
    disconnected: "disconnected.",
    manual_prefix: "Couldn't edit",
    manual_suffix: "Snippet copied: paste it into the file.",
    reason_managed: "managed by Nix",
    reason_readonly: "read-only",
    reason_invalid: "unrecognized format",
    prwatch_linked: "prwatch command installed in ~/.local/bin. Requires the GitHub CLI (gh).",
    prwatch_conflict: "There is already a prwatch in ~/.local/bin; left it alone.",
    error: "Something went wrong:",
    github_label: "GitHub (PRs)",
    github_connected: "GitHub connected as @{login}. Watching open PRs: {n}.",
    github_no_gh: "Couldn't find a logged-in GitHub CLI. Install gh and run “gh auth login”.",
    github_expired: "Your GitHub login expired. Run “gh auth login” and connect again.",
    github_off: "GitHub disconnected.",
    scripts_updated: "Updated the Miau scripts ({files}). Backup kept in ~/.claude/hooks.",
    welcome: "Welcome…",
    customize: "Customize…",
    github_sign_in: "Sign in to GitHub with a code: I opened the welcome screen.",
    media_missing: "Spotify already pauses. To pause YouTube and other players, install nowplaying-cli (brew install nowplaying-cli).",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escolha_manual_ignora_o_sistema() {
        assert_eq!(resolve(LangPref::PtBr, ["en-US"]), Lang::PtBr);
        assert_eq!(resolve(LangPref::En, ["pt-BR"]), Lang::En);
    }

    #[test]
    fn automatico_usa_o_primeiro_idioma_suportado() {
        // O Mac da Lara: inglês (região Brasil) primeiro, português depois.
        assert_eq!(resolve(LangPref::Auto, ["en-BR", "pt-BR"]), Lang::En);
        assert_eq!(resolve(LangPref::Auto, ["pt-BR", "en-US"]), Lang::PtBr);
        assert_eq!(
            resolve(LangPref::Auto, ["fr-FR", "pt-PT", "en"]),
            Lang::PtBr
        );
        assert_eq!(resolve(LangPref::Auto, ["pt"]), Lang::PtBr);
    }

    #[test]
    fn automatico_sem_idioma_suportado_cai_no_ingles() {
        assert_eq!(resolve(LangPref::Auto, ["fr-FR", "de"]), Lang::En);
        assert_eq!(resolve(LangPref::Auto, Vec::<String>::new()), Lang::En);
    }

    #[test]
    fn nao_confunde_prefixos_parecidos() {
        // "ptx"/"eng" não são pt/en.
        assert_eq!(resolve(LangPref::Auto, ["ptx", "pt-BR"]), Lang::PtBr);
        assert_eq!(resolve(LangPref::Auto, ["eng", "pt-BR"]), Lang::PtBr);
    }

    #[test]
    fn serializa_como_o_frontend_espera() {
        assert_eq!(serde_json::to_value(Lang::PtBr).unwrap(), "pt-BR");
        assert_eq!(serde_json::to_value(Lang::En).unwrap(), "en");
    }
}
