use std::fs;
use std::io::{self};

use super::files::install_script;
use super::*;

pub(super) const NOTIFY_SH: &str = include_str!("../../../scripts/notify.sh");
pub(super) const PRWATCH: &str = include_str!("../../../scripts/prwatch");

pub(super) const OURS_MARKER: &str = "QUACKO_";
pub(super) const LEGACY_MARKER: &str = "MIAU_";

pub fn sync_installed_scripts(paths: &Paths) -> Vec<String> {
    let mut updated = Vec::new();
    for (path, embedded) in [(paths.notify_sh(), NOTIFY_SH), (paths.prwatch(), PRWATCH)] {
        let Ok(current) = fs::read_to_string(&path) else {
            continue;
        };
        if current == embedded
            || !(current.contains(OURS_MARKER) || current.contains(LEGACY_MARKER))
        {
            continue;
        }
        match install_script(&path, embedded) {
            Ok(()) => updated.push(
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("script")
                    .to_string(),
            ),
            Err(e) => {
                tracing::warn!(%e, path = %path.display(), "não foi possível atualizar o script")
            }
        }
    }
    updated
}

pub enum PrwatchOutcome {
    Linked,
    Conflict,
}

pub fn install_prwatch(paths: &Paths) -> io::Result<PrwatchOutcome> {
    install_script(&paths.notify_sh(), NOTIFY_SH)?;
    install_script(&paths.prwatch(), PRWATCH)?;
    let link = paths.local_bin_prwatch();
    if let Some(dir) = link.parent() {
        fs::create_dir_all(dir)?;
    }
    match fs::read_link(&link) {
        Ok(dest) if dest == paths.prwatch() => return Ok(PrwatchOutcome::Linked),
        Ok(_) => return Ok(PrwatchOutcome::Conflict),
        Err(_) if link.exists() => return Ok(PrwatchOutcome::Conflict),
        Err(_) => {}
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(paths.prwatch(), &link)?;
    Ok(PrwatchOutcome::Linked)
}
