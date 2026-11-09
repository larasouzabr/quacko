use crate::model::Status;

pub fn system_sound(status: Status) -> &'static str {
    match status {
        Status::Ok => "/System/Library/Sounds/Glass.aiff",
        Status::Fail => "/System/Library/Sounds/Basso.aiff",
        Status::Attention | Status::Info => "/System/Library/Sounds/Ping.aiff",
    }
}

pub fn play(status: Status) {
    #[cfg(target_os = "macos")]
    {
        use std::process::{Command, Stdio};
        let path = system_sound(status);
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
    let _ = status;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesmos_sons_do_notify_sh() {
        assert!(system_sound(Status::Ok).ends_with("Glass.aiff"));
        assert!(system_sound(Status::Fail).ends_with("Basso.aiff"));
        assert!(system_sound(Status::Attention).ends_with("Ping.aiff"));
        assert!(system_sound(Status::Info).ends_with("Ping.aiff"));
        let script = include_str!("../../scripts/notify.sh");
        for s in ["Glass.aiff", "Basso.aiff", "Ping.aiff"] {
            assert!(script.contains(s), "o notify.sh usa {s}");
        }
    }
}
