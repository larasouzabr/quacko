//! Config em memória, compartilhada entre menu, servidor e frontend.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::config::{self, Config};
use crate::i18n::{self, Lang};
use crate::window::OVERLAY_LABEL;

pub struct Settings {
    path: Option<PathBuf>,
    config: Mutex<Config>,
    /// Espelho de `config.dnd` lido pelo servidor a cada aviso, sem trava.
    dnd: Arc<AtomicBool>,
}

/// O que o frontend precisa saber.
#[derive(Debug, Clone, Serialize)]
pub struct FrontendSettings {
    pub lang: Lang,
    pub max_cards: u8,
    pub pixel_size: u8,
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
        }
    }

    /// Aplica a mudança, sincroniza o "Não perturbe" e grava no arquivo.
    /// Se gravar falhar, a mudança vale até o app fechar.
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

/// Avisa o overlay que idioma ou limite de cartões mudaram.
pub fn broadcast(app: &AppHandle) {
    let settings = app.state::<Settings>().frontend();
    if let Err(e) = app.emit_to(OVERLAY_LABEL, "settings-changed", settings) {
        tracing::warn!(%e, "falha ao avisar o overlay da nova config");
    }
}

/// O sistema é a fonte da verdade do "Iniciar com o login": se a pessoa tirou o Miau
/// dos itens de login nos Ajustes do Sistema, a config passa a refletir isso.
pub fn reconcile_autostart(app: &AppHandle) {
    use tauri_plugin_autostart::ManagerExt;
    let Ok(enabled) = app.autolaunch().is_enabled() else {
        return;
    };
    let settings = app.state::<Settings>();
    if settings.snapshot().autostart != enabled {
        settings.update(|c| c.autostart = enabled);
    }
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
