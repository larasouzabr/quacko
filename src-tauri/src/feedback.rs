use tauri::{AppHandle, Manager};

use crate::model::{IdGen, Notification, Source, Status};

pub fn tell(app: &AppHandle, status: Status, message: String) {
    let message: String = message
        .chars()
        .take(crate::model::MAX_MESSAGE_CHARS)
        .collect();
    crate::overlay::present(
        app,
        Notification {
            id: app.state::<IdGen>().next(),
            source: Source::Custom,
            status,
            message: Some(message),
            title: Some("Quacko".into()),
            project: None,
            url: None,
            ttl_ms: 12_000,
            progress: None,
            sticky: false,
        },
    );
}
