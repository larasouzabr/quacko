pub const EFFECTS_FLAG: &str = "--effects";

#[cfg(target_os = "macos")]
const MAC_PAUSE: &str = r#"osascript -e 'if application "Spotify" is running then tell application "Spotify" to pause'
PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"; command -v nowplaying-cli >/dev/null && nowplaying-cli pause"#;

pub fn default_sound(status: &str) -> &'static str {
    if cfg!(target_os = "macos") {
        match status {
            "fail" => "/System/Library/Sounds/Basso.aiff",
            "attention" | "info" => "/System/Library/Sounds/Ping.aiff",
            _ => "/System/Library/Sounds/Glass.aiff",
        }
    } else if cfg!(windows) {
        match status {
            "fail" => "SystemHand",
            "attention" | "info" => "Notification.IM",
            _ => "Notification.Default",
        }
    } else {
        ""
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prefs {
    pub sound: bool,
    pub pause: bool,
}

impl Prefs {
    pub fn from_config(text: Option<&str>) -> Self {
        let config = text.and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok());
        let flag = |key: &str, default: bool| {
            config
                .as_ref()
                .and_then(|c| c.get(key))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(default)
        };
        let quiet = flag("dnd", false);
        Self {
            sound: !quiet && flag("sound", true),
            pause: !quiet && flag("pause_media", true),
        }
    }
}

pub fn origin_enabled(config: Option<&str>, origin: &str) -> bool {
    if origin.is_empty() {
        return true;
    }
    config
        .and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok())
        .and_then(|c| {
            c.get(format!("notify_{origin}"))
                .and_then(serde_json::Value::as_bool)
        })
        .unwrap_or(true)
}

fn chosen(status: &str, sound: &str) -> String {
    if sound.is_empty() {
        default_sound(status).to_string()
    } else {
        sound.to_string()
    }
}

pub fn sound_for(status: &str, sound: &str, prefs: Prefs) -> String {
    if prefs.sound {
        chosen(status, sound)
    } else {
        String::new()
    }
}

pub fn describe(status: &str, sound: &str, prefs: Prefs) -> String {
    format!(
        "effects: pause={} sound={}",
        u8::from(prefs.pause),
        sound_for(status, sound, prefs)
    )
}

pub fn start(status: &str, sound: &str, prefs: Prefs) {
    let sound = sound_for(status, sound, prefs);
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
        if prefs.pause {
            detached(Command::new("/bin/bash").args(["-c", MAC_PAUSE]));
        }
        if !sound.is_empty() && std::path::Path::new(&sound).is_file() {
            detached(Command::new("/usr/bin/afplay").arg(&sound));
        }
    }
    #[cfg(windows)]
    if prefs.pause || !sound.is_empty() {
        windows::spawn_effects(&sound, prefs.pause);
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = (sound, prefs);
}

pub fn run_detached_effects(sound: &str, pause: bool) {
    #[cfg(windows)]
    {
        if pause {
            windows::pause_media();
        }
        windows::play(sound, true);
    }
    #[cfg(not(windows))]
    let _ = (sound, pause);
}

pub fn play_async(status: &str) {
    #[cfg(windows)]
    windows::play(default_sound(status), false);
    #[cfg(not(windows))]
    let _ = status;
}

#[cfg(windows)]
mod windows {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    use windows::core::HSTRING;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager as SessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    };
    use windows::Win32::Media::Audio::{
        PlaySoundW, SND_ALIAS, SND_ASYNC, SND_FILENAME, SND_NODEFAULT, SND_SYNC,
    };
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

    pub fn spawn_effects(sound: &str, pause: bool) {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let spawn = |flags: u32| {
            Command::new(&exe)
                .args([super::EFFECTS_FLAG, sound, if pause { "1" } else { "0" }])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(flags)
                .spawn()
        };
        let base = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
        if spawn(base | CREATE_BREAKAWAY_FROM_JOB).is_err() {
            let _ = spawn(base);
        }
    }

    pub fn pause_media() {
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let _ = try_pause();
    }

    fn try_pause() -> windows::core::Result<()> {
        let manager = SessionManager::RequestAsync()?.join()?;
        for session in manager.GetSessions()? {
            let playing = session
                .GetPlaybackInfo()
                .and_then(|info| info.PlaybackStatus())
                .is_ok_and(|status| status == PlaybackStatus::Playing);
            if playing {
                let _ = session.TryPauseAsync().and_then(|op| op.join());
            }
        }
        Ok(())
    }

    pub fn play(sound: &str, wait: bool) {
        if sound.is_empty() {
            return;
        }
        let is_file =
            sound.to_ascii_lowercase().ends_with(".wav") && std::path::Path::new(sound).is_file();
        let kind = if is_file { SND_FILENAME } else { SND_ALIAS };
        let timing = if wait { SND_SYNC } else { SND_ASYNC };
        unsafe {
            let _ = PlaySoundW(&HSTRING::from(sound), None, kind | timing | SND_NODEFAULT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn mesmos_sons_do_notify_sh() {
        let script = include_str!("../../../scripts/notify.sh");
        for status in ["ok", "fail", "attention", "info"] {
            let sound = default_sound(status);
            assert!(script.contains(sound), "{status}: {sound}");
        }
    }

    #[test]
    fn preferencias_da_config() {
        let on = Prefs {
            sound: true,
            pause: true,
        };
        assert_eq!(Prefs::from_config(None), on);
        assert_eq!(Prefs::from_config(Some("não é json")), on);
        assert_eq!(Prefs::from_config(Some(r#"{"sound":"não"}"#)), on);
        assert_eq!(
            Prefs::from_config(Some(r#"{"sound":false}"#)),
            Prefs {
                sound: false,
                pause: true
            }
        );
        assert_eq!(
            Prefs::from_config(Some(r#"{"pause_media":false}"#)),
            Prefs {
                sound: true,
                pause: false
            }
        );
        assert_eq!(
            Prefs::from_config(Some(r#"{"dnd":true,"sound":true,"pause_media":true}"#)),
            Prefs {
                sound: false,
                pause: false
            }
        );
    }

    #[test]
    fn origem_desligada_na_config() {
        let config = r#"{"notify_cursor": false, "notify_ci": true}"#;
        assert!(!origin_enabled(Some(config), "cursor"));
        assert!(origin_enabled(Some(config), "ci"));
        assert!(origin_enabled(Some(config), "claude"));
        assert!(origin_enabled(Some(config), ""));
        assert!(origin_enabled(None, "cursor"));
        assert!(origin_enabled(Some(r#"{"notify_cursor":"não"}"#), "cursor"));
    }

    #[test]
    fn sem_som_nao_escolhe_arquivo() {
        let mute = Prefs {
            sound: false,
            pause: true,
        };
        assert_eq!(sound_for("ok", "/x/meu.wav", mute), "");
        assert_eq!(
            describe("ok", "/x/meu.wav", mute),
            "effects: pause=1 sound="
        );
    }

    #[test]
    fn som_escolhido_vence_o_padrao() {
        assert_eq!(chosen("ok", "/x/meu.wav"), "/x/meu.wav");
        assert_eq!(chosen("fail", ""), default_sound("fail"));
    }
}
