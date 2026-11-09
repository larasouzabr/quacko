use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn file(&self, tool: &str, id: &str) -> Option<PathBuf> {
        let clean: String = id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .take(80)
            .collect();
        (!clean.is_empty()).then(|| self.dir.join(format!("prompt-{tool}-{clean}")))
    }

    pub fn save(&self, file: Option<&Path>, prompt: &str) {
        let Some(file) = file else {
            return;
        };
        if create_private_dir(&self.dir).is_err() {
            return;
        }
        self.remove_old();
        let tmp = file.with_extension("tmp");
        if write_private(&tmp, prompt).is_ok() {
            let _ = fs::rename(&tmp, file);
        }
    }

    pub fn take(&self, file: Option<&Path>) -> String {
        let Some(file) = file else {
            return String::new();
        };
        let text = fs::read_to_string(file).unwrap_or_default();
        let _ = fs::remove_file(file);
        text
    }

    pub fn discard(&self, file: Option<&Path>) {
        if let Some(file) = file {
            let _ = fs::remove_file(file);
        }
    }

    fn remove_old(&self) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        let now = SystemTime::now();
        for entry in entries.flatten() {
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| now.duration_since(t).ok())
                .is_some_and(|age| age > MAX_AGE);
            if old && entry.file_name().to_string_lossy().starts_with("prompt-") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    std::io::Write::write_all(&mut options.open(path)?, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_higienizado_e_vazio_sem_arquivo() {
        let store = Store::new(PathBuf::from("/estado"));
        assert_eq!(
            store.file("claude", "../../etc/x"),
            Some(PathBuf::from("/estado/prompt-claude-etcx"))
        );
        assert_eq!(store.file("claude", "../"), None);
        let long = "a".repeat(200);
        let name = store.file("cursor", &long).unwrap();
        assert_eq!(name.file_name().unwrap().len(), "prompt-cursor-".len() + 80);
    }

    #[test]
    fn guarda_le_uma_vez_e_apaga() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("estado"));
        let file = store.file("claude", "s1");
        store.save(file.as_deref(), "pedido um");
        assert_eq!(store.take(file.as_deref()), "pedido um");
        assert_eq!(store.take(file.as_deref()), "", "já foi usado");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.path().join("estado"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700, "pasta só do usuário");
        }
    }
}
