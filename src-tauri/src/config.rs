use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::server::DEFAULT_PORT;

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
    pub sound: bool,
    pub pause_media: bool,
    pub notify_claude: bool,
    pub notify_cursor: bool,
    pub notify_ci: bool,
    pub notify_terminal: bool,
    pub autostart: bool,
    pub max_cards: u8,
    pub pixel_size: u8,
    pub github: bool,
    pub media_warned: bool,
    pub onboarded: bool,
    pub github_client_id: Option<String>,
    pub character: String,
    pub accessory_enabled: bool,
    pub accessory_id: String,
    pub accessory_color: String,
    pub check_updates: bool,
    pub update_announced: Option<String>,
}

pub const DEFAULT_ACCESSORY: &str = "laco";
pub const DEFAULT_ACCESSORY_COLOR: &str = "#ff6996";

pub fn valid_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

pub const DEFAULT_CHARACTER: &str = "pato-pixel";

fn valid_character_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
}

pub const PIXEL_SIZES: [u8; 5] = [3, 4, 5, 6, 8];
pub const DEFAULT_PIXEL_SIZE: u8 = 3;

impl Default for Config {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            lang: LangPref::Auto,
            dnd: false,
            sound: true,
            pause_media: true,
            notify_claude: true,
            notify_cursor: true,
            notify_ci: true,
            notify_terminal: true,
            autostart: false,
            max_cards: MAX_CARDS_LIMIT,
            pixel_size: DEFAULT_PIXEL_SIZE,
            github: false,
            media_warned: false,
            onboarded: false,
            github_client_id: None,
            character: DEFAULT_CHARACTER.to_string(),
            accessory_enabled: false,
            accessory_id: DEFAULT_ACCESSORY.to_string(),
            accessory_color: DEFAULT_ACCESSORY_COLOR.to_string(),
            check_updates: true,
            update_announced: None,
        }
    }
}

impl Config {
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
        if let Some(v) = map.get("sound") {
            match v.as_bool() {
                Some(b) => config.sound = b,
                None => invalid.push("sound"),
            }
        }
        if let Some(v) = map.get("pause_media") {
            match v.as_bool() {
                Some(b) => config.pause_media = b,
                None => invalid.push("pause_media"),
            }
        }
        for (key, field) in [
            ("notify_claude", &mut config.notify_claude),
            ("notify_cursor", &mut config.notify_cursor),
            ("notify_ci", &mut config.notify_ci),
            ("notify_terminal", &mut config.notify_terminal),
        ] {
            if let Some(v) = map.get(key) {
                match v.as_bool() {
                    Some(b) => *field = b,
                    None => invalid.push(key),
                }
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
        if let Some(v) = map.get("accessory_enabled") {
            match v.as_bool() {
                Some(b) => config.accessory_enabled = b,
                None => invalid.push("accessory_enabled"),
            }
        }
        if let Some(v) = map.get("accessory_id") {
            match v
                .as_str()
                .map(str::trim)
                .filter(|id| valid_character_id(id))
            {
                Some(id) => config.accessory_id = id.to_string(),
                None => invalid.push("accessory_id"),
            }
        }
        if let Some(v) = map.get("accessory_color") {
            match v.as_str().map(str::trim).filter(|c| valid_hex_color(c)) {
                Some(c) => config.accessory_color = c.to_ascii_lowercase(),
                None => invalid.push("accessory_color"),
            }
        }
        if let Some(v) = map.get("character") {
            match v
                .as_str()
                .map(str::trim)
                .filter(|id| valid_character_id(id))
            {
                Some(id) => config.character = id.to_string(),
                None => invalid.push("character"),
            }
        }
        if let Some(v) = map.get("github_client_id") {
            match v {
                Value::Null => {}
                Value::String(s) if crate::github::auth::valid_client_id(s.trim()) => {
                    config.github_client_id = Some(s.trim().to_string())
                }
                _ => invalid.push("github_client_id"),
            }
        }
        if let Some(v) = map.get("check_updates") {
            match v.as_bool() {
                Some(b) => config.check_updates = b,
                None => invalid.push("check_updates"),
            }
        }
        if let Some(v) = map.get("update_announced") {
            match v {
                Value::Null => {}
                Value::String(s) if crate::update::is_version(s) => {
                    config.update_announced = Some(s.clone())
                }
                _ => invalid.push("update_announced"),
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
    dirs::home_dir().map(|home| home.join(".config").join("quacko").join("config.json"))
}

pub fn load_or_create(path: &Path) -> Config {
    match fs::read_to_string(path) {
        Ok(text) => {
            let (config, invalid) = Config::from_json_lenient(&text);
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
            serde_json::json!({"port": 4747, "lang": "auto", "dnd": false, "sound": true, "pause_media": true, "notify_claude": true, "notify_cursor": true, "notify_ci": true, "notify_terminal": true, "autostart": false, "max_cards": 3, "pixel_size": 3, "github": false, "media_warned": false, "onboarded": false, "github_client_id": null, "character": "pato-pixel", "accessory_enabled": false, "accessory_id": "laco", "accessory_color": "#ff6996", "check_updates": true, "update_announced": null})
        );
    }

    #[test]
    fn le_config_completa() {
        let c = parse(
            r#"{"port":5000,"lang":"en","dnd":true,"sound":false,"pause_media":false,"notify_cursor":false,"notify_terminal":false,"autostart":true,"max_cards":2,"pixel_size":5}"#,
        );
        assert_eq!(
            c,
            Config {
                port: 5000,
                lang: LangPref::En,
                dnd: true,
                sound: false,
                pause_media: false,
                notify_cursor: false,
                notify_terminal: false,
                autostart: true,
                max_cards: 2,
                pixel_size: 5,
                ..Config::default()
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
    fn personagem_padrao_e_escolha_explicita() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        assert_eq!(load_or_create(&path).character, DEFAULT_CHARACTER);
        fs::write(&path, r#"{"port":4747,"dnd":true}"#).unwrap();
        let c = load_or_create(&path);
        assert_eq!((c.character.as_str(), c.dnd), (DEFAULT_CHARACTER, true));
        fs::write(&path, r#"{"character":"gato-pixel"}"#).unwrap();
        assert_eq!(load_or_create(&path).character, "gato-pixel");
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
    fn acessorio_desligado_por_padrao_e_cor_validada() {
        let c = Config::default();
        assert!(!c.accessory_enabled);
        assert_eq!(
            (c.accessory_id.as_str(), c.accessory_color.as_str()),
            ("laco", "#ff6996")
        );
        let c = parse(r##"{"accessory_enabled":true,"accessory_color":"#4A90E2"}"##);
        assert!(c.accessory_enabled);
        assert_eq!(c.accessory_color, "#4a90e2");
        for bad in ["\"azul\"", "\"#fff\"", "\"#gggggg\"", "\"url(x)\"", "1"] {
            let (c, invalid) =
                Config::from_json_lenient(&format!(r#"{{"accessory_color":{bad}}}"#));
            assert_eq!(c.accessory_color, DEFAULT_ACCESSORY_COLOR, "{bad}");
            assert_eq!(invalid, vec!["accessory_color"], "{bad}");
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
            r#"{"port":"abc","lang":"fr","dnd":"sim","sound":"não","pause_media":0,"notify_ci":"x","autostart":1,"max_cards":9,"pixel_size":7,"extra":true}"#,
        );
        assert_eq!(c, Config::default());
        assert_eq!(
            invalid,
            vec![
                "port",
                "lang",
                "dnd",
                "sound",
                "pause_media",
                "notify_ci",
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
