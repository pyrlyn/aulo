//! aulo audio I/O. Capture today: a microphone, by name or the default, as
//! 20 ms frames of 16 kHz mono `f32` in a fixed-size lock-free ring (spec §4.3,
//! §5.3). The audio callback never blocks or allocates, and a full ring drops
//! whole frames and counts them.
//!
//! The platform sits behind [`InputBackend`]; [`CpalBackend`] is the real one
//! and `testkit::FakeBackend` (feature `testkit`) the scriptable fake.
//!
//! A refused microphone delivers silence rather than an error, so
//! [`Capture::start_checked`] asks a [`MicPermissionProbe`] first and
//! [`MicPermission::notice`] turns the answer into a notice that names the
//! settings page to fix. [`SilenceWatch`] catches the same failure from the
//! frames when no probe can tell.
//!
//! The real backend wraps the shared `speech-capture` crate, which owns the
//! `cpal` stream.

mod capture;
mod cpal_input;
mod error;
mod permission;
mod silence;
mod sink;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use capture::{
    Capture, DEFAULT_RING_FRAMES, DeviceSelector, InputBackend, InputDevice, StreamGuard,
};
pub use cpal_input::{CpalBackend, CpalDevice};
pub use error::AudioError;
pub use permission::{MicPermission, MicPermissionProbe, Platform, SystemProbe, silence_notice};
pub use silence::{DEFAULT_SILENCE_FRAMES, SilenceWatch};
pub use sink::{FRAME_MS, FRAME_SAMPLES, SampleSink};
