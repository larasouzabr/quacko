use crate::feedback::tell;
use tauri::{AppHandle, Manager};

use crate::i18n::{self};
use crate::model::Status;
use crate::settings::Settings;

pub fn toggle_github(app: &AppHandle) {
    let settings = app.state::<Settings>();
    let on = settings.update(|c| c.github = !c.github).github;
    app.state::<crate::github::Monitor>().set_enabled(on);
    if !on {
        let t = i18n::tray_texts(settings.lang());
        tell(app, Status::Info, t.github_off.to_string());
    }
}

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
                crate::onboarding::open(app);
            }
            app.state::<Settings>().update(|c| c.github = false);
            app.state::<crate::github::Monitor>().set_enabled(false);
            tell(app, Status::Attention, msg.to_string());
            crate::tray::refresh(app);
        }
    }
}
