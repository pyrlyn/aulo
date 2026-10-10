//! Microphone permission. A denied microphone does not fail loudly: macOS and
//! Windows both open the stream and deliver silence. So the capture path asks
//! the operating system first, and turns a refusal into a notice that names the
//! settings page to fix it, instead of a voice that never hears anything.
//!
//! The probes only read the current state; asking (which shows a system
//! dialog, and kills a process that has no usage description) is left to the
//! desktop app.

use std::fmt;

use aulo_types::{AuloEvent, NoticeLevel};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// What the operating system says about this process using the microphone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MicPermission {
    Granted,
    /// The user (or an earlier prompt) refused.
    Denied,
    /// Nobody has been asked yet.
    NotDetermined,
    /// A system policy, such as a managed device, forbids it and the user
    /// cannot change that.
    Restricted,
    /// The platform has no permission to read, or it could not be read.
    Unknown,
}

impl MicPermission {
    /// Whether the daemon must not start a stream. `NotDetermined` blocks too:
    /// a first stream start makes the system prompt, and macOS kills a launchd
    /// process that prompts, so asking is left to the desktop app.
    pub fn blocks_capture(self) -> bool {
        matches!(self, Self::Denied | Self::Restricted | Self::NotDetermined)
    }

    /// The notice for this state, or `None` when there is nothing to tell.
    pub fn notice(self, platform: Platform) -> Option<AuloEvent> {
        let path = platform.settings_path();
        let (level, message) = match self {
            Self::Granted | Self::Unknown => return None,
            Self::Denied => (
                NoticeLevel::Error,
                format!(
                    "Microphone access is denied, so voice cannot hear you. {}",
                    platform.fix()
                ),
            ),
            Self::Restricted => (
                NoticeLevel::Error,
                format!(
                    "Microphone access is blocked by a system policy, so voice cannot hear you. Ask your administrator to allow it in {path}.",
                ),
            ),
            Self::NotDetermined => (
                NoticeLevel::Error,
                format!(
                    "Microphone access has not been granted yet, so voice cannot hear you. Open the aulo app and allow it when the system asks, or allow it in {path}.",
                ),
            ),
        };
        Some(notice(level, message))
    }
}

impl fmt::Display for MicPermission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Granted => "granted",
            Self::Denied => "denied",
            Self::NotDetermined => "not determined",
            Self::Restricted => "restricted",
            Self::Unknown => "unknown",
        })
    }
}

/// Which settings app the fix text points at. A value rather than a `cfg` so
/// the text of every platform is tested on whichever machine runs the tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Windows,
    Other,
}

impl Platform {
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Other
        }
    }

    fn settings_path(self) -> &'static str {
        match self {
            Self::MacOs => "System Settings > Privacy & Security > Microphone",
            Self::Windows => "Settings > Privacy & security > Microphone",
            Self::Other => "your system's sound settings",
        }
    }

    fn fix(self) -> String {
        let path = self.settings_path();
        match self {
            Self::MacOs => format!(
                "Open {path}, turn on the app that runs aulo (Terminal, or aulo itself), then restart it."
            ),
            // The three switches are separate in Windows 11, and a desktop app
            // such as aulo is only covered by the last one.
            Self::Windows => format!(
                "Open {path} and turn on Microphone access, Let apps access your microphone and Let desktop apps access your microphone, then restart aulo."
            ),
            Self::Other => format!(
                "Check {path}: this user must be allowed to record from the sound server (PipeWire, PulseAudio or ALSA). Then restart aulo."
            ),
        }
    }
}

/// The notice for a capture that is running but hears only digital silence
/// (see [`crate::SilenceWatch`]). A known refusal gets its own, exact text;
/// otherwise the microphone may simply be muted, so the notice says both.
pub fn silence_notice(permission: MicPermission, platform: Platform) -> AuloEvent {
    permission.notice(platform).unwrap_or_else(|| {
        notice(
            NoticeLevel::Warn,
            format!(
                "The microphone delivers only silence. It may be muted, or the system may be blocking access. {}",
                platform.fix()
            ),
        )
    })
}

fn notice(level: NoticeLevel, message: String) -> AuloEvent {
    AuloEvent::Notice {
        level,
        message,
        source: Some("microphone".to_owned()),
    }
}

/// Reads the microphone permission without prompting.
pub trait MicPermissionProbe {
    fn status(&self) -> MicPermission;
}

/// The operating system's own answer: TCC on macOS, the capability access
/// manager's consent store on Windows, and [`MicPermission::Unknown`] on Linux,
/// which has no per-app microphone permission.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemProbe;

impl MicPermissionProbe for SystemProbe {
    fn status(&self) -> MicPermission {
        #[cfg(target_os = "macos")]
        return macos::status();
        #[cfg(windows)]
        return windows::status();
        #[cfg(not(any(target_os = "macos", windows)))]
        MicPermission::Unknown
    }
}

/// Folds consent values (`Allow` or `Deny`, from the most general switch to the
/// most specific; `None` where a value is absent) into one answer. Any `Deny`
/// wins, because Windows applies every switch; with no value at all nothing
/// has ever been decided, and that is not a refusal.
#[cfg(any(windows, test))]
fn consent_status<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> MicPermission {
    let mut answer = MicPermission::Unknown;
    for value in values.into_iter().flatten() {
        if value.eq_ignore_ascii_case("Deny") {
            return MicPermission::Denied;
        }
        if value.eq_ignore_ascii_case("Allow") {
            answer = MicPermission::Granted;
        }
    }
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(permission: MicPermission, platform: Platform) -> String {
        match permission.notice(platform) {
            Some(AuloEvent::Notice { message, .. }) => message,
            other => panic!("expected a notice, got {other:?}"),
        }
    }

    #[test]
    fn a_denial_names_the_exact_settings_path_on_each_platform() {
        let mac = message(MicPermission::Denied, Platform::MacOs);
        assert_eq!(
            mac,
            "Microphone access is denied, so voice cannot hear you. Open System Settings > Privacy & Security > Microphone, turn on the app that runs aulo (Terminal, or aulo itself), then restart it."
        );
        let win = message(MicPermission::Denied, Platform::Windows);
        assert!(
            win.contains("Settings > Privacy & security > Microphone"),
            "{win}"
        );
        assert!(
            win.contains("Let desktop apps access your microphone"),
            "{win}"
        );
        let linux = message(MicPermission::Denied, Platform::Other);
        assert!(linux.contains("PipeWire"), "{linux}");
    }

    #[test]
    fn a_denial_is_an_error_notice_about_the_microphone() {
        assert_eq!(
            MicPermission::Denied.notice(Platform::MacOs),
            Some(AuloEvent::Notice {
                level: NoticeLevel::Error,
                message: message(MicPermission::Denied, Platform::MacOs),
                source: Some("microphone".to_owned()),
            })
        );
    }

    #[test]
    fn a_policy_block_says_the_user_cannot_fix_it_alone() {
        let text = message(MicPermission::Restricted, Platform::MacOs);
        assert!(
            text.contains("system policy") && text.contains("administrator"),
            "{text}"
        );
        assert!(
            text.contains("System Settings > Privacy & Security > Microphone"),
            "{text}"
        );
    }

    #[test]
    fn an_unasked_microphone_points_to_the_app_and_a_granted_or_unknown_one_is_silent() {
        let Some(AuloEvent::Notice { level, message, .. }) =
            MicPermission::NotDetermined.notice(Platform::Windows)
        else {
            panic!("no notice for an unasked microphone");
        };
        assert_eq!(level, NoticeLevel::Error);
        assert!(message.contains("Open the aulo app"), "{message}");
        assert_eq!(MicPermission::Granted.notice(Platform::MacOs), None);
        assert_eq!(MicPermission::Unknown.notice(Platform::MacOs), None);
    }

    #[test]
    fn a_refusal_or_an_unasked_microphone_blocks_capture() {
        use MicPermission::*;
        let blocked: Vec<_> = [Granted, Denied, NotDetermined, Restricted, Unknown]
            .into_iter()
            .filter(|p| p.blocks_capture())
            .collect();
        assert_eq!(blocked, [Denied, NotDetermined, Restricted]);
    }

    #[test]
    fn silence_with_a_known_refusal_reports_the_refusal_otherwise_a_generic_hint() {
        let refused = silence_notice(MicPermission::Denied, Platform::MacOs);
        assert_eq!(MicPermission::Denied.notice(Platform::MacOs), Some(refused));
        let AuloEvent::Notice { level, message, .. } =
            silence_notice(MicPermission::Granted, Platform::MacOs)
        else {
            panic!("not a notice");
        };
        assert_eq!(level, NoticeLevel::Warn);
        assert!(
            message.contains("only silence") && message.contains("muted"),
            "{message}"
        );
        assert!(
            message.contains("System Settings > Privacy & Security > Microphone"),
            "{message}"
        );
    }

    #[test]
    fn consent_values_fold_with_deny_winning() {
        assert_eq!(
            consent_status([Some("Allow"), Some("Allow")]),
            MicPermission::Granted
        );
        assert_eq!(
            consent_status([Some("Allow"), Some("Deny")]),
            MicPermission::Denied
        );
        assert_eq!(consent_status([Some("deny"), None]), MicPermission::Denied);
        assert_eq!(consent_status([None, None]), MicPermission::Unknown);
        assert_eq!(consent_status([Some("Prompt")]), MicPermission::Unknown);
    }
}
