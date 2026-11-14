use crate::feedback::tell;
use tauri::{AppHandle, Manager};

use crate::i18n::{self};
use crate::integrations::{
    self, ManualReason, Outcome, Paths, PrwatchOutcome, Shell, ShellOutcome, Tool,
};
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
        let shell = Shell::detect(cfg!(windows), std::env::var("SHELL").ok().as_deref());
        let exe = integrations::hook_exe().filter(|p| p.is_file());
        let (status, message) = match integrations::install_shell(&paths, shell, exe.as_deref()) {
            Ok(_) if shell == Shell::Pwsh => connect_powershell(&app, t),
            Ok(ShellOutcome::Copied { line, nix }) => {
                crate::clipboard::copy_to_clipboard(&line);
                let text = if nix {
                    t.shell_copied_nix
                } else {
                    t.shell_copied
                };
                (Status::Attention, text.replace("{file}", shell.rc_name()))
            }
            Ok(ShellOutcome::AlreadySourced) => {
                (Status::Ok, t.shell_ready.replace("{file}", shell.rc_name()))
            }
            Err(e) => (Status::Fail, format!("{} {e}", t.error)),
        };
        tell(&app, status, message);
    });
}

/// Windows: libera scripts locais (com consentimento) e põe a linha do Quacko no perfil de cada PowerShell.
fn connect_powershell(app: &AppHandle, t: &i18n::TrayTexts) -> (Status, String) {
    use integrations::pwsh::{self, Policy};

    let manual = |text: &str, status: Status| {
        crate::clipboard::copy_to_clipboard(pwsh::LINE);
        (status, text.to_string())
    };
    let hosts = pwsh::probe_hosts();
    if hosts.is_empty() {
        tracing::warn!("nenhum PowerShell respondeu");
        return manual(t.pwsh_failed, Status::Attention);
    }
    if hosts
        .iter()
        .any(|(_, p)| matches!(pwsh::policy(p), Some(Policy::Enforced(_))))
    {
        return manual(t.pwsh_enforced, Status::Attention);
    }
    let blocked: Vec<&str> = hosts
        .iter()
        .filter(|(_, p)| pwsh::policy(p) == Some(Policy::Blocked))
        .map(|(host, _)| *host)
        .collect();
    let mut profiles: Vec<std::path::PathBuf> = hosts
        .iter()
        .filter_map(|(_, p)| p.profile.clone())
        .filter(|file| {
            !std::fs::read(file).is_ok_and(|bytes| pwsh::sources_quacko(&pwsh::decode(&bytes)))
        })
        .collect();
    profiles.dedup();
    tracing::info!(?blocked, ?profiles, "terminal no Windows");
    if blocked.is_empty() && profiles.is_empty() {
        return (Status::Ok, t.shell_ready.replace("{file}", "$PROFILE"));
    }

    let (text, yes) = if blocked.is_empty() {
        (t.pwsh_ask_profile, t.pwsh_yes)
    } else {
        (t.pwsh_ask_unblock, t.pwsh_yes_unblock)
    };
    if !ask(app, t.pwsh_title, text, yes, t.pwsh_no) {
        return manual(t.pwsh_declined, Status::Info);
    }

    for host in blocked {
        let _ = pwsh::run(host, pwsh::UNBLOCK);
        let now =
            pwsh::run(host, pwsh::PROBE).and_then(|out| pwsh::policy(&pwsh::parse_probe(&out)));
        if now != Some(Policy::Allows) {
            tracing::warn!(host, ?now, "a política continua bloqueando scripts");
            return manual(t.pwsh_failed, Status::Fail);
        }
    }
    for profile in &profiles {
        if let Err(e) = pwsh::add_to_profile(profile) {
            tracing::warn!(%e, ?profile, "não deu para editar o perfil do PowerShell");
            return manual(t.pwsh_failed, Status::Fail);
        }
    }
    (Status::Ok, t.pwsh_done.to_string())
}

fn ask(app: &AppHandle, title: &str, text: &str, yes: &str, no: &str) -> bool {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    app.dialog()
        .message(text)
        .title(title)
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(yes.into(), no.into()))
        .blocking_show()
}

fn tilde(paths: &Paths, file: &std::path::Path) -> String {
    match file.strip_prefix(&paths.home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => file.display().to_string(),
    }
}
