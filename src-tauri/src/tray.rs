//! Ícone e menu da barra (seção 9 da spec).
//!
//! O menu é reconstruído a cada mudança: mantém as marcas (✓) e o idioma sempre certos.

use tauri::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Manager, Wry,
};

use crate::config::LangPref;
use crate::i18n::{self, TrayTexts};
use crate::integrations::{self, ManualReason, Outcome, Paths, PrwatchOutcome, Tool};
use crate::model::{IdGen, Notification, Source, Status};
use crate::settings::{self, Settings};

use crate::tray_icon::TRAY_ID;

mod ids {
    pub const STATUS: &str = "status";
    pub const DND: &str = "dnd";
    pub const TEST: &str = "test";
    pub const AUTOSTART: &str = "autostart";
    pub const LANG_AUTO: &str = "lang-auto";
    pub const LANG_PT: &str = "lang-pt";
    pub const LANG_EN: &str = "lang-en";
    pub const QUIT: &str = "quit";
    pub const CONNECT_CLAUDE: &str = "connect-claude";
    pub const CONNECT_CURSOR: &str = "connect-cursor";
    pub const PRWATCH: &str = "prwatch";
    pub const CONNECT_GITHUB: &str = "connect-github";
    pub const WELCOME: &str = "welcome";
}

#[derive(Debug, Clone)]
pub enum ServerStatus {
    Listening { port: u16 },
    PortBusy { port: u16 },
    Unavailable { port: u16 },
}

/// Status do servidor guardado para remontar o menu.
struct TrayStatus(ServerStatus);

impl ServerStatus {
    fn menu_text(&self, t: &TrayTexts) -> String {
        match self {
            Self::Listening { port } => format!("{} {port}", t.listening),
            Self::PortBusy { port } => format!("⚠ {port}: {}", t.port_busy),
            Self::Unavailable { port } => format!("⚠ {port}: {}", t.port_unavailable),
        }
    }

    fn tooltip(&self, t: &TrayTexts) -> String {
        format!("Miau · {}", self.menu_text(t))
    }
}

pub fn create(app: &AppHandle, status: ServerStatus) -> tauri::Result<()> {
    app.manage(TrayStatus(status));
    let (menu, tooltip) = build_menu(app)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(crate::tray_icon::initial_image())
        .icon_as_template(true)
        .tooltip(tooltip)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| {
            // Abrir o menu (clicar no ícone) conta como "vi o aviso": tira o alerta.
            if let tauri::tray::TrayIconEvent::Click {
                button_state: tauri::tray::MouseButtonState::Down,
                ..
            } = event
            {
                crate::tray_icon::set_alert(tray.app_handle(), false);
            }
        })
        .build(app)?;
    crate::tray_icon::apply_current(app);
    Ok(())
}

fn build_menu(app: &AppHandle) -> tauri::Result<(Menu<Wry>, String)> {
    let settings = app.state::<Settings>();
    let config = settings.snapshot();
    let t = i18n::tray_texts(settings.lang());
    let status = &app.state::<TrayStatus>().0;

    // Item desabilitado: só informa. Com a porta ocupada o app continua aberto para mostrar o erro.
    let status_item =
        MenuItem::with_id(app, ids::STATUS, status.menu_text(t), false, None::<&str>)?;
    let dnd = CheckMenuItem::with_id(app, ids::DND, t.dnd, true, config.dnd, None::<&str>)?;
    let test = MenuItem::with_id(app, ids::TEST, t.test, true, None::<&str>)?;
    let welcome = MenuItem::with_id(app, ids::WELCOME, t.welcome, true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(
        app,
        ids::AUTOSTART,
        t.autostart,
        true,
        config.autostart,
        None::<&str>,
    )?;
    let lang_auto = CheckMenuItem::with_id(
        app,
        ids::LANG_AUTO,
        t.lang_auto,
        true,
        config.lang == LangPref::Auto,
        None::<&str>,
    )?;
    let lang_pt = CheckMenuItem::with_id(
        app,
        ids::LANG_PT,
        "Português",
        true,
        config.lang == LangPref::PtBr,
        None::<&str>,
    )?;
    let lang_en = CheckMenuItem::with_id(
        app,
        ids::LANG_EN,
        "English",
        true,
        config.lang == LangPref::En,
        None::<&str>,
    )?;
    let language = Submenu::with_items(app, t.language, true, &[&lang_auto, &lang_pt, &lang_en])?;
    let connect = connect_submenu(app, t)?;
    let quit = MenuItem::with_id(app, ids::QUIT, t.quit, true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &status_item,
            &PredefinedMenuItem::separator(app)?,
            &dnd,
            &test,
            &connect,
            &autostart,
            &language,
            &welcome,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    Ok((menu, status.tooltip(t)))
}

/// Remonta menu e tooltip. Feito no próximo ciclo da thread principal, depois que o
/// menu que gerou o clique já fechou.
pub fn refresh(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(tray) = handle.tray_by_id(TRAY_ID) else {
            return;
        };
        match build_menu(&handle) {
            Ok((menu, tooltip)) => {
                let _ = tray.set_menu(Some(menu));
                let _ = tray.set_tooltip(Some(tooltip));
            }
            Err(e) => tracing::warn!(%e, "não foi possível remontar o menu"),
        }
    });
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let settings = app.state::<Settings>();
    match event.id().as_ref() {
        ids::QUIT => {
            app.exit(0);
            return;
        }
        ids::TEST => crate::window::run_test_sequence(app),
        ids::WELCOME => crate::onboarding::open(app),
        ids::DND => {
            let c = settings.update(|c| c.dnd = !c.dnd);
            tracing::info!(dnd = c.dnd, "não perturbe alterado");
        }
        ids::AUTOSTART => toggle_autostart(app),
        ids::LANG_AUTO => set_lang(app, LangPref::Auto),
        ids::LANG_PT => set_lang(app, LangPref::PtBr),
        ids::LANG_EN => set_lang(app, LangPref::En),
        ids::CONNECT_CLAUDE => toggle_integration(app, Tool::Claude),
        ids::CONNECT_CURSOR => toggle_integration(app, Tool::Cursor),
        ids::PRWATCH => install_prwatch(app),
        ids::CONNECT_GITHUB => toggle_github(app),
        _ => return,
    }
    // Também desfaz a marca que o macOS põe sozinho num item de seleção clicado.
    refresh(app);
}

fn set_lang(app: &AppHandle, lang: LangPref) {
    app.state::<Settings>().update(|c| c.lang = lang);
    settings::broadcast(app);
}

fn toggle_autostart(app: &AppHandle) {
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

// ---------- Conectar (Claude Code, Cursor, prwatch) ----------

/// Submenu "Conectar": a marca (✓) diz se a ferramenta já está ligada ao Miau.
fn connect_submenu(app: &AppHandle, t: &TrayTexts) -> tauri::Result<Submenu<Wry>> {
    let paths = Paths::from_home();
    let connected = |tool| {
        paths
            .as_ref()
            .is_some_and(|p| integrations::status(p, tool))
    };
    let cursor_present = paths.as_ref().is_some_and(Paths::cursor_present);
    let cursor_label = if cursor_present {
        "Cursor".to_string()
    } else {
        format!("Cursor {}", t.not_found)
    };

    let claude = CheckMenuItem::with_id(
        app,
        ids::CONNECT_CLAUDE,
        "Claude Code",
        paths.is_some(),
        connected(Tool::Claude),
        None::<&str>,
    )?;
    let cursor = CheckMenuItem::with_id(
        app,
        ids::CONNECT_CURSOR,
        cursor_label,
        cursor_present,
        connected(Tool::Cursor),
        None::<&str>,
    )?;
    let github_on = app.state::<Settings>().snapshot().github;
    let github = CheckMenuItem::with_id(
        app,
        ids::CONNECT_GITHUB,
        t.github_label,
        true,
        github_on,
        None::<&str>,
    )?;
    let prwatch = MenuItem::with_id(
        app,
        ids::PRWATCH,
        t.prwatch_item,
        paths.is_some(),
        None::<&str>,
    )?;
    Submenu::with_items(
        app,
        t.connect,
        true,
        &[
            &claude,
            &cursor,
            &github,
            &PredefinedMenuItem::separator(app)?,
            &prwatch,
        ],
    )
}

/// Liga ou desliga a ferramenta (o contrário do estado atual) e conta o resultado no cartão.
fn toggle_integration(app: &AppHandle, tool: Tool) {
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
                copy_to_clipboard(&snippet);
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
        refresh(&app);
    });
}

fn install_prwatch(app: &AppHandle) {
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

/// Mostra o resultado no cartão do gato (mesmo com "Não perturbar": foi um pedido explícito).
pub fn tell(app: &AppHandle, status: Status, message: String) {
    let message: String = message
        .chars()
        .take(crate::model::MAX_MESSAGE_CHARS)
        .collect();
    crate::window::present(
        app,
        Notification {
            id: app.state::<IdGen>().next(),
            source: Source::Custom,
            status,
            message: Some(message),
            title: Some("Miau".into()),
            project: None,
            url: None,
            ttl_ms: 12_000,
            progress: None,
            sticky: false,
        },
    );
}

fn tilde(paths: &Paths, file: &std::path::Path) -> String {
    match file.strip_prefix(&paths.home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => file.display().to_string(),
    }
}

/// Modo manual: o trecho vai para a área de transferência.
pub fn copy_to_clipboard(text: &str) {
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};
        if let Ok(mut child) = Command::new("/usr/bin/pbcopy")
            .stdin(Stdio::piped())
            .spawn()
        {
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = text;
}

// ---------- GitHub (monitor de PRs) ----------

fn toggle_github(app: &AppHandle) {
    let settings = app.state::<Settings>();
    let on = settings.update(|c| c.github = !c.github).github;
    app.state::<crate::github::Monitor>().set_enabled(on);
    if !on {
        let t = i18n::tray_texts(settings.lang());
        tell(app, Status::Info, t.github_off.to_string());
    }
    // Ao ligar, o próprio monitor conta no cartão se conectou (ou por que não).
}

/// Resultado do monitor de PRs, mostrado no cartão. Sem login, desliga o monitor.
pub fn github_report(app: &AppHandle, report: crate::github::Report) {
    use crate::github::Report;
    let t = i18n::tray_texts(app.state::<Settings>().lang());
    match report {
        Report::Connected { login, open_prs } => {
            let msg = t
                .github_connected
                .replace("{login}", &login)
                .replace("{n}", &open_prs.to_string());
            tell(app, Status::Ok, msg);
        }
        Report::NoGh | Report::LoginExpired => {
            let device_flow =
                crate::github::device_flow_client_id(&app.state::<Settings>().snapshot()).is_some();
            let msg = if device_flow {
                t.github_sign_in
            } else if matches!(report, Report::NoGh) {
                t.github_no_gh
            } else {
                t.github_expired
            };
            if device_flow {
                // A tela de boas-vindas tem o botão "Entrar com código".
                crate::onboarding::open(app);
            }
            app.state::<Settings>().update(|c| c.github = false);
            app.state::<crate::github::Monitor>().set_enabled(false);
            tell(app, Status::Attention, msg.to_string());
            refresh(app);
        }
    }
}

/// Ao abrir o app: atualiza os scripts instalados que estiverem desatualizados e avisa
/// com um cartão discreto (que respeita o "Não perturbar").
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

/// Ao abrir o app: se algum agente está conectado e falta o nowplaying-cli, avisa uma vez.
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
