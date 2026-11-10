use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::model::{IdGen, Notification, Source, Status};
use crate::settings::Settings;

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
const LATEST_URL: &str = "https://api.github.com/repos/larasouzabr/quacko/releases/latest";
const FIRST_CHECK: Duration = Duration::from_secs(60);
const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub url: String,
}

#[derive(Default)]
pub struct Available(Mutex<Option<Release>>);

impl Available {
    pub fn get(&self) -> Option<Release> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn set(&self, release: Option<Release>) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = release;
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

pub fn parse_latest(body: &Value) -> Option<Release> {
    if body["draft"].as_bool() == Some(true) || body["prerelease"].as_bool() == Some(true) {
        return None;
    }
    let tag = body["tag_name"].as_str()?;
    let url = crate::model::clean_url(body["html_url"].as_str()?)?;
    parts(tag)?;
    Some(Release {
        version: tag.trim_start_matches('v').to_string(),
        url,
    })
}

pub fn should_announce(current: &str, release: &Release, already: Option<&str>) -> bool {
    is_newer(current, &release.version) && already != Some(release.version.as_str())
}

pub fn spawn(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let client = match reqwest::Client::builder()
            .user_agent(concat!(
                "Quacko/",
                env!("CARGO_PKG_VERSION"),
                " (update check)"
            ))
            .timeout(Duration::from_secs(20))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(%e, "sem cliente HTTP para procurar atualizações");
                return;
            }
        };
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            if app.state::<Settings>().snapshot().check_updates {
                check(&app, &client).await;
            }
            tokio::time::sleep(EVERY).await;
        }
    });
}

async fn check(app: &AppHandle, client: &reqwest::Client) {
    let body = match client
        .get(LATEST_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r.json::<Value>().await.ok(),
        Ok(r) => {
            tracing::debug!(status = %r.status(), "sem release publicada");
            None
        }
        Err(e) => {
            tracing::debug!(%e, "não foi possível procurar atualizações");
            None
        }
    };
    let Some(release) = body.as_ref().and_then(parse_latest) else {
        return;
    };
    let newer = is_newer(CURRENT, &release.version);
    app.state::<Available>().set(newer.then(|| release.clone()));
    if !newer {
        return;
    }
    crate::tray::refresh(app);
    let settings = app.state::<Settings>();
    let config = settings.snapshot();
    if !should_announce(CURRENT, &release, config.update_announced.as_deref()) || config.dnd {
        return;
    }
    settings.update(|c| c.update_announced = Some(release.version.clone()));
    let t = crate::i18n::tray_texts(settings.lang());
    crate::overlay::present(
        app,
        Notification {
            id: app.state::<IdGen>().next(),
            source: Source::Custom,
            status: Status::Info,
            message: Some(t.update_message.to_string()),
            title: Some(format!("Quacko {}", release.version)),
            project: None,
            url: Some(release.url),
            ttl_ms: crate::model::TTL_MAX_MS,
            progress: None,
            sticky: false,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
    fn le_a_release_mais_recente() {
        let body = json!({"tag_name": "v0.2.0", "html_url": "https://github.com/larasouzabr/quacko/releases/tag/v0.2.0", "draft": false, "prerelease": false});
        assert_eq!(
            parse_latest(&body),
            Some(Release {
                version: "0.2.0".into(),
                url: "https://github.com/larasouzabr/quacko/releases/tag/v0.2.0".into()
            })
        );
        assert_eq!(
            parse_latest(
                &json!({"tag_name": "v0.3.0", "html_url": "https://x.y/z", "prerelease": true})
            ),
            None
        );
        assert_eq!(
            parse_latest(&json!({"tag_name": "lançamento", "html_url": "https://x.y/z"})),
            None
        );
        assert_eq!(
            parse_latest(&json!({"tag_name": "v1.0.0", "html_url": "javascript:alert(1)"})),
            None
        );
        assert_eq!(parse_latest(&json!({"message": "Not Found"})), None);
    }

    #[test]
    fn avisa_uma_vez_por_versao() {
        let r = Release {
            version: "0.2.0".into(),
            url: "https://x.y/z".into(),
        };
        assert!(should_announce("0.1.0", &r, None));
        assert!(should_announce("0.1.0", &r, Some("0.1.5")));
        assert!(!should_announce("0.1.0", &r, Some("0.2.0")));
        assert!(!should_announce("0.2.0", &r, None));
    }
}
