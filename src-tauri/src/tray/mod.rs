pub mod icon;

use tauri::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Manager, Wry,
};

use crate::config::LangPref;
use crate::i18n::{self, TrayTexts};
use crate::integrations::{self, Paths, Tool};
use crate::settings::{self, Settings};

use crate::tray::icon::TRAY_ID;

mod ids {
    pub const STATUS: &str = "status";
    pub const DND: &str = "dnd";
    pub const SOUND: &str = "sound";
    pub const PAUSE_MEDIA: &str = "pause-media";
    pub const TEST: &str = "test";
    pub const AUTOSTART: &str = "autostart";
    pub const LANG_AUTO: &str = "lang-auto";
    pub const LANG_PT: &str = "lang-pt";
    pub const LANG_EN: &str = "lang-en";
    pub const QUIT: &str = "quit";
    pub const CONNECT_CLAUDE: &str = "connect-claude";
    pub const CONNECT_CURSOR: &str = "connect-cursor";
    pub const PRWATCH: &str = "prwatch";
    pub const SHELL: &str = "shell";
    pub const CONNECT_GITHUB: &str = "connect-github";
    pub const WELCOME: &str = "welcome";
    pub const CUSTOMIZE: &str = "customize";
}

#[derive(Debug, Clone)]
pub enum ServerStatus {
    Listening { port: u16 },
    PortBusy { port: u16 },
    Unavailable { port: u16 },
}

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
        format!("Quacko · {}", self.menu_text(t))
    }
}

pub fn create(app: &AppHandle, status: ServerStatus) -> tauri::Result<()> {
    app.manage(TrayStatus(status));
    let (menu, tooltip) = build_menu(app)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(crate::tray::icon::initial_image())
        .icon_as_template(true)
        .tooltip(tooltip)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button_state: tauri::tray::MouseButtonState::Down,
                ..
            } = event
            {
                crate::tray::icon::set_alert(tray.app_handle(), false);
            }
        })
        .build(app)?;
    crate::tray::icon::apply_current(app);
    Ok(())
}

fn build_menu(app: &AppHandle) -> tauri::Result<(Menu<Wry>, String)> {
    let settings = app.state::<Settings>();
    let config = settings.snapshot();
    let t = i18n::tray_texts(settings.lang());
    let status = &app.state::<TrayStatus>().0;

    let status_item =
        MenuItem::with_id(app, ids::STATUS, status.menu_text(t), false, None::<&str>)?;
    let dnd = CheckMenuItem::with_id(app, ids::DND, t.dnd, true, config.dnd, None::<&str>)?;
    let sound = CheckMenuItem::with_id(
        app,
        ids::SOUND,
        t.sound,
        !config.dnd,
        config.sound,
        None::<&str>,
    )?;
    let pause_media = CheckMenuItem::with_id(
        app,
        ids::PAUSE_MEDIA,
        t.pause_media,
        !config.dnd,
        config.pause_media,
        None::<&str>,
    )?;
    let test = MenuItem::with_id(app, ids::TEST, t.test, true, None::<&str>)?;
    let welcome = MenuItem::with_id(app, ids::WELCOME, t.welcome, true, None::<&str>)?;
    let customize = MenuItem::with_id(app, ids::CUSTOMIZE, t.customize, true, None::<&str>)?;
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
            &sound,
            &pause_media,
            &test,
            &connect,
            &autostart,
            &language,
            &customize,
            &welcome,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    Ok((menu, status.tooltip(t)))
}

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
        ids::TEST => crate::test_sequence::run_test_sequence(app),
        ids::WELCOME => crate::onboarding::open(app),
        ids::CUSTOMIZE => crate::customize::open(app),
        ids::DND => {
            let c = settings.update(|c| c.dnd = !c.dnd);
            tracing::info!(dnd = c.dnd, "não perturbe alterado");
        }
        ids::SOUND => {
            let c = settings.update(|c| c.sound = !c.sound);
            tracing::info!(sound = c.sound, "som alterado");
        }
        ids::PAUSE_MEDIA => {
            let c = settings.update(|c| c.pause_media = !c.pause_media);
            tracing::info!(pause_media = c.pause_media, "pausar música alterado");
        }
        ids::AUTOSTART => crate::autostart::toggle_autostart(app),
        ids::LANG_AUTO => set_lang(app, LangPref::Auto),
        ids::LANG_PT => set_lang(app, LangPref::PtBr),
        ids::LANG_EN => set_lang(app, LangPref::En),
        ids::CONNECT_CLAUDE => crate::integrations::actions::toggle_integration(app, Tool::Claude),
        ids::CONNECT_CURSOR => crate::integrations::actions::toggle_integration(app, Tool::Cursor),
        ids::PRWATCH => crate::integrations::actions::install_prwatch(app),
        ids::SHELL => crate::integrations::actions::install_shell(app),
        ids::CONNECT_GITHUB => crate::github::ui::toggle_github(app),
        _ => return,
    }
    refresh(app);
}

fn set_lang(app: &AppHandle, lang: LangPref) {
    app.state::<Settings>().update(|c| c.lang = lang);
    settings::broadcast(app);
}

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
    let submenu = Submenu::with_items(app, t.connect, true, &[&claude, &cursor, &github])?;
    if cfg!(target_os = "macos") {
        let prwatch = MenuItem::with_id(
            app,
            ids::PRWATCH,
            t.prwatch_item,
            paths.is_some(),
            None::<&str>,
        )?;
        let shell =
            MenuItem::with_id(app, ids::SHELL, t.shell_item, paths.is_some(), None::<&str>)?;
        submenu.append_items(&[&PredefinedMenuItem::separator(app)?, &shell, &prwatch])?;
    }
    Ok(submenu)
}
