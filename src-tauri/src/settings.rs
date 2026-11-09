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
    pub character: String,
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
            character: config.character,
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
    if enabled {
        reregister_if_moved(app);
    }
}

/// O item de login aponta para um executável. Se o app foi movido, reinstalado em outro
/// lugar ou o registro aponta para um caminho que não existe mais, registra de novo
/// com o executável atual. Só vale para o app instalado (dentro de um `.app`): o modo
/// de desenvolvimento nunca toma o lugar do app instalado.
fn reregister_if_moved(app: &AppHandle) {
    use tauri_plugin_autostart::ManagerExt;
    let Ok(current) = std::env::current_exe().and_then(|p| p.canonicalize()) else {
        return;
    };
    if !is_inside_app_bundle(&current) {
        return;
    }
    let registered = login_item_program(app);
    if !needs_reregister(registered.as_deref(), &current) {
        return;
    }
    let manager = app.autolaunch();
    let _ = manager.disable();
    match manager.enable() {
        Ok(()) => {
            tracing::info!(from = ?registered, to = %current.display(), "item de login registrado de novo")
        }
        Err(e) => tracing::warn!(%e, "não foi possível registrar o item de login de novo"),
    }
}

fn is_inside_app_bundle(exe: &std::path::Path) -> bool {
    exe.to_string_lossy().contains(".app/Contents/MacOS/")
}

/// Precisa registrar de novo se não há caminho registrado, se ele não existe mais ou se
/// é outro executável.
pub fn needs_reregister(registered: Option<&std::path::Path>, current: &std::path::Path) -> bool {
    match registered {
        None => true,
        Some(path) => !path.exists() || path.canonicalize().map(|p| p != current).unwrap_or(true),
    }
}

/// Primeiro argumento do LaunchAgent (`~/Library/LaunchAgents/<App>.plist`).
#[cfg(target_os = "macos")]
fn login_item_program(app: &AppHandle) -> Option<PathBuf> {
    let name = app
        .config()
        .product_name
        .clone()
        .unwrap_or_else(|| "Miau".into());
    let file = dirs::home_dir()?
        .join("Library/LaunchAgents")
        .join(format!("{name}.plist"));
    let value = plist::Value::from_file(file).ok()?;
    let program = value
        .as_dictionary()?
        .get("ProgramArguments")?
        .as_array()?
        .first()?
        .as_string()?;
    Some(PathBuf::from(program))
}

#[cfg(not(target_os = "macos"))]
fn login_item_program(_app: &AppHandle) -> Option<PathBuf> {
    None
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
    fn registra_de_novo_so_quando_o_caminho_mudou() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("Miau.app/Contents/MacOS/miau");
        std::fs::create_dir_all(current.parent().unwrap()).unwrap();
        std::fs::write(&current, "").unwrap();
        let current = current.canonicalize().unwrap();
        let other = dir.path().join("outro");
        std::fs::write(&other, "").unwrap();

        assert!(
            !needs_reregister(Some(&current), &current),
            "mesmo executável: nada a fazer"
        );
        assert!(needs_reregister(None, &current), "sem registro");
        assert!(
            needs_reregister(Some(&dir.path().join("sumiu")), &current),
            "caminho que não existe"
        );
        assert!(needs_reregister(Some(&other), &current), "outro executável");
        assert!(is_inside_app_bundle(&current));
        assert!(!is_inside_app_bundle(std::path::Path::new(
            "/x/target/debug/miau"
        )));
    }

    #[test]
    fn sem_caminho_funciona_so_em_memoria() {
        let settings = Settings::new(None, Config::default());
        let c = settings.update(|c| c.max_cards = 2);
        assert_eq!(c.max_cards, 2);
        assert_eq!(settings.snapshot().max_cards, 2);
    }
}
