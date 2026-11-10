use crate::model::Status;

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Ok => "ok",
        Status::Fail => "fail",
        Status::Attention => "attention",
        Status::Info => "info",
    }
}

pub fn play(config: &crate::config::Config, status: Status) {
    if !config.sound || config.dnd {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::{Command, Stdio};
        let path = quacko_hook::effects::default_sound(status_name(status));
        if !std::path::Path::new(path).exists() {
            return;
        }
        match Command::new("/usr/bin/afplay")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Err(e) => tracing::warn!(%e, "não foi possível tocar o som"),
        }
    }
    #[cfg(not(target_os = "macos"))]
    quacko_hook::effects::play_async(status_name(status));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn um_som_por_estado() {
        let names: Vec<&str> = [Status::Ok, Status::Fail, Status::Attention, Status::Info]
            .into_iter()
            .map(status_name)
            .collect();
        assert_eq!(names, ["ok", "fail", "attention", "info"]);
        if cfg!(target_os = "macos") {
            assert!(quacko_hook::effects::default_sound("ok").ends_with("Glass.aiff"));
            assert!(quacko_hook::effects::default_sound("fail").ends_with("Basso.aiff"));
        }
    }
}
