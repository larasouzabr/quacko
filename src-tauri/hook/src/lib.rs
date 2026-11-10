pub mod args;
pub mod effects;
pub mod plan;
pub mod prompts;
pub mod send;
pub mod text;

use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;

const MAX_INPUT: u64 = 1024 * 1024;

pub fn main() -> i32 {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.first().map(String::as_str) == Some(effects::EFFECTS_FLAG) {
        effects::run_detached_effects(
            raw.get(1).map(String::as_str).unwrap_or_default(),
            raw.get(2).map(String::as_str) != Some("0"),
        );
        return 0;
    }
    let args = args::parse(raw);
    let input = read_input();
    let store = prompts::Store::new(state_dir());
    let plan = plan::plan(args, input.as_ref(), &store);
    let dry_run = std::env::var_os("QUACKO_DRY_RUN").is_some_and(|v| !v.is_empty());

    let config = config_text();
    if plan.effects {
        let prefs = effects::Prefs::from_config(config.as_deref());
        if dry_run {
            eprintln!("{}", effects::describe(&plan.status, &plan.sound, prefs));
        } else {
            effects::start(&plan.status, &plan.sound, prefs);
        }
    }
    if let Some(payload) = &plan.payload {
        let body = payload.to_string();
        if dry_run {
            eprintln!("payload: {body}");
        } else {
            send::post(port(config.as_deref()), &body);
        }
    }
    if let Some(reply) = plan.reply {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{reply}");
        let _ = out.flush();
    }
    0
}

fn read_input() -> Option<serde_json::Value> {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return None;
    }
    let mut bytes = Vec::new();
    stdin.lock().take(MAX_INPUT).read_to_end(&mut bytes).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn home() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .iter()
        .filter_map(std::env::var_os)
        .find(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn state_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("QUACKO_STATE_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| home().map(|h| h.join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    cache.join("quacko")
}

fn config_text() -> Option<String> {
    std::env::var_os("QUACKO_CONFIG")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| home().map(|h| h.join(".config").join("quacko").join("config.json")))
        .and_then(|p| std::fs::read_to_string(p).ok())
}

fn port(config: Option<&str>) -> u16 {
    send::port(std::env::var("QUACKO_PORT").ok().as_deref(), config)
}
