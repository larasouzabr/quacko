use serde_json::{Map, Value};

use crate::args::Args;
use crate::prompts::Store;
use crate::text::{basename, field, one_line, quoted};

const CLAUDE_EVENTS: [&str; 5] = [
    "Stop",
    "StopFailure",
    "SubagentStop",
    "Notification",
    "UserPromptSubmit",
];
const CURSOR_EVENTS: [&str; 2] = ["stop", "beforeSubmitPrompt"];
const INFO_NOTIFICATIONS: [&str; 5] = [
    "auth_success",
    "elicitation_complete",
    "elicitation_response",
    "plugin_notification",
    "quota_",
];

pub const CURSOR_CONTINUE: &str = r#"{"continue":true}"#;
pub const CURSOR_EMPTY: &str = "{}";

#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    pub reply: Option<&'static str>,
    pub effects: bool,
    pub status: String,
    pub sound: String,
    pub payload: Option<Value>,
}

impl Plan {
    fn quiet(reply: Option<&'static str>) -> Self {
        Self {
            reply,
            ..Self::default()
        }
    }
}

pub fn plan(mut a: Args, input: Option<&Value>, store: &Store) -> Plan {
    let j = |path: &str| field(input, path);
    let event = j("hook_event_name");
    if a.from.is_empty() {
        if CLAUDE_EVENTS.contains(&event.as_str()) {
            a.from = "claude".into();
        } else if CURSOR_EVENTS.contains(&event.as_str()) {
            a.from = "cursor".into();
        } else if a.source == "claude" {
            a.from = "claude".into();
        }
    }

    let mut reply = None;
    match a.from.as_str() {
        "claude" => {
            default(&mut a.source, "agent");
            default(&mut a.title, "Claude Code");
            let cwd = j("cwd");
            if a.project.is_empty() && !cwd.is_empty() {
                a.project = basename(&cwd);
            }
            let file = store.file("claude", &j("session_id"));
            match event.as_str() {
                "UserPromptSubmit" => {
                    store.save(file.as_deref(), &one_line(&j("prompt")));
                    return Plan::quiet(None);
                }
                "Stop" | "SubagentStop" | "StopFailure" => {
                    default(
                        &mut a.status,
                        if event == "StopFailure" { "fail" } else { "ok" },
                    );
                    if a.message.is_empty() {
                        a.message = quoted(&store.take(file.as_deref()));
                    }
                }
                "Notification" => {
                    if a.status.is_empty() {
                        a.status = notification_status(&j("notification_type")).into();
                    }
                    if a.message.is_empty() {
                        a.message = one_line(&j("message"));
                    }
                }
                _ => {}
            }
        }
        "cursor" => {
            default(&mut a.source, "agent");
            default(&mut a.title, "Cursor");
            let root = j("workspace_roots.0");
            if a.project.is_empty() && !root.is_empty() {
                a.project = basename(&root);
            }
            let file = store.file("cursor", &j("conversation_id"));
            match event.as_str() {
                "beforeSubmitPrompt" => {
                    store.save(file.as_deref(), &one_line(&j("prompt")));
                    return Plan::quiet(Some(CURSOR_CONTINUE));
                }
                "stop" => {
                    reply = Some(CURSOR_EMPTY);
                    match j("status").as_str() {
                        "aborted" => {
                            store.discard(file.as_deref());
                            return Plan::quiet(reply);
                        }
                        "error" => default(&mut a.status, "fail"),
                        _ => default(&mut a.status, "ok"),
                    }
                    if a.message.is_empty() {
                        a.message = quoted(&store.take(file.as_deref()));
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }

    if !a.status.is_empty() {
        default(&mut a.source, "custom");
    }
    let payload = (!a.status.is_empty()).then(|| payload(&a));
    Plan {
        reply,
        effects: true,
        status: a.status,
        sound: a.sound,
        payload,
    }
}

fn default(value: &mut String, fallback: &str) {
    if value.is_empty() {
        *value = fallback.to_string();
    }
}

fn notification_status(kind: &str) -> &'static str {
    if kind == "agent_completed" {
        "ok"
    } else if INFO_NOTIFICATIONS
        .iter()
        .any(|k| kind == *k || (k.ends_with('_') && kind.starts_with(k)))
    {
        "info"
    } else {
        "attention"
    }
}

fn payload(a: &Args) -> Value {
    let mut map = Map::new();
    map.insert("source".into(), a.source.clone().into());
    map.insert("status".into(), a.status.clone().into());
    for (key, value) in [
        ("message", &a.message),
        ("title", &a.title),
        ("project", &a.project),
        ("url", &a.url),
    ] {
        if !value.is_empty() {
            map.insert(key.into(), value.clone().into());
        }
    }
    if a.sticky {
        map.insert("sticky".into(), true.into());
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::parse;
    use serde_json::json;

    fn run(store: &Store, input: Value, args: &[&str]) -> Plan {
        plan(
            parse(args.iter().map(|s| s.to_string())),
            Some(&input),
            store,
        )
    }

    #[test]
    fn claude_guarda_o_pedido_e_mostra_no_stop() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let s = json!({"session_id": "abc", "cwd": "/x/lagoa-api", "hook_event_name": "UserPromptSubmit", "prompt": "adicione\ttestes"});
        assert_eq!(run(&store, s, &[]), Plan::quiet(None));
        let p = run(
            &store,
            json!({"session_id": "abc", "cwd": "/x/lagoa-api", "hook_event_name": "Stop"}),
            &[],
        );
        assert_eq!(
            p.payload.unwrap(),
            json!({"source": "agent", "status": "ok", "message": "“adicione testes”", "title": "Claude Code", "project": "lagoa-api"})
        );
        assert!(p.effects);
    }

    #[test]
    fn tipos_de_notification() {
        assert_eq!(notification_status("permission_prompt"), "attention");
        assert_eq!(notification_status("agent_completed"), "ok");
        assert_eq!(notification_status("quota_warning"), "info");
        assert_eq!(notification_status("auth_success"), "info");
        assert_eq!(notification_status(""), "attention");
    }

    #[test]
    fn cursor_responde_e_aborted_nao_avisa() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let c = |extra: Value| {
            let mut v = json!({"conversation_id": "c9", "workspace_roots": ["/x/meu-app"]});
            v.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            v
        };
        let before = run(
            &store,
            c(json!({"hook_event_name": "beforeSubmitPrompt", "prompt": "refatore"})),
            &[],
        );
        assert_eq!(before.reply, Some(CURSOR_CONTINUE));
        assert!(!before.effects);
        let aborted = run(
            &store,
            c(json!({"hook_event_name": "stop", "status": "aborted"})),
            &[],
        );
        assert_eq!(
            (aborted.reply, aborted.payload, aborted.effects),
            (Some(CURSOR_EMPTY), None, false)
        );
        let error = run(
            &store,
            c(json!({"hook_event_name": "stop", "status": "error"})),
            &[],
        );
        assert_eq!(error.payload.unwrap()["status"], "fail");
    }

    #[test]
    fn sem_status_toca_mas_nao_avisa() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let p = plan(parse(["/x/Glass.aiff".to_string()]), None, &store);
        assert_eq!(
            (p.effects, p.payload, p.sound.as_str()),
            (true, None, "/x/Glass.aiff")
        );
    }
}
