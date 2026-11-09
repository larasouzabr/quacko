//! Modelo da notificação recebida em `POST /notify` e sua validação (seção 5 da spec).
//!
//! Regras de tolerância (decisão do Marco 2):
//! - `source`, `status` e JSON malformado: erro 400.
//! - Textos longos são **cortados** (com "…") em vez de rejeitados, para que um aviso
//!   real (ex.: título de PR comprido) não se perca.
//! - `url` com esquema que não seja http(s) é **descartada**; o resto do aviso segue.
//! - `ttl_ms` fora da faixa é ajustado para o limite mais próximo.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_MESSAGE_CHARS: usize = 140;
pub const MAX_TITLE_CHARS: usize = 60;
pub const MAX_PROJECT_CHARS: usize = 80;
pub const MAX_URL_BYTES: usize = 2048;

pub const TTL_MIN_MS: u32 = 2_000;
pub const TTL_MAX_MS: u32 = 30_000;
pub const TTL_DEFAULT_MS: u32 = 8_000;
pub const TTL_ATTENTION_MS: u32 = 15_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Claude Code. Mantido por compatibilidade; equivale a `agent` com título "Claude Code".
    Claude,
    /// Qualquer agente de IA (Cursor, Codex, Claude…); o nome da ferramenta vai no `title`.
    Agent,
    Ci,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Fail,
    Attention,
    Info,
}

impl Status {
    pub const ALL: [Status; 4] = [Status::Ok, Status::Fail, Status::Attention, Status::Info];

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Fail => "fail",
            Status::Attention => "attention",
            Status::Info => "info",
        }
    }
}

/// Ids crescentes compartilhados entre o servidor e o "Testar notificação" do menu.
#[derive(Clone, Default)]
pub struct IdGen(Arc<AtomicU64>);

impl IdGen {
    pub fn next(&self) -> u64 {
        self.0.fetch_add(1, Ordering::Relaxed) + 1
    }
}

/// Notificação já validada, pronta para o frontend.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Notification {
    /// Atribuído pelo servidor; 0 até lá.
    pub id: u64,
    pub source: Source,
    pub status: Status,
    pub message: Option<String>,
    pub title: Option<String>,
    pub project: Option<String>,
    pub url: Option<String>,
    pub ttl_ms: u32,
    /// Barra de progresso no cartão (hoje só o monitor de PRs usa; não vem pela API HTTP).
    pub progress: Option<Progress>,
    /// Fica na tela até a pessoa fechar no ×: não expira (ex.: CI que terminou).
    pub sticky: bool,
}

pub const MAX_PROGRESS_TOTAL: u32 = 10_000;

/// "3 de 12 checks": `done` e `failed` nunca passam de `total`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub done: u32,
    pub total: u32,
    pub failed: u32,
}

/// Payload como chega pela rede, antes da limpeza.
#[derive(Deserialize)]
struct RawNotification {
    source: Source,
    status: Status,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    ttl_ms: Option<i64>,
    /// Opcional: barra de progresso. Valores inválidos descartam só a barra.
    #[serde(default)]
    progress: Option<Value>,
    /// Opcional: só fecha no × (sem expirar).
    #[serde(default)]
    sticky: Option<bool>,
}

/// Aceita `{done, total, failed?}` com inteiros, `total` de 1 a 10000; o resto é ajustado.
fn clean_progress(raw: &Value) -> Option<Progress> {
    let int = |key: &str| raw.get(key).and_then(Value::as_u64);
    let total = int("total").filter(|t| (1..=MAX_PROGRESS_TOTAL as u64).contains(t))? as u32;
    let done = int("done")?.min(total as u64) as u32;
    let failed = int("failed").unwrap_or(0).min(done as u64) as u32;
    Some(Progress {
        done,
        total,
        failed,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidPayload(pub String);

impl std::fmt::Display for InvalidPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Notification {
    pub fn from_json(body: &[u8]) -> Result<Self, InvalidPayload> {
        let raw: RawNotification = serde_json::from_slice(body)
            .map_err(|e| InvalidPayload(format!("payload inválido: {e}")))?;

        Ok(Notification {
            id: 0,
            source: raw.source,
            status: raw.status,
            message: raw.message.and_then(|s| clean_text(&s, MAX_MESSAGE_CHARS)),
            title: raw.title.and_then(|s| clean_text(&s, MAX_TITLE_CHARS)),
            project: raw.project.and_then(|s| clean_text(&s, MAX_PROJECT_CHARS)),
            url: raw.url.and_then(|s| clean_url(&s)),
            ttl_ms: resolve_ttl(raw.ttl_ms, raw.status),
            progress: raw.progress.as_ref().and_then(clean_progress),
            sticky: raw.sticky.unwrap_or(false),
        })
    }
}

/// Troca caracteres de controle por espaço, apara e corta em `max` caracteres
/// (não bytes), terminando com "…" quando corta. Texto vazio vira `None`.
fn clean_text(input: &str, max: usize) -> Option<String> {
    let cleaned: String = input
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().count() <= max {
        return Some(trimmed.to_string());
    }
    let mut cut: String = trimmed.chars().take(max - 1).collect();
    cut.truncate(cut.trim_end().len());
    cut.push('…');
    Some(cut)
}

/// Aceita apenas `http://` ou `https://` com host, sem espaços nem caracteres de controle.
pub fn clean_url(input: &str) -> Option<String> {
    let url = input.trim();
    if url.len() > MAX_URL_BYTES || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let lower = url.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))?;
    if rest.is_empty() || rest.starts_with('/') {
        return None;
    }
    Some(url.to_string())
}

fn resolve_ttl(ttl: Option<i64>, status: Status) -> u32 {
    match ttl {
        Some(v) => v.clamp(TTL_MIN_MS as i64, TTL_MAX_MS as i64) as u32,
        None if status == Status::Attention => TTL_ATTENTION_MS,
        None => TTL_DEFAULT_MS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Result<Notification, InvalidPayload> {
        Notification::from_json(json.as_bytes())
    }

    fn ok(json: &str) -> Notification {
        parse(json).expect("payload deveria ser válido")
    }

    #[test]
    fn minimo_valido() {
        let n = ok(r#"{"source":"claude","status":"ok"}"#);
        assert_eq!(n.source, Source::Claude);
        assert_eq!(n.status, Status::Ok);
        assert_eq!(n.message, None);
        assert_eq!(n.title, None);
        assert_eq!(n.project, None);
        assert_eq!(n.url, None);
        assert_eq!(n.ttl_ms, TTL_DEFAULT_MS);
    }

    #[test]
    fn completo_valido() {
        let n = ok(r#"{
            "source": "ci", "status": "fail",
            "message": "PR #1: checks falharam", "title": "CI",
            "project": "lagoa-api",
            "url": "https://github.com/org/repo/pull/1234", "ttl_ms": 5000
        }"#);
        assert_eq!(n.source, Source::Ci);
        assert_eq!(n.status, Status::Fail);
        assert_eq!(n.message.as_deref(), Some("PR #1: checks falharam"));
        assert_eq!(n.title.as_deref(), Some("CI"));
        assert_eq!(n.project.as_deref(), Some("lagoa-api"));
        assert_eq!(
            n.url.as_deref(),
            Some("https://github.com/org/repo/pull/1234")
        );
        assert_eq!(n.ttl_ms, 5000);
    }

    #[test]
    fn campos_extras_sao_ignorados() {
        ok(r#"{"source":"custom","status":"info","outro":123}"#);
    }

    #[test]
    fn nulos_e_vazios_viram_none() {
        let n =
            ok(r#"{"source":"claude","status":"ok","message":null,"title":"   ","project":""}"#);
        assert_eq!((n.message, n.title, n.project), (None, None, None));
    }

    #[test]
    fn todas_as_origens_validas() {
        for (txt, src) in [
            ("claude", Source::Claude),
            ("agent", Source::Agent),
            ("ci", Source::Ci),
            ("custom", Source::Custom),
        ] {
            let n = ok(&format!(r#"{{"source":"{txt}","status":"ok"}}"#));
            assert_eq!(n.source, src);
        }
    }

    #[test]
    fn enums_invalidos() {
        assert!(parse(r#"{"source":"x","status":"ok"}"#).is_err());
        assert!(parse(r#"{"source":"claude","status":"done"}"#).is_err());
        assert!(parse(r#"{"source":"Claude","status":"ok"}"#).is_err());
        assert!(parse(r#"{"source":1,"status":"ok"}"#).is_err());
    }

    #[test]
    fn campos_obrigatorios() {
        assert!(parse(r#"{"status":"ok"}"#).is_err());
        assert!(parse(r#"{"source":"claude"}"#).is_err());
    }

    #[test]
    fn json_malformado_ou_nao_objeto() {
        for body in ["", "{", "nada", "[]", "\"ok\"", "null"] {
            assert!(parse(body).is_err(), "deveria rejeitar {body:?}");
        }
    }

    #[test]
    fn mensagem_de_erro_explica_o_campo() {
        let err = parse(r#"{"source":"x","status":"ok"}"#).unwrap_err();
        assert!(err.0.contains("claude"), "{err}");
    }

    #[test]
    fn mensagem_longa_e_cortada_em_140() {
        let n = ok(&format!(
            r#"{{"source":"claude","status":"ok","message":"{}"}}"#,
            "a".repeat(500)
        ));
        let msg = n.message.unwrap();
        assert_eq!(msg.chars().count(), MAX_MESSAGE_CHARS);
        assert!(msg.ends_with('…'));
    }

    #[test]
    fn limite_exato_nao_corta() {
        let n = ok(&format!(
            r#"{{"source":"claude","status":"ok","message":"{}"}}"#,
            "a".repeat(140)
        ));
        assert_eq!(n.message.unwrap(), "a".repeat(140));
    }

    #[test]
    fn titulo_e_projeto_longos() {
        let n = ok(&format!(
            r#"{{"source":"claude","status":"ok","title":"{}","project":"{}"}}"#,
            "t".repeat(61),
            "p".repeat(81)
        ));
        assert_eq!(n.title.unwrap().chars().count(), MAX_TITLE_CHARS);
        assert_eq!(n.project.unwrap().chars().count(), MAX_PROJECT_CHARS);
    }

    #[test]
    fn corte_conta_caracteres_e_nao_bytes() {
        // "ã" ocupa 2 bytes; 140 deles cabem exatamente.
        let n = ok(&format!(
            r#"{{"source":"claude","status":"ok","message":"{}"}}"#,
            "ã".repeat(140)
        ));
        assert_eq!(n.message.unwrap().chars().count(), 140);
        let n = ok(&format!(
            r#"{{"source":"claude","status":"ok","message":"{}"}}"#,
            "🐱".repeat(200)
        ));
        assert_eq!(n.message.unwrap().chars().count(), 140);
    }

    #[test]
    fn caracteres_de_controle_viram_espaco() {
        let n = ok(r#"{"source":"claude","status":"ok","message":"linha 1\nlinha 2\u0007"}"#);
        assert_eq!(n.message.as_deref(), Some("linha 1 linha 2"));
    }

    #[test]
    fn url_com_esquema_invalido_e_descartada() {
        for url in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "ftp://exemplo.com",
            "data:text/html,oi",
            "vscode://abrir",
            "http://",
            "https:///caminho",
            "https://exemplo.com/com espaco",
            "exemplo.com",
        ] {
            let n = ok(&format!(
                r#"{{"source":"claude","status":"ok","url":"{url}"}}"#
            ));
            assert_eq!(n.url, None, "deveria descartar {url:?}");
            assert_eq!(n.status, Status::Ok, "o resto do aviso continua");
        }
    }

    #[test]
    fn url_http_e_https_aceitas() {
        for url in [
            "http://localhost:3000/x",
            "https://github.com/a/b/pull/1",
            "HTTPS://Exemplo.com",
        ] {
            let n = ok(&format!(
                r#"{{"source":"claude","status":"ok","url":"{url}"}}"#
            ));
            assert_eq!(n.url.as_deref(), Some(url));
        }
    }

    #[test]
    fn url_longa_demais_e_descartada() {
        let url = format!("https://exemplo.com/{}", "a".repeat(MAX_URL_BYTES));
        let n = ok(&format!(
            r#"{{"source":"claude","status":"ok","url":"{url}"}}"#
        ));
        assert_eq!(n.url, None);
    }

    #[test]
    fn ttl_padrao_por_status() {
        assert_eq!(
            ok(r#"{"source":"claude","status":"attention"}"#).ttl_ms,
            TTL_ATTENTION_MS
        );
        assert_eq!(
            ok(r#"{"source":"claude","status":"fail"}"#).ttl_ms,
            TTL_DEFAULT_MS
        );
        assert_eq!(
            ok(r#"{"source":"claude","status":"info"}"#).ttl_ms,
            TTL_DEFAULT_MS
        );
    }

    #[test]
    fn ttl_fora_da_faixa_e_ajustado() {
        assert_eq!(
            ok(r#"{"source":"claude","status":"ok","ttl_ms":10}"#).ttl_ms,
            TTL_MIN_MS
        );
        assert_eq!(
            ok(r#"{"source":"claude","status":"ok","ttl_ms":-5}"#).ttl_ms,
            TTL_MIN_MS
        );
        assert_eq!(
            ok(r#"{"source":"claude","status":"ok","ttl_ms":999999}"#).ttl_ms,
            TTL_MAX_MS
        );
        assert_eq!(
            ok(r#"{"source":"claude","status":"attention","ttl_ms":3000}"#).ttl_ms,
            3000
        );
    }

    #[test]
    fn ttl_nao_inteiro_e_rejeitado() {
        assert!(parse(r#"{"source":"claude","status":"ok","ttl_ms":"8000"}"#).is_err());
        assert!(parse(r#"{"source":"claude","status":"ok","ttl_ms":1.5}"#).is_err());
    }

    #[test]
    fn progresso_pela_api() {
        let n = ok(
            r#"{"source":"custom","status":"info","progress":{"done":3,"total":12,"failed":1}}"#,
        );
        assert_eq!(
            n.progress,
            Some(Progress {
                done: 3,
                total: 12,
                failed: 1
            })
        );
        let n = ok(r#"{"source":"custom","status":"info","progress":{"done":30,"total":12}}"#);
        assert_eq!(
            n.progress,
            Some(Progress {
                done: 12,
                total: 12,
                failed: 0
            }),
            "done passa do total: ajusta"
        );
        for bad in [
            r#"{"done":1,"total":0}"#,
            r#"{"done":1,"total":20000}"#,
            r#"{"done":-1,"total":3}"#,
            r#"{"done":"1","total":3}"#,
            r#"{"total":3}"#,
            r#"[1,2]"#,
        ] {
            let n = ok(&format!(
                r#"{{"source":"custom","status":"info","progress":{bad}}}"#
            ));
            assert_eq!(n.progress, None, "{bad}");
            assert_eq!(n.status, Status::Info, "o resto do aviso continua");
        }
        assert_eq!(ok(r#"{"source":"custom","status":"ok"}"#).progress, None);
    }

    #[test]
    fn sticky_pela_api() {
        assert!(ok(r#"{"source":"ci","status":"fail","sticky":true}"#).sticky);
        assert!(!ok(r#"{"source":"ci","status":"fail"}"#).sticky);
        assert!(parse(r#"{"source":"ci","status":"fail","sticky":"sim"}"#).is_err());
    }

    #[test]
    fn serializa_para_o_frontend_em_minusculas() {
        let n = ok(r#"{"source":"ci","status":"attention"}"#);
        let v = serde_json::to_value(&n).unwrap();
        assert_eq!(v["source"], "ci");
        assert_eq!(v["status"], "attention");
        assert_eq!(v["ttl_ms"], TTL_ATTENTION_MS);
    }
}
