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
    pub const NOTIFY_FROM: &str = "notify-from";
    pub const NOTIFY_CLAUDE: &str = "notify-claude";
    pub const NOTIFY_CURSOR: &str = "notify-cursor";
    pub const NOTIFY_CI: &str = "notify-ci";
    pub const NOTIFY_REVIEWS: &str = "notify-reviews";
    pub const NOTIFY_TERMINAL: &str = "notify-terminal";
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
    pub const CHECK_UPDATES: &str = "check-updates";
    pub const UPDATE: &str = "update";
    pub const TEXT_PREFIX: &str = "text-scale:";
    pub const RECENT_PREFIX: &str = "recent:";
    pub const RECENT_EMPTY: &str = "recent-empty";
    pub const RECENT_CLEAR: &str = "recent-clear";
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
    let origin = |id: &str, label: &str, on: bool| {
        CheckMenuItem::with_id(app, id, label, !config.dnd, on, None::<&str>)
    };
    let from_label = MenuItem::with_id(app, ids::NOTIFY_FROM, t.notify_from, false, None::<&str>)?;
    let from_claude = origin(ids::NOTIFY_CLAUDE, "Claude Code", config.notify_claude)?;
    let from_cursor = origin(ids::NOTIFY_CURSOR, "Cursor", config.notify_cursor)?;
    let from_ci = origin(ids::NOTIFY_CI, t.notify_ci, config.notify_ci)?;
    let from_reviews = origin(ids::NOTIFY_REVIEWS, t.notify_reviews, config.notify_reviews)?;
    let from_terminal = origin(
        ids::NOTIFY_TERMINAL,
        t.notify_terminal,
        config.notify_terminal,
    )?;
    let notices = Submenu::with_items(
        app,
        t.notices,
        true,
        &[
            &sound,
            &pause_media,
            &PredefinedMenuItem::separator(app)?,
            &from_label,
            &from_claude,
            &from_cursor,
            &from_ci,
            &from_reviews,
            &from_terminal,
            &PredefinedMenuItem::separator(app)?,
            &test,
        ],
    )?;
    let check_updates = CheckMenuItem::with_id(
        app,
        ids::CHECK_UPDATES,
        t.check_updates,
        true,
        config.check_updates,
        None::<&str>,
    )?;
    let text_items = crate::config::TEXT_SCALES
        .iter()
        .zip([t.text_normal, t.text_large, t.text_larger])
        .map(|(scale, label)| {
            CheckMenuItem::with_id(
                app,
                format!("{}{scale}", ids::TEXT_PREFIX),
                label,
                true,
                config.text_scale == *scale,
                None::<&str>,
            )
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let text_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = text_items
        .iter()
        .map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>)
        .collect();
    let text_size = Submenu::with_items(app, t.text_size, true, &text_refs)?;
    let preferences = Submenu::with_items(
        app,
        t.preferences,
        true,
        &[&autostart, &check_updates, &language, &text_size, &welcome],
    )?;
    let connect = connect_submenu(app, t)?;
    let recent = recent_submenu(app, t)?;
    let quit = MenuItem::with_id(app, ids::QUIT, t.quit, true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&status_item])?;
    if let Some(release) = app.state::<crate::update::Available>().get() {
        let update = MenuItem::with_id(
            app,
            ids::UPDATE,
            t.update_item.replace("{version}", &release.version),
            true,
            None::<&str>,
        )?;
        menu.append(&update)?;
    }
    menu.append_items(&[
        &PredefinedMenuItem::separator(app)?,
        &dnd,
        &notices,
        &recent,
        &connect,
        &PredefinedMenuItem::separator(app)?,
        &customize,
        &preferences,
        &PredefinedMenuItem::separator(app)?,
        &quit,
    ])?;
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

fn recent_submenu(app: &AppHandle, t: &TrayTexts) -> tauri::Result<Submenu<Wry>> {
    let entries = app.state::<crate::history::History>().entries();
    let submenu = Submenu::with_id(app, "recent", t.recent, true)?;
    if entries.is_empty() {
        submenu.append(&MenuItem::with_id(
            app,
            ids::RECENT_EMPTY,
            t.recent_empty,
            false,
            None::<&str>,
        )?)?;
        return Ok(submenu);
    }
    for entry in &entries {
        submenu.append(&MenuItem::with_id(
            app,
            format!("{}{}", ids::RECENT_PREFIX, entry.notification.id),
            crate::history::label(entry, t.silenced),
            true,
            None::<&str>,
        )?)?;
    }
    submenu.append(&PredefinedMenuItem::separator(app)?)?;
    submenu.append(&MenuItem::with_id(
        app,
        ids::RECENT_CLEAR,
        t.recent_clear,
        true,
        None::<&str>,
    )?)?;
    Ok(submenu)
}

fn open_recent(app: &AppHandle, id: u64) {
    let Some(entry) = app.state::<crate::history::History>().find(id) else {
        return;
    };
    if let Some(url) = entry.notification.url.clone() {
        use tauri_plugin_opener::OpenerExt;
        if let Err(e) = app.opener().open_url(url, None::<&str>) {
            tracing::warn!(%e, "não foi possível abrir o link do aviso");
        }
        return;
    }
    let mut again = entry.notification;
    again.id = app.state::<crate::model::IdGen>().next();
    again.sticky = false;
    crate::overlay::present(app, again);
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let settings = app.state::<Settings>();
    if let Some(id) = event
        .id()
        .as_ref()
        .strip_prefix(ids::RECENT_PREFIX)
        .and_then(|id| id.parse::<u64>().ok())
    {
        open_recent(app, id);
        return;
    }
    if let Some(scale) = event
        .id()
        .as_ref()
        .strip_prefix(ids::TEXT_PREFIX)
        .and_then(|s| s.parse::<u16>().ok())
        .filter(|s| crate::config::TEXT_SCALES.contains(s))
    {
        settings.update(|c| c.text_scale = scale);
        settings::broadcast(app);
        refresh(app);
        return;
    }
    match event.id().as_ref() {
        ids::RECENT_CLEAR => app.state::<crate::history::History>().clear(),
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
        ids::NOTIFY_CLAUDE => {
            settings.update(|c| c.notify_claude = !c.notify_claude);
        }
        ids::NOTIFY_CURSOR => {
            settings.update(|c| c.notify_cursor = !c.notify_cursor);
        }
        ids::NOTIFY_CI => {
            settings.update(|c| c.notify_ci = !c.notify_ci);
        }
        ids::NOTIFY_REVIEWS => {
            settings.update(|c| c.notify_reviews = !c.notify_reviews);
        }
        ids::NOTIFY_TERMINAL => {
            settings.update(|c| c.notify_terminal = !c.notify_terminal);
        }
        ids::AUTOSTART => crate::autostart::toggle_autostart(app),
        ids::CHECK_UPDATES => {
            let c = settings.update(|c| c.check_updates = !c.check_updates);
            tracing::info!(
                check_updates = c.check_updates,
                "procurar atualizações alterado"
            );
        }
        ids::UPDATE => {
            if let Some(release) = app.state::<crate::update::Available>().get() {
                use tauri_plugin_opener::OpenerExt;
                if let Err(e) = app.opener().open_url(release.url, None::<&str>) {
                    tracing::warn!(%e, "não foi possível abrir a página da versão nova");
                }
            }
        }
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
    let shell = MenuItem::with_id(app, ids::SHELL, t.shell_item, paths.is_some(), None::<&str>)?;
    submenu.append_items(&[&PredefinedMenuItem::separator(app)?, &shell])?;
    if cfg!(target_os = "macos") {
        let prwatch = MenuItem::with_id(
            app,
            ids::PRWATCH,
            t.prwatch_item,
            paths.is_some(),
            None::<&str>,
        )?;
        submenu.append(&prwatch)?;
    }
    Ok(submenu)
}
