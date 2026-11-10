use crate::feedback::tell;
use tauri::{AppHandle, Manager};

use crate::i18n::{self};
use crate::integrations::{self, ManualReason, Outcome, Paths, PrwatchOutcome, ShellOutcome, Tool};
use crate::model::Status;
use crate::settings::Settings;

pub fn toggle_integration(app: &AppHandle, tool: Tool) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(paths) = Paths::from_home() else {
            return;
        };
        let t = i18n::tray_texts(app.state::<Settings>().lang());
        let name = match tool {
            Tool::Claude => "Claude Code",
            Tool::Cursor => "Cursor",
        };
        let connect = !integrations::status(&paths, tool);
        let (status, message) = match integrations::set_connected(&paths, tool, connect) {
            Ok(Outcome::Connected) => (Status::Ok, format!("{name} {}", t.connected)),
            Ok(Outcome::Disconnected) => (Status::Info, format!("{name} {}", t.disconnected)),
            Ok(Outcome::Manual {
                reason,
                file,
                snippet,
            }) => {
                crate::clipboard::copy_to_clipboard(&snippet);
                let reason = match reason {
                    ManualReason::Managed => t.reason_managed,
                    ManualReason::ReadOnly => t.reason_readonly,
                    ManualReason::Invalid => t.reason_invalid,
                };
                (
                    Status::Attention,
                    format!(
                        "{} {} ({reason}). {}",
                        t.manual_prefix,
                        tilde(&paths, &file),
                        t.manual_suffix
                    ),
                )
            }
            Err(e) => (Status::Fail, format!("{} {e}", t.error)),
        };
        tracing::info!(?tool, connect, ?status, "conectar ferramenta");
        tell(&app, status, message);
        crate::tray::refresh(&app);
    });
}

pub fn install_prwatch(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(paths) = Paths::from_home() else {
            return;
        };
        let t = i18n::tray_texts(app.state::<Settings>().lang());
        let (status, message) = match integrations::install_prwatch(&paths) {
            Ok(PrwatchOutcome::Linked) => (Status::Ok, t.prwatch_linked.to_string()),
            Ok(PrwatchOutcome::Conflict) => (Status::Attention, t.prwatch_conflict.to_string()),
            Err(e) => (Status::Fail, format!("{} {e}", t.error)),
        };
        tell(&app, status, message);
    });
}

pub fn install_shell(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(paths) = Paths::from_home() else {
            return;
        };
        let t = i18n::tray_texts(app.state::<Settings>().lang());
        let (status, message) = match integrations::install_shell(&paths) {
            Ok(ShellOutcome::Copied { line, nix }) => {
                crate::clipboard::copy_to_clipboard(&line);
                let text = if nix {
                    t.shell_copied_nix
                } else {
                    t.shell_copied
                };
                (Status::Attention, text.to_string())
            }
            Ok(ShellOutcome::AlreadySourced) => (Status::Ok, t.shell_ready.to_string()),
            Err(e) => (Status::Fail, format!("{} {e}", t.error)),
        };
        tell(&app, status, message);
    });
}

fn tilde(paths: &Paths, file: &std::path::Path) -> String {
    match file.strip_prefix(&paths.home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => file.display().to_string(),
    }
}
