//! Login no GitHub sem o `gh`: device flow de um GitHub App (spec v2, seção 6.2).
//!
//! Conferido na documentação do GitHub (2026-11-09):
//! - `POST /login/device/code` com `client_id` → `device_code`, `user_code`,
//!   `verification_uri`, `expires_in`, `interval`.
//! - `POST /login/oauth/access_token` com `client_id`, `device_code` e
//!   `grant_type=urn:ietf:params:oauth:grant-type:device_code`; sem `client_secret`.
//! - Erros: `authorization_pending` (continua), `slow_down` (usa o intervalo novo ou +5 s),
//!   `expired_token`/`token_expired` e `access_denied` (recomeça).
//! - Com expiração ligada (padrão do GitHub App): token de 8 h + refresh de 6 meses. O
//!   refresh de um token do device flow **não** precisa do `client_secret`, e cada refresh
//!   troca os dois tokens (o antigo deixa de valer).
//!
//! O token fica no Keychain (nunca em arquivo). Só o `client_id`, que é público, fica na
//! config ou na variável `MIAU_GITHUB_CLIENT_ID`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const GITHUB_WEB: &str = "https://github.com";
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const KEYCHAIN_SERVICE: &str = "dev.miau.notifier.github";
const KEYCHAIN_ACCOUNT: &str = "device-flow";
/// Renova o token quando faltar menos que isso para expirar.
const REFRESH_MARGIN_S: u64 = 300;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// Token guardado no Keychain. `expires_at` em segundos Unix; `None` = não expira
/// (GitHub App com expiração desligada).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
    pub refresh_expires_at: Option<u64>,
}

#[derive(Debug, PartialEq)]
pub enum AuthError {
    /// A pessoa cancelou no GitHub.
    Denied,
    /// O código expirou antes de ser usado: pedir outro.
    Expired,
    /// Refresh inválido ou vencido: fazer login de novo.
    NeedsLogin,
    /// Erro de configuração do app (client_id errado, device flow desligado no app).
    Config(String),
    Network(String),
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Client ID aceito: só letras, números, ponto, hífen e sublinhado (ex.: `Iv23li…`).
pub fn valid_client_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

/// Monta o token a partir da resposta de sucesso.
fn token_from(body: &Value, now: u64) -> Option<StoredToken> {
    let access_token = body.get("access_token")?.as_str()?.to_string();
    let secs = |k: &str| body.get(k).and_then(Value::as_u64);
    Some(StoredToken {
        access_token,
        refresh_token: body
            .get("refresh_token")
            .and_then(Value::as_str)
            .map(String::from),
        expires_at: secs("expires_in").map(|s| now + s),
        refresh_expires_at: secs("refresh_token_expires_in").map(|s| now + s),
    })
}

pub fn needs_refresh(token: &StoredToken, now: u64) -> bool {
    token
        .expires_at
        .is_some_and(|at| now + REFRESH_MARGIN_S >= at)
}

fn can_refresh(token: &StoredToken, now: u64) -> bool {
    token.refresh_token.is_some() && token.refresh_expires_at.is_none_or(|at| now < at)
}

pub struct Client {
    http: reqwest::Client,
    base: String,
    client_id: String,
}

impl Client {
    pub fn new(base: &str, client_id: &str) -> Result<Self, AuthError> {
        if !valid_client_id(client_id) {
            return Err(AuthError::Config("client_id inválido".into()));
        }
        let http = reqwest::Client::builder()
            .user_agent(concat!(
                "Miau/",
                env!("CARGO_PKG_VERSION"),
                " (desktop notifier)"
            ))
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| AuthError::Network(e.to_string()))?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            client_id: client_id.to_string(),
        })
    }

    async fn post(&self, path: &str, form: &[(&str, &str)]) -> Result<Value, AuthError> {
        self.http
            .post(format!("{}{path}", self.base))
            .header("Accept", "application/json")
            .form(form)
            .send()
            .await
            .map_err(|e| AuthError::Network(e.to_string()))?
            .json::<Value>()
            .await
            .map_err(|e| AuthError::Network(e.to_string()))
    }

    /// Passo 1: pede o código que a pessoa vai digitar no GitHub.
    pub async fn start(&self) -> Result<DeviceCode, AuthError> {
        let body = self
            .post("/login/device/code", &[("client_id", &self.client_id)])
            .await?;
        if let Some(err) = body.get("error").and_then(Value::as_str) {
            return Err(map_error(err));
        }
        serde_json::from_value(body)
            .map_err(|e| AuthError::Network(format!("resposta inesperada: {e}")))
    }

    /// Passo 2: espera a pessoa autorizar (respeitando o intervalo e o prazo do código).
    pub async fn wait_for_token(&self, code: &DeviceCode) -> Result<StoredToken, AuthError> {
        let deadline = now() + code.expires_in;
        let mut interval = code.interval;
        loop {
            tokio::time::sleep(Duration::from_secs(interval)).await;
            if now() > deadline {
                return Err(AuthError::Expired);
            }
            let body = self
                .post(
                    "/login/oauth/access_token",
                    &[
                        ("client_id", &self.client_id),
                        ("device_code", &code.device_code),
                        ("grant_type", DEVICE_GRANT),
                    ],
                )
                .await?;
            match body.get("error").and_then(Value::as_str) {
                None => {
                    return token_from(&body, now())
                        .ok_or_else(|| AuthError::Network("resposta sem token".into()))
                }
                Some("authorization_pending") => {}
                Some("slow_down") => {
                    interval = body
                        .get("interval")
                        .and_then(Value::as_u64)
                        .unwrap_or(interval + 5);
                }
                Some(other) => return Err(map_error(other)),
            }
        }
    }

    /// Troca o refresh por um token novo (e um refresh novo: guardar os dois).
    pub async fn refresh(&self, refresh_token: &str) -> Result<StoredToken, AuthError> {
        let body = self
            .post(
                "/login/oauth/access_token",
                &[
                    ("client_id", &self.client_id),
                    ("grant_type", "refresh_token"),
                    ("refresh_token", refresh_token),
                ],
            )
            .await?;
        if let Some(err) = body.get("error").and_then(Value::as_str) {
            return Err(map_error(err));
        }
        token_from(&body, now()).ok_or_else(|| AuthError::Network("resposta sem token".into()))
    }
}

fn map_error(code: &str) -> AuthError {
    match code {
        "access_denied" => AuthError::Denied,
        "expired_token" | "token_expired" | "incorrect_device_code" | "bad_verification_code" => {
            AuthError::Expired
        }
        "bad_refresh_token" => AuthError::NeedsLogin,
        "incorrect_client_credentials" | "device_flow_disabled" | "unsupported_grant_type" => {
            AuthError::Config(code.to_string())
        }
        other => AuthError::Network(other.to_string()),
    }
}

// ---------- onde o token fica ----------

pub trait TokenStore: Send + Sync {
    fn load(&self) -> Option<StoredToken>;
    fn save(&self, token: &StoredToken) -> Result<(), String>;
    fn clear(&self);
}

/// Keychain do macOS (Credential Manager no Windows, Secret Service no Linux).
pub struct KeychainStore;

impl KeychainStore {
    fn entry() -> Option<keyring::Entry> {
        keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT).ok()
    }
}

impl TokenStore for KeychainStore {
    fn load(&self) -> Option<StoredToken> {
        serde_json::from_str(&Self::entry()?.get_password().ok()?).ok()
    }
    fn save(&self, token: &StoredToken) -> Result<(), String> {
        let json = serde_json::to_string(token).map_err(|e| e.to_string())?;
        Self::entry()
            .ok_or("Keychain indisponível")?
            .set_password(&json)
            .map_err(|e| e.to_string())
    }
    fn clear(&self) {
        if let Some(entry) = Self::entry() {
            let _ = entry.delete_credential();
        }
    }
}

/// Token pronto para usar: renova se estiver para vencer; se não der, apaga e devolve None
/// (o app volta para o `gh`, se houver).
pub async fn usable_token(client: Option<&Client>, store: &dyn TokenStore) -> Option<String> {
    let token = store.load()?;
    let now = now();
    if !needs_refresh(&token, now) {
        return Some(token.access_token);
    }
    if let (Some(client), true) = (client, can_refresh(&token, now)) {
        match client
            .refresh(token.refresh_token.as_deref().unwrap_or_default())
            .await
        {
            Ok(fresh) => {
                if let Err(e) = store.save(&fresh) {
                    tracing::warn!(%e, "não foi possível guardar o token renovado");
                }
                return Some(fresh.access_token);
            }
            Err(AuthError::Network(e)) => {
                // Sem rede: tenta de novo na próxima consulta, sem apagar o login.
                tracing::warn!(%e, "falha ao renovar o token do GitHub");
                return None;
            }
            Err(_) => {}
        }
    }
    store.clear();
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::State, routing::post, Form, Json, Router};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// GitHub de mentira: o comportamento depende do client_id usado no teste.
    #[derive(Clone, Default)]
    struct Fake {
        polls: Arc<Mutex<u32>>,
        seen: Arc<Mutex<Vec<HashMap<String, String>>>>,
    }

    async fn device_code(Form(f): Form<HashMap<String, String>>) -> Json<Value> {
        if f.get("client_id").map(String::as_str) == Some("desligado") {
            return Json(json!({"error": "device_flow_disabled"}));
        }
        Json(json!({"device_code": "dc-1", "user_code": "WDJB-MJHT",
                    "verification_uri": "https://github.com/login/device", "expires_in": 900, "interval": 0}))
    }

    async fn access_token(
        State(fake): State<Fake>,
        Form(f): Form<HashMap<String, String>>,
    ) -> Json<Value> {
        fake.seen.lock().unwrap().push(f.clone());
        let ok = json!({"access_token": "ghu_novo", "expires_in": 28800, "refresh_token": "ghr_novo",
                        "refresh_token_expires_in": 15897600, "token_type": "bearer", "scope": ""});
        if f.get("grant_type").map(String::as_str) == Some("refresh_token") {
            return Json(
                if f.get("refresh_token").map(String::as_str) == Some("ghr_valido") {
                    ok
                } else {
                    json!({"error": "bad_refresh_token"})
                },
            );
        }
        let n = {
            let mut p = fake.polls.lock().unwrap();
            *p += 1;
            *p
        };
        Json(
            match f.get("client_id").map(String::as_str).unwrap_or_default() {
                "nega" => json!({"error": "access_denied"}),
                "expira" => json!({"error": "token_expired"}),
                _ => match n {
                    1 => json!({"error": "authorization_pending"}),
                    2 => json!({"error": "slow_down", "interval": 0}),
                    _ => ok,
                },
            },
        )
    }

    async fn fake_github() -> (String, Fake) {
        let fake = Fake::default();
        let app = Router::new()
            .route("/login/device/code", post(device_code))
            .route("/login/oauth/access_token", post(access_token))
            .with_state(fake.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}"), fake)
    }

    #[derive(Default)]
    struct MemoryStore(Mutex<Option<StoredToken>>);
    impl TokenStore for MemoryStore {
        fn load(&self) -> Option<StoredToken> {
            self.0.lock().unwrap().clone()
        }
        fn save(&self, t: &StoredToken) -> Result<(), String> {
            *self.0.lock().unwrap() = Some(t.clone());
            Ok(())
        }
        fn clear(&self) {
            *self.0.lock().unwrap() = None;
        }
    }

    #[tokio::test]
    async fn fluxo_completo_com_pendente_e_slow_down() {
        let (base, fake) = fake_github().await;
        let client = Client::new(&base, "Iv23liTeste").unwrap();
        let code = client.start().await.unwrap();
        assert_eq!(code.user_code, "WDJB-MJHT");
        let token = client.wait_for_token(&code).await.unwrap();
        assert_eq!(token.access_token, "ghu_novo");
        assert_eq!(token.refresh_token.as_deref(), Some("ghr_novo"));
        assert!(token.expires_at.unwrap() > now() + 28_000);
        assert_eq!(
            *fake.polls.lock().unwrap(),
            3,
            "pendente, slow_down e sucesso"
        );
        let seen = fake.seen.lock().unwrap();
        assert!(
            seen.iter().all(|f| !f.contains_key("client_secret")),
            "nunca manda segredo"
        );
        assert_eq!(
            seen[0].get("grant_type").map(String::as_str),
            Some(DEVICE_GRANT)
        );
    }

    #[tokio::test]
    async fn cancelado_ou_expirado() {
        let (base, _) = fake_github().await;
        let code = DeviceCode {
            device_code: "dc".into(),
            user_code: "X".into(),
            verification_uri: "https://github.com/login/device".into(),
            expires_in: 900,
            interval: 0,
        };
        let nega = Client::new(&base, "nega").unwrap();
        assert_eq!(nega.wait_for_token(&code).await, Err(AuthError::Denied));
        let expira = Client::new(&base, "expira").unwrap();
        assert_eq!(expira.wait_for_token(&code).await, Err(AuthError::Expired));
    }

    #[tokio::test]
    async fn device_flow_desligado_no_app_e_erro_de_config() {
        let (base, _) = fake_github().await;
        let client = Client::new(&base, "desligado").unwrap();
        assert!(
            matches!(client.start().await, Err(AuthError::Config(c)) if c == "device_flow_disabled")
        );
    }

    #[test]
    fn client_id_invalido_nem_comeca() {
        assert!(Client::new(GITHUB_WEB, "").is_err());
        assert!(Client::new(GITHUB_WEB, "tem espaço").is_err());
        assert!(Client::new(GITHUB_WEB, "x/../y").is_err());
        assert!(valid_client_id("Iv23liAbC1.2-3_4"));
    }

    #[tokio::test]
    async fn token_valido_e_usado_sem_renovar() {
        let store = MemoryStore::default();
        let t = StoredToken {
            access_token: "ghu_a".into(),
            refresh_token: None,
            expires_at: None,
            refresh_expires_at: None,
        };
        store.save(&t).unwrap();
        assert_eq!(
            usable_token(None, &store).await.as_deref(),
            Some("ghu_a"),
            "sem expiração: usa direto"
        );
    }

    #[tokio::test]
    async fn token_vencendo_e_renovado_e_guardado() {
        let (base, _) = fake_github().await;
        let client = Client::new(&base, "Iv23liTeste").unwrap();
        let store = MemoryStore::default();
        store
            .save(&StoredToken {
                access_token: "ghu_velho".into(),
                refresh_token: Some("ghr_valido".into()),
                expires_at: Some(now() + 10),
                refresh_expires_at: Some(now() + 1000),
            })
            .unwrap();
        assert_eq!(
            usable_token(Some(&client), &store).await.as_deref(),
            Some("ghu_novo")
        );
        assert_eq!(
            store.load().unwrap().refresh_token.as_deref(),
            Some("ghr_novo"),
            "guarda o refresh novo"
        );
    }

    #[tokio::test]
    async fn refresh_invalido_ou_vencido_apaga_o_login() {
        let (base, _) = fake_github().await;
        let client = Client::new(&base, "Iv23liTeste").unwrap();
        let store = MemoryStore::default();
        let vencendo = |refresh: &str, refresh_at: u64| StoredToken {
            access_token: "ghu_velho".into(),
            refresh_token: Some(refresh.into()),
            expires_at: Some(now()),
            refresh_expires_at: Some(refresh_at),
        };
        store.save(&vencendo("ghr_ruim", now() + 1000)).unwrap();
        assert_eq!(usable_token(Some(&client), &store).await, None);
        assert!(store.load().is_none(), "refresh recusado: apaga");

        store.save(&vencendo("ghr_valido", now() - 1)).unwrap();
        assert_eq!(usable_token(Some(&client), &store).await, None);
        assert!(
            store.load().is_none(),
            "refresh vencido: apaga sem chamar o GitHub"
        );
    }

    #[test]
    fn le_a_resposta_de_sucesso() {
        let t = token_from(
            &json!({"access_token": "ghu_x", "token_type": "bearer", "scope": ""}),
            100,
        )
        .unwrap();
        assert_eq!(
            t,
            StoredToken {
                access_token: "ghu_x".into(),
                refresh_token: None,
                expires_at: None,
                refresh_expires_at: None
            }
        );
        assert!(token_from(&json!({"error": "x"}), 0).is_none());
        let t = token_from(&json!({"access_token": "a", "expires_in": 28800, "refresh_token": "r", "refresh_token_expires_in": 10}), 100).unwrap();
        assert_eq!(
            (t.expires_at, t.refresh_expires_at),
            (Some(28900), Some(110))
        );
        assert!(needs_refresh(&t, 28900 - 299));
        assert!(!needs_refresh(&t, 28900 - 301));
    }
}
