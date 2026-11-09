mod config;
mod github;
mod i18n;
mod integrations;
#[cfg(target_os = "macos")]
mod macos;
mod model;
mod server;
mod settings;
mod tray;
mod window;

use std::sync::Arc;

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

use model::IdGen;
use settings::Settings;

pub fn run() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    let builder = tauri::Builder::default();

    // Não ativar o app ao iniciar: o Miau não pode roubar o foco do editor.
    #[cfg(target_os = "macos")]
    let builder = builder.activate_ignoring_other_apps(false);

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            window::overlay_empty,
            window::set_interactive,
            window::set_overlay_size,
            window::card_action,
            settings::get_settings,
        ])
        .setup(|app| {
            // Só barra de menus, sem ícone no Dock.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.manage(Settings::load());
            app.manage(IdGen::default());
            let github_on = app.state::<Settings>().snapshot().github;
            github::spawn(app.handle(), github_on);
            settings::reconcile_autostart(app.handle());

            window::create_overlay(app.handle())?;
            let status = start_server(app.handle());
            tray::create(app.handle(), status)?;
            tray::sync_scripts_on_start(app.handle());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Miau");
}

fn start_server(app: &tauri::AppHandle) -> tray::ServerStatus {
    let settings = app.state::<Settings>();
    let port = server::resolve_port(
        std::env::var("MIAU_PORT").ok().as_deref(),
        settings.snapshot().port,
    );

    let handle = app.clone();
    let deliver: server::Deliver =
        Arc::new(move |notification| window::present(&handle, notification));
    let state = server::ServerState::new(
        settings.dnd_flag(),
        app.state::<IdGen>().inner().clone(),
        deliver,
    );

    match server::bind(port) {
        Ok(listener) => {
            tracing::info!(port, "ouvindo em 127.0.0.1");
            tauri::async_runtime::spawn(async move {
                if let Err(e) = server::serve(listener, state).await {
                    tracing::error!(%e, "servidor HTTP parou");
                }
            });
            tray::ServerStatus::Listening { port }
        }
        Err(e) => {
            tracing::error!(%e, port, "não foi possível abrir a porta");
            if e.kind() == std::io::ErrorKind::AddrInUse {
                tray::ServerStatus::PortBusy { port }
            } else {
                tray::ServerStatus::Unavailable { port }
            }
        }
    }
}
