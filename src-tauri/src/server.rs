use std::net::{Ipv4Addr, SocketAddr, TcpListener as StdTcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;

use crate::model::{IdGen, Notification};

pub const DEFAULT_PORT: u16 = 4747;
pub const MAX_BODY_BYTES: usize = 4096;
pub const RATE_LIMIT_PER_SEC: u32 = 10;

pub type Deliver = Arc<dyn Fn(Notification) + Send + Sync>;

#[derive(Clone)]
pub struct ServerState {
    dnd: Arc<AtomicBool>,
    deliver: Deliver,
    ids: IdGen,
    limiter: Arc<Mutex<RateLimiter>>,
}

impl ServerState {
    pub fn new(dnd: Arc<AtomicBool>, ids: IdGen, deliver: Deliver) -> Self {
        Self {
            dnd,
            deliver,
            ids,
            limiter: Arc::new(Mutex::new(RateLimiter::new(RATE_LIMIT_PER_SEC))),
        }
    }
}

pub fn resolve_port(env_value: Option<&str>, configured: u16) -> u16 {
    env_value
        .and_then(|v| v.trim().parse::<u16>().ok())
        .filter(|p| *p != 0)
        .unwrap_or(configured)
}

pub fn bind(port: u16) -> std::io::Result<StdTcpListener> {
    let listener = StdTcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

pub async fn serve(listener: StdTcpListener, state: ServerState) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::from_std(listener)?;
    axum::serve(listener, router(state)).await
}

pub fn router(state: ServerState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/notify", post(notify))
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state)
}

async fn guard(State(state): State<ServerState>, req: Request, next: Next) -> Response {
    if req.headers().contains_key(header::ORIGIN) {
        return error(
            StatusCode::FORBIDDEN,
            "requisições vindas de navegador não são aceitas",
        );
    }
    if !host_is_local(req.headers()) {
        return error(StatusCode::FORBIDDEN, "host não permitido");
    }
    let allowed = state
        .limiter
        .lock()
        .map(|mut l| l.allow(Instant::now()))
        .unwrap_or(true);
    if !allowed {
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            "muitas requisições; tente de novo em 1 s",
        );
    }
    next.run(req).await
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "app": "quacko", "version": env!("CARGO_PKG_VERSION") }))
}

async fn notify(State(state): State<ServerState>, headers: HeaderMap, body: Body) -> Response {
    if !is_json(&headers) {
        return error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "use o cabeçalho Content-Type: application/json",
        );
    }
    let declared_len = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());
    if declared_len.is_some_and(|len| len > MAX_BODY_BYTES) {
        return too_large();
    }
    let bytes = match to_bytes(body, MAX_BODY_BYTES).await {
        Ok(b) => b,
        Err(e) if is_length_limit(&e) => return too_large(),
        Err(_) => return error(StatusCode::BAD_REQUEST, "não foi possível ler o corpo"),
    };

    let mut notification = match Notification::from_json(&bytes) {
        Ok(n) => n,
        Err(e) => return error(StatusCode::BAD_REQUEST, &e.0),
    };

    if state.dnd.load(Ordering::Relaxed) {
        tracing::info!(source = ?notification.source, status = ?notification.status, "não perturbe: aviso descartado");
        return (StatusCode::ACCEPTED, Json(json!({ "delivered": false }))).into_response();
    }

    notification.id = state.ids.next();
    tracing::info!(id = notification.id, source = ?notification.source, status = ?notification.status, "aviso recebido");
    (state.deliver)(notification);
    (StatusCode::ACCEPTED, Json(json!({ "delivered": true }))).into_response()
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn too_large() -> Response {
    error(StatusCode::PAYLOAD_TOO_LARGE, "corpo maior que 4 KB")
}

fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
}

fn host_is_local(headers: &HeaderMap) -> bool {
    let Some(host) = headers.get(header::HOST) else {
        return true;
    };
    let Ok(host) = host.to_str() else {
        return false;
    };
    let name = if let Some(rest) = host.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        host.split(':').next().unwrap_or("")
    };
    matches!(
        name.to_ascii_lowercase().as_str(),
        "127.0.0.1" | "localhost" | "::1"
    )
}

fn is_length_limit(err: &axum::Error) -> bool {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(e) = source {
        if e.is::<http_body_util::LengthLimitError>() {
            return true;
        }
        source = e.source();
    }
    false
}

pub struct RateLimiter {
    limit: u32,
    window_start: Option<Instant>,
    count: u32,
}

impl RateLimiter {
    pub fn new(limit: u32) -> Self {
        Self {
            limit,
            window_start: None,
            count: 0,
        }
    }

    pub fn allow(&mut self, now: Instant) -> bool {
        match self.window_start {
            Some(start) if now.duration_since(start) < Duration::from_secs(1) => {
                if self.count >= self.limit {
                    return false;
                }
                self.count += 1;
                true
            }
            _ => {
                self.window_start = Some(now);
                self.count = 1;
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    struct Harness {
        state: ServerState,
        dnd: Arc<AtomicBool>,
        delivered: Arc<Mutex<Vec<Notification>>>,
    }

    fn harness() -> Harness {
        let dnd = Arc::new(AtomicBool::new(false));
        let delivered = Arc::new(Mutex::new(Vec::new()));
        let sink = delivered.clone();
        let deliver: Deliver = Arc::new(move |n| sink.lock().unwrap().push(n));
        Harness {
            state: ServerState::new(dnd.clone(), IdGen::default(), deliver),
            dnd,
            delivered,
        }
    }

    fn post_notify() -> axum::http::request::Builder {
        HttpRequest::builder()
            .method("POST")
            .uri("/notify")
            .header(header::HOST, "localhost:4747")
    }

    async fn send(state: &ServerState, req: HttpRequest<Body>) -> (StatusCode, serde_json::Value) {
        let res = router(state.clone()).oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    fn json_req(body: &str) -> HttpRequest<Body> {
        post_notify()
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    const VALID: &str = r#"{"source":"claude","status":"ok"}"#;

    #[tokio::test]
    async fn health_responde_app_e_versao() {
        let h = harness();
        let req = HttpRequest::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(&h.state, req).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["app"], "quacko");
        assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn notify_valido_e_aceito_e_entregue() {
        let h = harness();
        let (status, body) = send(&h.state, json_req(VALID)).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body["delivered"], true);
        let delivered = h.delivered.lock().unwrap();
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].id, 1);
    }

    #[tokio::test]
    async fn ids_sao_crescentes() {
        let h = harness();
        send(&h.state, json_req(VALID)).await;
        send(&h.state, json_req(VALID)).await;
        let ids: Vec<u64> = h.delivered.lock().unwrap().iter().map(|n| n.id).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[tokio::test]
    async fn content_type_com_charset_e_aceito() {
        let h = harness();
        let req = post_notify()
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .body(Body::from(VALID))
            .unwrap();
        assert_eq!(send(&h.state, req).await.0, StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn sem_content_type_json_da_415() {
        let h = harness();
        let sem = post_notify().body(Body::from(VALID)).unwrap();
        assert_eq!(
            send(&h.state, sem).await.0,
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
        for ct in [
            "text/plain",
            "application/x-www-form-urlencoded",
            "multipart/form-data",
        ] {
            let req = post_notify()
                .header(header::CONTENT_TYPE, ct)
                .body(Body::from(VALID))
                .unwrap();
            assert_eq!(
                send(&h.state, req).await.0,
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "{ct}"
            );
        }
        assert!(h.delivered.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn origin_da_403_mesmo_com_payload_valido() {
        let h = harness();
        let req = post_notify()
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, "http://evil.test")
            .body(Body::from(VALID))
            .unwrap();
        assert_eq!(send(&h.state, req).await.0, StatusCode::FORBIDDEN);
        assert!(h.delivered.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn origin_tambem_bloqueia_health() {
        let h = harness();
        let req = HttpRequest::builder()
            .uri("/health")
            .header(header::ORIGIN, "http://evil.test")
            .body(Body::empty())
            .unwrap();
        assert_eq!(send(&h.state, req).await.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn host_externo_da_403() {
        let h = harness();
        let req = HttpRequest::builder()
            .method("POST")
            .uri("/notify")
            .header(header::HOST, "evil.test:4747")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(VALID))
            .unwrap();
        assert_eq!(send(&h.state, req).await.0, StatusCode::FORBIDDEN);
    }

    #[test]
    fn hosts_locais_aceitos() {
        for host in [
            "localhost",
            "localhost:4747",
            "127.0.0.1:4747",
            "[::1]:4747",
            "LOCALHOST",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(header::HOST, host.parse().unwrap());
            assert!(host_is_local(&headers), "{host}");
        }
        for host in [
            "evil.test",
            "127.0.0.1.evil.test",
            "localhost.evil.test:4747",
            "[::2]:1",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(header::HOST, host.parse().unwrap());
            assert!(!host_is_local(&headers), "{host}");
        }
    }

    #[tokio::test]
    async fn payload_invalido_da_400_com_mensagem() {
        let h = harness();
        let (status, body) = send(&h.state, json_req(r#"{"source":"x","status":"ok"}"#)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            body["error"].as_str().unwrap().contains("payload inválido"),
            "{body}"
        );
        let (status, _) = send(&h.state, json_req("não é json")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn corpo_grande_da_413() {
        let h = harness();
        let big = format!(
            r#"{{"source":"claude","status":"ok","message":"{}"}}"#,
            "a".repeat(5000)
        );
        assert_eq!(
            send(&h.state, json_req(&big)).await.0,
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }

    #[tokio::test]
    async fn corpo_grande_sem_content_length_da_413() {
        let h = harness();
        let chunks: Vec<Result<String, std::io::Error>> =
            (0..10).map(|_| Ok("a".repeat(1000))).collect();
        let req = post_notify()
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from_stream(futures_util::stream::iter(chunks)))
            .unwrap();
        assert_eq!(send(&h.state, req).await.0, StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn acima_de_10_por_segundo_da_429() {
        let h = harness();
        for i in 0..RATE_LIMIT_PER_SEC {
            assert_eq!(
                send(&h.state, json_req(VALID)).await.0,
                StatusCode::ACCEPTED,
                "req {i}"
            );
        }
        assert_eq!(
            send(&h.state, json_req(VALID)).await.0,
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[test]
    fn rate_limiter_reabre_apos_1_segundo() {
        let mut l = RateLimiter::new(2);
        let t0 = Instant::now();
        assert!(l.allow(t0));
        assert!(l.allow(t0 + Duration::from_millis(100)));
        assert!(!l.allow(t0 + Duration::from_millis(999)));
        assert!(l.allow(t0 + Duration::from_millis(1000)));
    }

    #[tokio::test]
    async fn nao_perturbe_responde_202_sem_entregar() {
        let h = harness();
        h.dnd.store(true, Ordering::Relaxed);
        let (status, body) = send(&h.state, json_req(VALID)).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body, json!({ "delivered": false }));
        assert!(h.delivered.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn sem_cabecalhos_cors() {
        let h = harness();
        let res = router(h.state.clone())
            .oneshot(json_req(VALID))
            .await
            .unwrap();
        assert!(res
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());
    }

    #[test]
    fn porta_ambiente_vence_a_config() {
        assert_eq!(resolve_port(None, DEFAULT_PORT), DEFAULT_PORT);
        assert_eq!(resolve_port(None, 6000), 6000);
        assert_eq!(resolve_port(Some("5000"), 6000), 5000);
        assert_eq!(resolve_port(Some(" 5001 "), 6000), 5001);
        for bad in ["", "abc", "0", "70000", "-1"] {
            assert_eq!(resolve_port(Some(bad), 6000), 6000, "{bad:?}");
        }
    }
}
