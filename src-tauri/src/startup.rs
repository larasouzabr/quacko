use crate::feedback::tell;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

use crate::i18n::{self};
use crate::integrations::{self, Paths, Tool};
use crate::model::{IdGen, Status};
use crate::settings::Settings;

pub fn sync_scripts_on_start(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(paths) = Paths::from_home() else {
            return;
        };
        let updated = integrations::sync_installed_scripts(&paths);
        if updated.is_empty() {
            return;
        }
        tracing::info!(?updated, "scripts instalados atualizados");
        let settings = app.state::<Settings>();
        if settings.snapshot().dnd {
            return;
        }
        let t = i18n::tray_texts(settings.lang());
        tell(
            &app,
            Status::Info,
            t.scripts_updated.replace("{files}", &updated.join(", ")),
        );
    });
}

pub fn warn_media_on_start(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(paths) = Paths::from_home() else {
            return;
        };
        let settings = app.state::<Settings>();
        let config = settings.snapshot();
        let connected = integrations::status(&paths, Tool::Claude)
            || integrations::status(&paths, Tool::Cursor);
        let found = crate::media::find_nowplaying_cli(&paths.home).is_some();
        if !crate::media::should_warn(connected, found, config.media_warned) || config.dnd {
            return;
        }
        settings.update(|c| c.media_warned = true);
        let t = i18n::tray_texts(settings.lang());
        tell(&app, Status::Info, t.media_missing.to_string());
    });
}

pub fn start_server(app: &tauri::AppHandle) -> crate::tray::ServerStatus {
    let settings = app.state::<Settings>();
    let port = crate::server::resolve_port(
        std::env::var("QUACKO_PORT").ok().as_deref(),
        settings.snapshot().port,
    );

    let handle = app.clone();
    let deliver: crate::server::Deliver =
        Arc::new(move |notification| crate::overlay::present(&handle, notification));
    let state = crate::server::ServerState::new(
        settings.dnd_flag(),
        app.state::<IdGen>().inner().clone(),
        deliver,
    );

    match crate::server::bind(port) {
        Ok(listener) => {
            tracing::info!(port, "ouvindo em 127.0.0.1");
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::server::serve(listener, state).await {
                    tracing::error!(%e, "servidor HTTP parou");
                }
            });
            crate::tray::ServerStatus::Listening { port }
        }
        Err(e) => {
            tracing::error!(%e, port, "não foi possível abrir a porta");
            if e.kind() == std::io::ErrorKind::AddrInUse {
                crate::tray::ServerStatus::PortBusy { port }
            } else {
                crate::tray::ServerStatus::Unavailable { port }
            }
        }
    }
}
