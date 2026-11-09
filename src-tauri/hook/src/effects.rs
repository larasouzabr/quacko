#[cfg(target_os = "macos")]
const MAC_PAUSE: &str = r#"osascript -e 'if application "Spotify" is running then tell application "Spotify" to pause'
PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"; command -v nowplaying-cli >/dev/null && nowplaying-cli pause"#;

pub fn default_sound(status: &str) -> &'static str {
    #[cfg(target_os = "macos")]
    {
        match status {
            "fail" => "/System/Library/Sounds/Basso.aiff",
            "attention" | "info" => "/System/Library/Sounds/Ping.aiff",
            _ => "/System/Library/Sounds/Glass.aiff",
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = status;
        ""
    }
}

pub fn start(status: &str, sound: &str) {
    let sound = if sound.is_empty() {
        default_sound(status)
    } else {
        sound
    };
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};
        let detached = |cmd: &mut Command| {
            let _ = cmd
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .process_group(0)
                .spawn();
        };
        detached(Command::new("/bin/bash").args(["-c", MAC_PAUSE]));
        if std::path::Path::new(sound).is_file() {
            detached(Command::new("/usr/bin/afplay").arg(sound));
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = sound;
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn mesmos_sons_do_notify_sh() {
        let script = include_str!("../../../scripts/notify.sh");
        for status in ["ok", "fail", "attention", "info"] {
            let sound = default_sound(status);
            assert!(script.contains(sound), "{status}: {sound}");
        }
    }
}
