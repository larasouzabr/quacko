use std::path::Path;

use super::*;
use serde_json::{json, Map, Value};

const QUACKO_MARKERS: [&str; 2] = ["/.claude/hooks/notify.sh", "quacko hook"];

const CLAUDE_HOOKS: [(&str, &str); 3] = [
    ("UserPromptSubmit", "--from claude"),
    (
        "Stop",
        "--from claude --sound /System/Library/Sounds/Glass.aiff",
    ),
    (
        "Notification",
        "--from claude --sound /System/Library/Sounds/Ping.aiff",
    ),
];

const CURSOR_HOOKS: [(&str, &str); 2] = [
    ("beforeSubmitPrompt", "--from cursor"),
    (
        "stop",
        "--from cursor --sound /System/Library/Sounds/Glass.aiff",
    ),
];

pub(super) fn is_quacko(command: &str) -> bool {
    QUACKO_MARKERS.iter().any(|m| command.contains(m))
}

fn shell_quote(path: &str) -> String {
    let safe = path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c));
    if safe {
        path.to_string()
    } else {
        format!("'{}'", path.replace('\'', r"'\''"))
    }
}

pub(super) fn command_for(notify: &Path, args: &str) -> String {
    format!("{} {args}", shell_quote(&notify.to_string_lossy()))
}

fn events(tool: Tool) -> &'static [(&'static str, &'static str)] {
    match tool {
        Tool::Claude => &CLAUDE_HOOKS,
        Tool::Cursor => &CURSOR_HOOKS,
    }
}

pub(super) fn commands_in(tool: Tool, entries: &[Value]) -> Vec<String> {
    let as_str = |v: &Value| v.get("command").and_then(Value::as_str).map(String::from);
    match tool {
        Tool::Claude => entries
            .iter()
            .filter_map(|g| g.get("hooks").and_then(Value::as_array))
            .flatten()
            .filter_map(as_str)
            .collect(),
        Tool::Cursor => entries.iter().filter_map(as_str).collect(),
    }
}

pub fn is_connected(tool: Tool, doc: &Value) -> bool {
    let flag = match tool {
        Tool::Claude => "--from claude",
        Tool::Cursor => "--from cursor",
    };
    events(tool).iter().all(|(event, _)| {
        doc.get("hooks")
            .and_then(|h| h.get(*event))
            .and_then(Value::as_array)
            .is_some_and(|entries| {
                commands_in(tool, entries)
                    .iter()
                    .any(|c| is_quacko(c) && c.contains(flag))
            })
    })
}

fn strip_quacko(tool: Tool, entries: &mut Vec<Value>) {
    match tool {
        Tool::Claude => {
            entries.retain_mut(|group| {
                let Some(hooks) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                    return true;
                };
                let before = hooks.len();
                hooks.retain(|h| {
                    !h.get("command")
                        .and_then(Value::as_str)
                        .is_some_and(is_quacko)
                });
                !(hooks.is_empty() && before > 0)
            });
        }
        Tool::Cursor => entries.retain(|h| {
            !h.get("command")
                .and_then(Value::as_str)
                .is_some_and(is_quacko)
        }),
    }
}

fn new_entry(tool: Tool, command: String) -> Value {
    match tool {
        Tool::Claude => json!({ "hooks": [{ "type": "command", "command": command }] }),
        Tool::Cursor => json!({ "command": command }),
    }
}

pub fn apply(
    tool: Tool,
    doc: &mut Value,
    notify: &Path,
    connect: bool,
) -> Result<(), ManualReason> {
    let mut work = doc.clone();
    apply_in_place(tool, &mut work, notify, connect)?;
    *doc = work;
    Ok(())
}

fn apply_in_place(
    tool: Tool,
    doc: &mut Value,
    notify: &Path,
    connect: bool,
) -> Result<(), ManualReason> {
    let root = doc.as_object_mut().ok_or(ManualReason::Invalid)?;
    if tool == Tool::Cursor && connect && !root.contains_key("version") {
        root.insert("version".into(), json!(1));
    }
    if !root.contains_key("hooks") {
        if !connect {
            return Ok(());
        }
        root.insert("hooks".into(), Value::Object(Map::new()));
    }
    let hooks = root
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or(ManualReason::Invalid)?;

    for (event, args) in events(tool) {
        if !hooks.contains_key(*event) {
            if !connect {
                continue;
            }
            hooks.insert((*event).into(), Value::Array(Vec::new()));
        }
        let entries = hooks
            .get_mut(*event)
            .and_then(Value::as_array_mut)
            .ok_or(ManualReason::Invalid)?;
        strip_quacko(tool, entries);
        if connect {
            entries.push(new_entry(tool, command_for(notify, args)));
        }
    }
    if !connect {
        let ours: Vec<&str> = events(tool).iter().map(|(e, _)| *e).collect();
        hooks.retain(|k, v| {
            !(ours.contains(&k.as_str()) && v.as_array().is_some_and(Vec::is_empty))
        });
        if hooks.is_empty() {
            root.remove("hooks");
        }
    }
    Ok(())
}

pub fn snippet(tool: Tool, notify: &Path) -> String {
    let mut doc = json!({});
    let _ = apply(tool, &mut doc, notify, true);
    serde_json::to_string_pretty(&doc).unwrap_or_default()
}
