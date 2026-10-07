//! macOS TCC through `AVAudioApplication.recordPermission` (macOS 14+; aulo
//! needs 15). It reports the state without asking, unlike
//! `requestRecordPermission`, and its enum has no restricted case, so a
//! managed Mac reads as denied, which carries the same fix.
//!
//! The bindings are `unsafe` because every Objective-C message is; both calls
//! here are argument-free getters.
#![allow(unsafe_code)]

use objc2_avf_audio::{AVAudioApplication, AVAudioApplicationRecordPermission};

use super::MicPermission;

pub(super) fn status() -> MicPermission {
    // SAFETY: a class method with no arguments that returns the shared instance.
    let app = unsafe { AVAudioApplication::sharedInstance() };
    // SAFETY: a property getter; it reads the TCC state and never prompts.
    map(unsafe { app.recordPermission() })
}

fn map(permission: AVAudioApplicationRecordPermission) -> MicPermission {
    match permission {
        AVAudioApplicationRecordPermission::Granted => MicPermission::Granted,
        AVAudioApplicationRecordPermission::Denied => MicPermission::Denied,
        AVAudioApplicationRecordPermission::Undetermined => MicPermission::NotDetermined,
        _ => MicPermission::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tcc_state_maps() {
        assert_eq!(
            map(AVAudioApplicationRecordPermission::Granted),
            MicPermission::Granted
        );
        assert_eq!(
            map(AVAudioApplicationRecordPermission::Denied),
            MicPermission::Denied
        );
        assert_eq!(
            map(AVAudioApplicationRecordPermission::Undetermined),
            MicPermission::NotDetermined
        );
        assert_eq!(
            map(AVAudioApplicationRecordPermission(0)),
            MicPermission::Unknown
        );
    }
}
