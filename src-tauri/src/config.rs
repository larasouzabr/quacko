//! Config em `~/.config/miau/config.json` (seção 10 da spec).
//!
//! Leitura tolerante: cada campo é validado sozinho; um valor inválido cai no padrão
//! sem descartar os outros. Arquivo ausente é criado com os padrões. JSON quebrado não
//! trava o app: vale o padrão, e o arquivo é regravado na próxima mudança pelo menu.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::server::DEFAULT_PORT;

/// A janela comporta até 3 cartões; acima disso eles seriam cortados.
pub const MAX_CARDS_LIMIT: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LangPref {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "pt-BR")]
    PtBr,
    #[serde(rename = "en")]
    En,
}

impl LangPref {
    fn parse(value: &str) -> Option<Self> {
        let v = value.trim().to_ascii_lowercase();
        match v.as_str() {
            "auto" => Some(Self::Auto),
            "pt" | "pt-br" | "pt_br" => Some(Self::PtBr),
            "en" => Some(Self::En),
            _ if v.starts_with("en-") || v.starts_with("en_") => Some(Self::En),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Config {
    pub port: u16,
    pub lang: LangPref,
    pub dnd: bool,
    pub autostart: bool,
    pub max_cards: u8,
    /// Personagem pixel art: px por pixel de desenho (miau-spec-personalizacao.md, seção 4).
    /// Fica no arquivo até a janela de Configurações existir.
    pub pixel_size: u8,
    /// Monitor de PRs do GitHub ligado (menu Conectar → GitHub).
    pub github: bool,
    /// Já avisou que falta o nowplaying-cli (para não repetir a cada abertura).
    pub media_warned: bool,
    /// Já viu a tela de boas-vindas (ela só abre sozinha na primeira vez).
    pub onboarded: bool,
    /// Client ID do GitHub App para o login por código (público). Também por
    /// MIAU_GITHUB_CLIENT_ID. Sem ele, o login por código fica desligado.
    pub github_client_id: Option<String>,
    /// Personagem escolhido (id do pacote).
    pub character: String,
}

/// Personagem de instalações novas.
pub const DEFAULT_CHARACTER: &str = "pato-pixel";
/// Quem já usava o app antes de existir a escolha continua com o gato que via.
pub const LEGACY_CHARACTER: &str = "gato-pixel";

fn valid_character_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
}

/// Tamanhos de pixel oferecidos: sempre inteiros, para o pixel art não deformar.
pub const PIXEL_SIZES: [u8; 5] = [3, 4, 5, 6, 8];
pub const DEFAULT_PIXEL_SIZE: u8 = 3;

impl Default for Config {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            lang: LangPref::Auto,
            dnd: false,
            autostart: false,
            max_cards: MAX_CARDS_LIMIT,
            pixel_size: DEFAULT_PIXEL_SIZE,
            github: false,
            media_warned: false,
            onboarded: false,
            github_client_id: None,
            character: DEFAULT_CHARACTER.to_string(),
        }
    }
}

impl Config {
    /// Interpreta o JSON campo a campo. Devolve também os nomes dos campos ignorados.
    pub fn from_json_lenient(text: &str) -> (Self, Vec<&'static str>) {
        let mut config = Self::default();
        let mut invalid = Vec::new();
        let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) else {
            return (config, vec!["(arquivo inteiro)"]);
        };

        if let Some(v) = map.get("port") {
            match v
                .as_u64()
                .and_then(|p| u16::try_from(p).ok())
                .filter(|p| *p != 0)
            {
                Some(p) => config.port = p,
                None => invalid.push("port"),
            }
        }
        if let Some(v) = map.get("lang") {
            match v.as_str().and_then(LangPref::parse) {
                Some(l) => config.lang = l,
                None => invalid.push("lang"),
            }
        }
        if let Some(v) = map.get("dnd") {
            match v.as_bool() {
                Some(b) => config.dnd = b,
                None => invalid.push("dnd"),
            }
        }
        if let Some(v) = map.get("autostart") {
            match v.as_bool() {
                Some(b) => config.autostart = b,
                None => invalid.push("autostart"),
            }
        }
        if let Some(v) = map.get("max_cards") {
            match v
                .as_u64()
                .filter(|n| (1..=MAX_CARDS_LIMIT as u64).contains(n))
            {
                Some(n) => config.max_cards = n as u8,
                None => invalid.push("max_cards"),
            }
        }
        if let Some(v) = map.get("pixel_size") {
            match v
                .as_u64()
                .filter(|n| PIXEL_SIZES.iter().any(|s| u64::from(*s) == *n))
            {
                Some(n) => config.pixel_size = n as u8,
                None => invalid.push("pixel_size"),
            }
        }
        if let Some(v) = map.get("character") {
            match v
                .as_str()
                .map(str::trim)
                .filter(|id| valid_character_id(id))
            {
                // Id desconhecido do app (ex.: pacote que ainda não existe) fica: a escolha
                // é da pessoa; a tela cai no padrão enquanto ele não existir.
                Some(id) => config.character = id.to_string(),
                None => invalid.push("character"),
            }
        }
        if let Some(v) = map.get("github_client_id") {
            match v {
                Value::Null => {}
                Value::String(s) if crate::github_auth::valid_client_id(s.trim()) => {
                    config.github_client_id = Some(s.trim().to_string())
                }
                _ => invalid.push("github_client_id"),
            }
        }
        if let Some(v) = map.get("onboarded") {
            match v.as_bool() {
                Some(b) => config.onboarded = b,
                None => invalid.push("onboarded"),
            }
        }
        if let Some(v) = map.get("media_warned") {
            match v.as_bool() {
                Some(b) => config.media_warned = b,
                None => invalid.push("media_warned"),
            }
        }
        if let Some(v) = map.get("github") {
            match v.as_bool() {
                Some(b) => config.github = b,
                None => invalid.push("github"),
            }
        }
        (config, invalid)
    }
}

pub fn default_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".config").join("miau").join("config.json"))
}

/// Lê a config; se o arquivo não existe, cria com os padrões.
pub fn load_or_create(path: &Path) -> Config {
    match fs::read_to_string(path) {
        Ok(text) => {
            let (mut config, invalid) = Config::from_json_lenient(&text);
            // Config de antes da escolha de personagem: mantém o gato que a pessoa via.
            // Só com JSON válido sem o campo; arquivo quebrado não é regravado na leitura.
            if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&text) {
                if !map.contains_key("character") {
                    config.character = LEGACY_CHARACTER.to_string();
                    if let Err(e) = save(path, &config) {
                        tracing::warn!(%e, "não foi possível gravar o personagem na config");
                    }
                }
            }
            if !invalid.is_empty() {
                tracing::warn!(?invalid, path = %path.display(), "config com valores inválidos; usando o padrão nesses campos");
            }
            config
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let config = Config::default();
            if let Err(e) = save(path, &config) {
                tracing::warn!(%e, path = %path.display(), "não foi possível criar a config");
            }
            config
        }
        Err(e) => {
            tracing::warn!(%e, path = %path.display(), "não foi possível ler a config; usando o padrão");
            Config::default()
        }
    }
}

/// Grava de forma atômica: escreve num arquivo temporário e renomeia.
pub fn save(path: &Path, config: &Config) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "caminho sem pasta"))?;
    fs::create_dir_all(dir)?;
    let mut json = serde_json::to_string_pretty(config).map_err(io::Error::other)?;
    json.push('\n');
    let tmp = dir.join(format!(".config.json.{}.tmp", std::process::id()));
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Config {
        Config::from_json_lenient(text).0
    }

    #[test]
    fn padroes_da_spec() {
        let c = Config::default();
        assert_eq!(
            serde_json::to_value(&c).unwrap(),
            serde_json::json!({"port": 4747, "lang": "auto", "dnd": false, "autostart": false, "max_cards": 3, "pixel_size": 3, "github": false, "media_warned": false, "onboarded": false, "github_client_id": null, "character": "pato-pixel"})
        );
    }

    #[test]
    fn le_config_completa() {
        let c = parse(
            r#"{"port":5000,"lang":"en","dnd":true,"autostart":true,"max_cards":2,"pixel_size":5}"#,
        );
        assert_eq!(
            c,
            Config {
                port: 5000,
                lang: LangPref::En,
                dnd: true,
                autostart: true,
                max_cards: 2,
                pixel_size: 5,
                github: false,
                media_warned: false,
                onboarded: false,
                github_client_id: None,
                character: DEFAULT_CHARACTER.to_string(),
            }
        );
    }

    #[test]
    fn tamanho_do_pixel_so_aceita_os_inteiros_oferecidos() {
        for ok in PIXEL_SIZES {
            assert_eq!(parse(&format!(r#"{{"pixel_size":{ok}}}"#)).pixel_size, ok);
        }
        for bad in ["2", "7", "9", "3.5", "\"4\"", "-3"] {
            assert_eq!(
                parse(&format!(r#"{{"pixel_size":{bad}}}"#)).pixel_size,
                DEFAULT_PIXEL_SIZE,
                "{bad}"
            );
        }
    }

    #[test]
    fn client_id_do_github_app() {
        assert_eq!(
            parse(r#"{"github_client_id":"Iv23liAbc"}"#)
                .github_client_id
                .as_deref(),
            Some("Iv23liAbc")
        );
        assert_eq!(parse(r#"{"github_client_id":null}"#).github_client_id, None);
        let (c, invalid) =
            Config::from_json_lenient(r#"{"github_client_id":"tem espaço","dnd":true}"#);
        assert_eq!((c.github_client_id, c.dnd), (None, true));
        assert_eq!(invalid, vec!["github_client_id"]);
    }

    #[test]
    fn personagem_novo_e_legado() {
        // Instalação nova: arquivo criado com o pato.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        assert_eq!(load_or_create(&path).character, DEFAULT_CHARACTER);
        // Config antiga (sem o campo): mantém o gato, e grava a escolha.
        fs::write(&path, r#"{"port":4747,"dnd":true}"#).unwrap();
        let c = load_or_create(&path);
        assert_eq!((c.character.as_str(), c.dnd), (LEGACY_CHARACTER, true));
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains(LEGACY_CHARACTER));
        // Quem escolheu outro mantém a escolha (mesmo um que o app ainda não tem).
        fs::write(&path, r#"{"character":"capivara-pixel"}"#).unwrap();
        assert_eq!(load_or_create(&path).character, "capivara-pixel");
    }

    #[test]
    fn personagem_com_id_invalido_cai_no_padrao() {
        for bad in ["\"\"", "\"../x\"", "\"Pato\"", "1", "\"-x\""] {
            let (c, invalid) = Config::from_json_lenient(&format!(r#"{{"character":{bad}}}"#));
            assert_eq!(c.character, DEFAULT_CHARACTER, "{bad}");
            assert_eq!(invalid, vec!["character"], "{bad}");
        }
    }

    #[test]
    fn campos_ausentes_usam_padrao() {
        assert_eq!(parse("{}"), Config::default());
        assert_eq!(
            parse(r#"{"dnd":true}"#),
            Config {
                dnd: true,
                ..Config::default()
            }
        );
    }

    #[test]
    fn valor_invalido_cai_no_padrao_sem_perder_os_outros() {
        let (c, invalid) = Config::from_json_lenient(
            r#"{"port":"abc","lang":"fr","dnd":"sim","autostart":1,"max_cards":9,"pixel_size":7,"extra":true}"#,
        );
        assert_eq!(c, Config::default());
        assert_eq!(
            invalid,
            vec![
                "port",
                "lang",
                "dnd",
                "autostart",
                "max_cards",
                "pixel_size"
            ]
        );

        let c = parse(r#"{"port":0,"dnd":true}"#);
        assert_eq!((c.port, c.dnd), (DEFAULT_PORT, true));
    }

    #[test]
    fn portas_e_cartoes_fora_da_faixa() {
        assert_eq!(parse(r#"{"port":70000}"#).port, DEFAULT_PORT);
        assert_eq!(parse(r#"{"port":-1}"#).port, DEFAULT_PORT);
        assert_eq!(parse(r#"{"max_cards":0}"#).max_cards, MAX_CARDS_LIMIT);
        assert_eq!(parse(r#"{"max_cards":1}"#).max_cards, 1);
        assert_eq!(parse(r#"{"max_cards":2.5}"#).max_cards, MAX_CARDS_LIMIT);
    }

    #[test]
    fn variantes_de_idioma_aceitas() {
        for (txt, lang) in [
            ("auto", LangPref::Auto),
            ("pt-BR", LangPref::PtBr),
            ("pt", LangPref::PtBr),
            ("PT_br", LangPref::PtBr),
            ("en", LangPref::En),
            ("en-US", LangPref::En),
        ] {
            assert_eq!(parse(&format!(r#"{{"lang":"{txt}"}}"#)).lang, lang, "{txt}");
        }
    }

    #[test]
    fn json_quebrado_ou_nao_objeto_nao_trava() {
        for text in ["", "{", "[]", "null", "\"oi\"", "// comentário\n{}"] {
            assert_eq!(parse(text), Config::default(), "{text:?}");
        }
    }

    #[test]
    fn arquivo_ausente_e_criado_com_padroes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("config.json");
        assert_eq!(load_or_create(&path), Config::default());
        let saved = fs::read_to_string(&path).unwrap();
        assert_eq!(parse(&saved), Config::default());
    }

    #[test]
    fn salva_e_le_de_volta_sem_deixar_temporario() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let c = Config {
            dnd: true,
            lang: LangPref::PtBr,
            ..Config::default()
        };
        save(&path, &c).unwrap();
        assert_eq!(load_or_create(&path), c);
        let files: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(files, vec![std::ffi::OsString::from("config.json")]);
    }

    #[test]
    fn arquivo_invalido_nao_e_sobrescrito_na_leitura() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "{ quebrado").unwrap();
        assert_eq!(load_or_create(&path), Config::default());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ quebrado");
    }
}
