mod accessories;
mod autostart;
mod clipboard;
mod config;
mod customize;
mod feedback;
mod github;
mod history;
mod i18n;
mod integrations;
#[cfg(target_os = "macos")]
mod macos;
mod media;
mod migrate;
mod model;
mod onboarding;
mod overlay;
mod server;
mod settings;
mod sound;
mod startup;
mod test_sequence;
mod tray;
mod update;
mod windows;

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

    #[cfg(target_os = "macos")]
    let builder = builder.activate_ignoring_other_apps(false);

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            overlay::commands::overlay_empty,
            overlay::commands::set_interactive,
            overlay::commands::set_overlay_size,
            overlay::commands::tray_alert_cleared,
            overlay::commands::card_action,
            settings::get_settings,
            settings::set_appearance,
            onboarding::onboarding_status,
            onboarding::onboarding_connect_claude,
            onboarding::onboarding_enable_github,
            onboarding::onboarding_test,
            onboarding::onboarding_finish,
            github::commands::github_device_start,
            github::commands::github_device_wait,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            if let Some(home) = dirs::home_dir() {
                migrate::migrate_dirs(&home);
            }
            app.manage(Settings::load());
            app.manage(IdGen::default());
            accessories::ensure_user_dir();
            app.manage(tray::icon::TrayAlert::default());
            app.manage(github::commands::PendingDevice::default());
            app.manage(update::Available::default());
            app.manage(history::History::default());
            let github_on = app.state::<Settings>().snapshot().github;
            github::spawn(app.handle(), github_on);
            migrate::migrate_login_item(app.handle());
            autostart::reconcile_autostart(app.handle());

            overlay::create_overlay(app.handle())?;
            let status = startup::start_server(app.handle());
            tray::create(app.handle(), status)?;
            startup::sync_scripts_on_start(app.handle());
            startup::warn_media_on_start(app.handle());
            update::spawn(app.handle());
            if !app.state::<Settings>().snapshot().onboarded {
                onboarding::open(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == onboarding::WELCOME_LABEL
                && matches!(event, tauri::WindowEvent::Destroyed)
            {
                onboarding::mark_done(window.app_handle());
            }
        })
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Quacko");
}
