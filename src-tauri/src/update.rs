use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::model::{IdGen, Notification, Source, Status};
use crate::settings::Settings;

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// Link especial do card: instala a versão já baixada em vez de abrir uma página.
/// Avisos que chegam de fora passam por `clean_url`, que só aceita http(s),
/// então só o próprio app consegue criar um card com esse link.
pub const APPLY_ACTION: &str = "quacko:update";
const RELEASE_PAGE: &str = "https://github.com/larasouzabr/quacko/releases/tag/v";
const FIRST_CHECK: Duration = Duration::from_secs(60);
const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub url: String,
    /// Já baixada e com a assinatura conferida: um clique instala e reinicia.
    pub ready: bool,
}

impl Release {
    fn new(version: &str, ready: bool) -> Self {
        Release {
            version: version.to_string(),
            url: format!("{RELEASE_PAGE}{version}"),
            ready,
        }
    }
}

#[derive(Default)]
pub struct Available {
    release: Mutex<Option<Release>>,
    downloaded: Mutex<Option<(Update, Vec<u8>)>>,
}

impl Available {
    pub fn get(&self) -> Option<Release> {
        self.release
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn set(&self, release: Option<Release>) {
        *self.release.lock().unwrap_or_else(|e| e.into_inner()) = release;
    }

    fn downloaded_version(&self) -> Option<String> {
        let slot = self.downloaded.lock().unwrap_or_else(|e| e.into_inner());
        slot.as_ref().map(|(u, _)| u.version.clone())
    }

    fn keep(&self, update: Update, bytes: Vec<u8>) {
        *self.downloaded.lock().unwrap_or_else(|e| e.into_inner()) = Some((update, bytes));
    }

    fn take(&self) -> Option<(Update, Vec<u8>)> {
        self.downloaded
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }
}

fn parts(version: &str) -> Option<Vec<u64>> {
    let core = version.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next()?;
    core.split('.').map(|p| p.parse().ok()).collect()
}

pub fn is_version(text: &str) -> bool {
    text.len() <= 32 && parts(text).is_some()
}

pub fn is_newer(current: &str, candidate: &str) -> bool {
    match (parts(current), parts(candidate)) {
        (Some(mut a), Some(mut b)) => {
            let len = a.len().max(b.len());
            a.resize(len, 0);
            b.resize(len, 0);
            b > a
        }
        _ => false,
    }
}

pub fn should_announce(current: &str, release: &Release, already: Option<&str>) -> bool {
    is_newer(current, &release.version) && already != Some(release.version.as_str())
}

pub fn spawn(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            if app.state::<Settings>().snapshot().check_updates {
                check(&app).await;
            }
            tokio::time::sleep(EVERY).await;
        }
    });
}

/// Procura o latest.json da última release. Achando versão nova, baixa e confere a
/// assinatura com a chave pública do tauri.conf.json. Nada é instalado sem um clique.
async fn check(app: &AppHandle) {
    let found = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => {
            tracing::warn!(%e, "atualizador indisponível");
            return;
        }
    };
    let update = match found {
        Ok(Some(update)) => update,
        Ok(None) => {
            app.state::<Available>().set(None);
            return;
        }
        Err(e) => {
            tracing::debug!(%e, "não foi possível procurar atualizações");
            return;
        }
    };
    let available = app.state::<Available>();
    if available.downloaded_version().as_deref() == Some(update.version.as_str()) {
        return;
    }
    let version = update.version.clone();
    let ready = match update.download(|_, _| {}, || {}).await {
        Ok(bytes) => {
            available.keep(update, bytes);
            true
        }
        Err(e) => {
            tracing::warn!(%e, version, "não foi possível baixar a versão nova");
            false
        }
    };
    let release = Release::new(&version, ready);
    available.set(Some(release.clone()));
    crate::tray::refresh(app);
    announce(app, release);
}

fn announce(app: &AppHandle, release: Release) {
    let settings = app.state::<Settings>();
    let config = settings.snapshot();
    if !should_announce(CURRENT, &release, config.update_announced.as_deref()) || config.dnd {
        return;
    }
    settings.update(|c| c.update_announced = Some(release.version.clone()));
    let t = crate::i18n::tray_texts(settings.lang());
    let (message, url) = if release.ready {
        (t.update_ready_message, APPLY_ACTION.to_string())
    } else {
        (t.update_message, release.url)
    };
    crate::overlay::present(
        app,
        Notification {
            id: app.state::<IdGen>().next(),
            source: Source::Custom,
            status: Status::Info,
            message: Some(message.to_string()),
            title: Some(format!("Quacko {}", release.version)),
            project: None,
            url: Some(url),
            ttl_ms: crate::model::TTL_MAX_MS,
            progress: None,
            sticky: false,
        },
    );
}

/// Clique no card ou no item da bandeja: instala a versão baixada e reinicia.
/// Sem nada baixado (ou se a instalação falhar), abre a página da release.
pub fn apply(app: &AppHandle) {
    let available = app.state::<Available>();
    let Some(release) = available.get() else {
        return;
    };
    let Some((update, bytes)) = available.take() else {
        open_page(app, &release.url);
        return;
    };
    tracing::info!(version = %update.version, "instalando a versão nova");
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // No Windows, install() abre o instalador e encerra o app sozinho.
        match update.install(&bytes) {
            Ok(()) => app.restart(),
            Err(e) => {
                tracing::warn!(%e, "não foi possível instalar a versão nova");
                app.state::<Available>()
                    .set(Some(Release::new(&release.version, false)));
                crate::tray::refresh(&app);
                open_page(&app, &release.url);
            }
        }
    });
}

fn open_page(app: &AppHandle, url: &str) {
    use tauri_plugin_opener::OpenerExt;
    if let Err(e) = app.opener().open_url(url, None::<&str>) {
        tracing::warn!(%e, "não foi possível abrir a página da versão nova");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_versoes() {
        assert!(is_version("0.2.0") && is_version("v1.10"));
        assert!(!is_version("") && !is_version("nova") && !is_version(&"1.".repeat(20)));
    }

    #[test]
    fn compara_versoes() {
        assert!(is_newer("0.1.0", "0.2.0"));
        assert!(is_newer("0.1.0", "v0.1.1"));
        assert!(is_newer("0.9.9", "1.0"));
        assert!(is_newer("0.1.9", "0.1.10"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.2.0", "v0.1.5"));
        assert!(!is_newer("0.1.0", "0.1.0-beta"));
        assert!(!is_newer("0.1.0", "nova"));
    }

    #[test]
    fn release_aponta_para_a_pagina_da_tag() {
        let r = Release::new("0.4.0", true);
        assert_eq!(
            r.url,
            "https://github.com/larasouzabr/quacko/releases/tag/v0.4.0"
        );
        assert!(crate::model::clean_url(&r.url).is_some());
    }

    #[test]
    fn link_de_instalar_nao_vem_de_fora() {
        assert_eq!(crate::model::clean_url(APPLY_ACTION), None);
    }

    #[test]
    fn avisa_uma_vez_por_versao() {
        let r = Release::new("0.2.0", false);
        assert!(should_announce("0.1.0", &r, None));
        assert!(should_announce("0.1.0", &r, Some("0.1.5")));
        assert!(!should_announce("0.1.0", &r, Some("0.2.0")));
        assert!(!should_announce("0.2.0", &r, None));
    }
}
