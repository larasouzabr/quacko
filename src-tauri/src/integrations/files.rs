use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use serde_json::{json, Value};

pub(super) fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(super) fn prepare(path: &Path) -> Result<(PathBuf, Value, bool), ManualReason> {
    if !path.exists() && fs::symlink_metadata(path).is_err() {
        return Ok((path.to_path_buf(), json!({}), false));
    }
    let target = fs::canonicalize(path).map_err(|_| ManualReason::ReadOnly)?;
    if target.starts_with("/nix/store") {
        return Err(ManualReason::Managed);
    }
    let writable = fs::OpenOptions::new().append(true).open(&target).is_ok()
        && target.parent().is_some_and(|dir| tempfile_in(dir).is_ok());
    if !writable {
        return Err(ManualReason::ReadOnly);
    }
    let text = fs::read_to_string(&target).map_err(|_| ManualReason::ReadOnly)?;
    let doc = if text.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&text).map_err(|_| ManualReason::Invalid)?
    };
    Ok((target, doc, true))
}

pub(super) fn tempfile_in(dir: &Path) -> io::Result<()> {
    let probe = dir.join(format!(".quacko-probe-{}", std::process::id()));
    fs::File::create(&probe)?;
    fs::remove_file(&probe)
}

pub(super) fn write_atomic(path: &Path, contents: &str) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "caminho sem pasta"))?;
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.quacko-tmp-{}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("arquivo"),
        std::process::id()
    ));
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
    }
    if let Ok(meta) = fs::metadata(path) {
        let _ = fs::set_permissions(&tmp, meta.permissions());
    }
    fs::rename(&tmp, path)
}

pub(super) fn install_script(path: &Path, contents: &str) -> io::Result<()> {
    if let Ok(current) = fs::read_to_string(path) {
        if current == contents {
            return Ok(());
        }
        let mut backup = PathBuf::from(format!("{}.bak", path.display()));
        if backup.exists() {
            backup = PathBuf::from(format!("{}.bak-{}", path.display(), timestamp()));
        }
        fs::copy(path, &backup)?;
    }
    write_atomic(path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}
