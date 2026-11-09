use std::fs;
use std::path::{Path, PathBuf};

pub const LEGACY_DIR_NAME: &str = "miau";
pub const LEGACY_PRODUCT_NAME: &str = "Miau";
const NEW_DIR_NAME: &str = "quacko";

pub fn move_dir_once(old: &Path, new: &Path) -> bool {
    if new.exists() || !old.is_dir() {
        return false;
    }
    if let Some(parent) = new.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match fs::rename(old, new) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(%e, from = %old.display(), to = %new.display(), "não foi possível migrar a pasta");
            false
        }
    }
}

pub fn migrate_dirs(home: &Path) -> Vec<PathBuf> {
    let mut moved = Vec::new();
    for base in [home.join(".config"), home.join(".cache")] {
        let (old, new) = (base.join(LEGACY_DIR_NAME), base.join(NEW_DIR_NAME));
        if move_dir_once(&old, &new) {
            tracing::info!(to = %new.display(), "pasta migrada do nome antigo");
            moved.push(new);
        }
    }
    moved
}

pub fn legacy_login_item(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents")
        .join(format!("{LEGACY_PRODUCT_NAME}.plist"))
}

pub fn migrate_login_item(app: &tauri::AppHandle) {
    use tauri_plugin_autostart::ManagerExt;
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let old = legacy_login_item(&home);
    if !old.exists() {
        return;
    }
    match fs::remove_file(&old) {
        Ok(()) => tracing::info!("item de login antigo removido"),
        Err(e) => {
            tracing::warn!(%e, "não foi possível remover o item de login antigo");
            return;
        }
    }
    let manager = app.autolaunch();
    if !manager.is_enabled().unwrap_or(false) {
        if let Err(e) = manager.enable() {
            tracing::warn!(%e, "não foi possível recriar o item de login");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_as_pastas_antigas_uma_vez() {
        let home = tempfile::tempdir().unwrap();
        let old_config = home.path().join(".config/miau");
        fs::create_dir_all(old_config.join("accessories")).unwrap();
        fs::write(old_config.join("config.json"), r#"{"dnd":true}"#).unwrap();
        fs::create_dir_all(home.path().join(".cache/miau")).unwrap();

        let moved = migrate_dirs(home.path());
        assert_eq!(moved.len(), 2);
        assert!(!old_config.exists());
        let new_config = home.path().join(".config/quacko");
        assert_eq!(
            fs::read_to_string(new_config.join("config.json")).unwrap(),
            r#"{"dnd":true}"#
        );
        assert!(new_config.join("accessories").is_dir());
        assert!(
            migrate_dirs(home.path()).is_empty(),
            "segunda vez: nada a fazer"
        );
    }

    #[test]
    fn nunca_sobrescreve_a_pasta_nova() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join(".config/miau")).unwrap();
        fs::write(home.path().join(".config/miau/config.json"), "antigo").unwrap();
        fs::create_dir_all(home.path().join(".config/quacko")).unwrap();
        fs::write(home.path().join(".config/quacko/config.json"), "novo").unwrap();
        assert!(migrate_dirs(home.path()).is_empty());
        assert_eq!(
            fs::read_to_string(home.path().join(".config/quacko/config.json")).unwrap(),
            "novo"
        );
        assert!(
            home.path().join(".config/miau").exists(),
            "a antiga fica onde está"
        );
    }

    #[test]
    fn caminho_do_item_de_login_antigo() {
        assert!(
            legacy_login_item(Path::new("/Users/x")).ends_with("Library/LaunchAgents/Miau.plist")
        );
    }
}
