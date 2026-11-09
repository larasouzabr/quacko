use std::path::{Path, PathBuf};

use super::*;
use serde_json::{json, Map, Value};

pub const HOOK_EXE_NAME: &str = "quacko-hook";
const QUACKO_MARKERS: [&str; 3] = ["/.claude/hooks/notify.sh", "quacko hook", HOOK_EXE_NAME];

const GLASS: &str = "/System/Library/Sounds/Glass.aiff";
const PING: &str = "/System/Library/Sounds/Ping.aiff";

const CLAUDE_HOOKS: [(&str, Option<&str>); 3] = [
    ("UserPromptSubmit", None),
    ("Stop", Some(GLASS)),
    ("Notification", Some(PING)),
];

const CURSOR_HOOKS: [(&str, Option<&str>); 2] =
    [("beforeSubmitPrompt", None), ("stop", Some(GLASS))];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hook {
    Script(PathBuf),
    Exe(PathBuf),
}

impl Hook {
    fn args(&self, tool: Tool, sound: Option<&str>) -> Vec<String> {
        let mut args = vec!["--from".to_string(), tool.flag_name().to_string()];
        if let (Hook::Script(_), Some(sound)) = (self, sound) {
            args.extend(["--sound".to_string(), sound.to_string()]);
        }
        args
    }

    fn entry(&self, tool: Tool, args: Vec<String>) -> Value {
        match (self, tool) {
            (Hook::Script(path), Tool::Claude) => {
                json!({ "hooks": [{ "type": "command", "command": command_for(path, &args.join(" ")) }] })
            }
            (Hook::Script(path), Tool::Cursor) => {
                json!({ "command": command_for(path, &args.join(" ")) })
            }
            (Hook::Exe(path), Tool::Claude) => {
                json!({ "hooks": [{ "type": "command", "command": path.to_string_lossy(), "args": args }] })
            }
            (Hook::Exe(path), Tool::Cursor) => json!({ "command": powershell_call(path, &args) }),
        }
    }
}

pub fn powershell_call(exe: &Path, args: &[String]) -> String {
    let quoted = format!("'{}'", exe.to_string_lossy().replace('\'', "''"));
    format!("& {quoted} {}", args.join(" "))
}

pub(super) fn is_quacko(command: &str) -> bool {
    let command = command.replace('\\', "/");
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

fn events(tool: Tool) -> &'static [(&'static str, Option<&'static str>)] {
    match tool {
        Tool::Claude => &CLAUDE_HOOKS,
        Tool::Cursor => &CURSOR_HOOKS,
    }
}

pub(super) fn commands_in(tool: Tool, entries: &[Value]) -> Vec<String> {
    let as_str = |v: &Value| {
        let command = v.get("command").and_then(Value::as_str)?;
        let args: Vec<&str> = v
            .get("args")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        Some(if args.is_empty() {
            command.to_string()
        } else {
            format!("{command} {}", args.join(" "))
        })
    };
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
    let flag = format!("--from {}", tool.flag_name());
    events(tool).iter().all(|(event, _)| {
        doc.get("hooks")
            .and_then(|h| h.get(*event))
            .and_then(Value::as_array)
            .is_some_and(|entries| {
                commands_in(tool, entries)
                    .iter()
                    .any(|c| is_quacko(c) && c.contains(&flag))
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

pub fn apply(tool: Tool, doc: &mut Value, hook: &Hook, connect: bool) -> Result<(), ManualReason> {
    let mut work = doc.clone();
    apply_in_place(tool, &mut work, hook, connect)?;
    *doc = work;
    Ok(())
}

fn apply_in_place(
    tool: Tool,
    doc: &mut Value,
    hook: &Hook,
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

    for (event, sound) in events(tool) {
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
            entries.push(hook.entry(tool, hook.args(tool, *sound)));
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

pub fn snippet(tool: Tool, hook: &Hook) -> String {
    let mut doc = json!({});
    let _ = apply(tool, &mut doc, hook, true);
    serde_json::to_string_pretty(&doc).unwrap_or_default()
}
