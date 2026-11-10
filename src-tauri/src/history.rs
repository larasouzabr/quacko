use std::collections::VecDeque;
use std::sync::Mutex;

use crate::model::{Notification, Status};

pub const MAX_ENTRIES: usize = 10;
const LABEL_CHARS: usize = 60;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub notification: Notification,
    pub clock: String,
    pub silenced: bool,
}

#[derive(Default)]
pub struct History(Mutex<VecDeque<Entry>>);

pub fn worth_keeping(n: &Notification) -> bool {
    n.sticky || n.progress.is_none()
}

impl History {
    pub fn record(&self, notification: &Notification, clock: String, silenced: bool) -> bool {
        if !worth_keeping(notification) {
            return false;
        }
        let mut entries = self.0.lock().unwrap_or_else(|e| e.into_inner());
        entries.push_front(Entry {
            notification: notification.clone(),
            clock,
            silenced,
        });
        entries.truncate(MAX_ENTRIES);
        true
    }

    pub fn entries(&self) -> Vec<Entry> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect()
    }

    pub fn find(&self, id: u64) -> Option<Entry> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|e| e.notification.id == id)
            .cloned()
    }

    pub fn clear(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

fn symbol(status: Status) -> &'static str {
    match status {
        Status::Ok => "✓",
        Status::Fail => "✗",
        Status::Attention => "!",
        Status::Info => "i",
    }
}

pub fn label(entry: &Entry, silenced_mark: &str) -> String {
    let n = &entry.notification;
    let what = [
        n.title.as_deref(),
        n.project.as_deref(),
        n.message.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.trim().is_empty())
    .take(2)
    .collect::<Vec<_>>()
    .join(" · ");
    let mut text = format!("{} {}  {}", symbol(n.status), entry.clock, what);
    if entry.silenced {
        text.push_str(&format!(" ({silenced_mark})"));
    }
    if text.chars().count() > LABEL_CHARS {
        text = text.chars().take(LABEL_CHARS - 1).collect::<String>() + "…";
    }
    text
}

pub fn now_clock() -> String {
    chrono::Local::now().format("%H:%M").to_string()
}

pub fn remember(app: &tauri::AppHandle, notification: &Notification, silenced: bool) {
    use tauri::Manager;
    if app
        .state::<History>()
        .record(notification, now_clock(), silenced)
    {
        crate::tray::refresh(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Progress, Source};

    fn n(id: u64, status: Status) -> Notification {
        Notification {
            id,
            source: Source::Agent,
            status,
            message: Some("“adicione testes”".into()),
            title: Some("Claude Code".into()),
            project: Some("lagoa-api".into()),
            url: None,
            ttl_ms: 8000,
            progress: None,
            sticky: false,
        }
    }

    #[test]
    fn guarda_os_dez_mais_recentes_primeiro() {
        let h = History::default();
        for id in 1..=12 {
            h.record(&n(id, Status::Ok), "10:00".into(), false);
        }
        let ids: Vec<u64> = h.entries().iter().map(|e| e.notification.id).collect();
        assert_eq!(ids, (3..=12).rev().collect::<Vec<_>>());
        assert_eq!(h.find(12).map(|e| e.notification.id), Some(12));
        assert!(h.find(1).is_none());
        h.clear();
        assert!(h.entries().is_empty());
    }

    #[test]
    fn progresso_da_ci_nao_entra_so_o_resultado() {
        let h = History::default();
        let mut progress = n(1, Status::Info);
        progress.progress = Some(Progress {
            done: 3,
            total: 12,
            failed: 0,
        });
        assert!(!h.record(&progress, "10:00".into(), false));
        let mut done = progress.clone();
        done.sticky = true;
        assert!(h.record(&done, "10:01".into(), false));
        assert_eq!(h.entries().len(), 1);
    }

    #[test]
    fn rotulo_com_estado_hora_titulo_e_projeto() {
        let entry = Entry {
            notification: n(1, Status::Fail),
            clock: "14:32".into(),
            silenced: false,
        };
        assert_eq!(
            label(&entry, "silenciado"),
            "✗ 14:32  Claude Code · lagoa-api"
        );
        let quiet = Entry {
            silenced: true,
            ..entry.clone()
        };
        assert!(label(&quiet, "silenciado").ends_with("(silenciado)"));
        let mut long = entry;
        long.notification.title = Some("x".repeat(80));
        let text = label(&long, "silenciado");
        assert_eq!(text.chars().count(), 60);
        assert!(text.ends_with('…'));
    }
}
