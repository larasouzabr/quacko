use crate::settings::Settings;
use std::path::PathBuf;

use tauri::{AppHandle, Manager};

pub fn toggle_autostart(app: &AppHandle) {
    use tauri_plugin_autostart::ManagerExt;
    let settings = app.state::<Settings>();
    let want = !settings.snapshot().autostart;
    let result = if want {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    };
    match result {
        Ok(()) => {
            settings.update(|c| c.autostart = want);
            tracing::info!(autostart = want, "iniciar com o login alterado");
        }
        Err(e) => tracing::warn!(%e, "não foi possível alterar o iniciar com o login"),
    }
}

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

pub fn needs_reregister(registered: Option<&std::path::Path>, current: &std::path::Path) -> bool {
    match registered {
        None => true,
        Some(path) => !path.exists() || path.canonicalize().map(|p| p != current).unwrap_or(true),
    }
}

#[cfg(target_os = "macos")]
fn login_item_program(app: &AppHandle) -> Option<PathBuf> {
    let name = app
        .config()
        .product_name
        .clone()
        .unwrap_or_else(|| "Quacko".into());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registra_de_novo_so_quando_o_caminho_mudou() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("Quacko.app/Contents/MacOS/quacko");
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
            "/x/target/debug/quacko"
        )));
    }
}
