use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::config::{self, Config};
use crate::i18n::{self, Lang};
use crate::overlay::OVERLAY_LABEL;

pub struct Settings {
    path: Option<PathBuf>,
    config: Mutex<Config>,
    dnd: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FrontendSettings {
    pub lang: Lang,
    pub max_cards: u8,
    pub pixel_size: u8,
    pub text_scale: u16,
    pub character: String,
    pub accessory_enabled: bool,
    pub accessory_id: String,
    pub accessory_color: String,
}

impl Settings {
    pub fn load() -> Self {
        let path = config::default_path();
        let config = match &path {
            Some(p) => config::load_or_create(p),
            None => {
                tracing::warn!("pasta pessoal não encontrada; config só em memória");
                Config::default()
            }
        };
        tracing::info!(path = ?path, "config carregada");
        Self::new(path, config)
    }

    pub fn new(path: Option<PathBuf>, config: Config) -> Self {
        let dnd = Arc::new(AtomicBool::new(config.dnd));
        Self {
            path,
            config: Mutex::new(config),
            dnd,
        }
    }

    pub fn snapshot(&self) -> Config {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn dnd_flag(&self) -> Arc<AtomicBool> {
        self.dnd.clone()
    }

    pub fn lang(&self) -> Lang {
        i18n::resolve_from_system(self.snapshot().lang)
    }

    pub fn frontend(&self) -> FrontendSettings {
        let config = self.snapshot();
        FrontendSettings {
            lang: i18n::resolve_from_system(config.lang),
            max_cards: config.max_cards,
            pixel_size: config.pixel_size,
            text_scale: config.text_scale,
            character: config.character,
            accessory_enabled: config.accessory_enabled,
            accessory_id: config.accessory_id,
            accessory_color: config.accessory_color,
        }
    }

    pub fn update(&self, change: impl FnOnce(&mut Config)) -> Config {
        let mut guard = self.config.lock().unwrap_or_else(|e| e.into_inner());
        change(&mut guard);
        self.dnd.store(guard.dnd, Ordering::Relaxed);
        if let Some(path) = &self.path {
            if let Err(e) = config::save(path, &guard) {
                tracing::warn!(%e, path = %path.display(), "não foi possível gravar a config");
            }
        }
        guard.clone()
    }
}

pub fn broadcast(app: &AppHandle) {
    let settings = app.state::<Settings>().frontend();
    if let Err(e) = app.emit_to(OVERLAY_LABEL, "settings-changed", settings) {
        tracing::warn!(%e, "falha ao avisar o overlay da nova config");
    }
}

#[tauri::command]
pub fn set_appearance(
    app: AppHandle,
    character: String,
    accessory_enabled: bool,
    accessory_color: String,
) -> Result<FrontendSettings, String> {
    let valid_id = |id: &str| {
        (1..=40).contains(&id.len())
            && id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };
    if !valid_id(&character) {
        return Err("personagem inválido".into());
    }
    if !config::valid_hex_color(&accessory_color) {
        return Err("cor inválida (use #rrggbb)".into());
    }
    let settings = app.state::<Settings>();
    settings.update(|c| {
        c.character = character;
        c.accessory_enabled = accessory_enabled;
        c.accessory_color = accessory_color.to_ascii_lowercase();
    });
    broadcast(&app);
    Ok(settings.frontend())
}

#[tauri::command]
pub fn get_settings(settings: State<'_, Settings>) -> FrontendSettings {
    settings.frontend()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_grava_e_sincroniza_o_nao_perturbe() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let settings = Settings::new(Some(path.clone()), Config::default());
        let dnd = settings.dnd_flag();
        assert!(!dnd.load(Ordering::Relaxed));

        settings.update(|c| c.dnd = true);
        assert!(dnd.load(Ordering::Relaxed));
        assert!(config::load_or_create(&path).dnd);

        settings.update(|c| c.dnd = false);
        assert!(!dnd.load(Ordering::Relaxed));
        assert!(!config::load_or_create(&path).dnd);
    }

    #[test]
    fn sem_caminho_funciona_so_em_memoria() {
        let settings = Settings::new(None, Config::default());
        let c = settings.update(|c| c.max_cards = 2);
        assert_eq!(c.max_cards, 2);
        assert_eq!(settings.snapshot().max_cards, 2);
    }
}
