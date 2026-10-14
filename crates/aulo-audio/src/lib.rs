//! aulo audio I/O (spec §4.3, §5.3). Capture: a microphone, by name or the
//! default, as 20 ms frames of 16 kHz mono `f32` in a fixed-size lock-free
//! ring; a full ring drops whole frames and counts them. Playback: streaming
//! audio of any format to a speaker, with a volume, an immediate stop for
//! barge-in and a count of what has been played; a full ring pushes back on
//! the writer instead of dropping, and a ring that runs dry mid-reply counts an
//! underrun. Neither audio callback blocks or allocates.
//!
//! The platform sits behind [`InputBackend`] and [`OutputBackend`];
//! [`CpalBackend`] is the real one for both and `testkit::FakeBackend` and
//! `testkit::FakeOutputBackend` (feature `testkit`) the scriptable fakes.
//!
//! A refused microphone delivers silence rather than an error, so
//! [`Capture::start_checked`] asks a [`MicPermissionProbe`] first and
//! [`MicPermission::notice`] turns the answer into a notice that names the
//! settings page to fix. [`SilenceWatch`] catches the same failure from the
//! frames when no probe can tell.
//!
//! The real backend wraps the shared `speech-capture` crate, which owns the
//! `cpal` streams.
//!
//! Feature `opus` adds [`OpusEncoder`] and [`OpusDecoder`] for remote clients
//! that need less bandwidth than PCM16 (T6.7). It builds libopus, so it is off
//! by default.

mod capture;
mod cpal_input;
mod cpal_output;
mod error;
#[cfg(feature = "opus")]
mod opus;
mod permission;
mod playback;
mod silence;
mod sink;
mod source;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use capture::{
    Capture, DEFAULT_RING_FRAMES, DeviceSelector, InputBackend, InputDevice, StreamGuard,
};
pub use cpal_input::{CpalBackend, CpalDevice};
pub use cpal_output::CpalOutputDevice;
pub use error::AudioError;
#[cfg(feature = "opus")]
pub use opus::{
    DEFAULT_BITRATE_BPS, MAX_FRAMES_PER_PACKET, MAX_PACKET_BYTES, MAX_PACKET_SAMPLES, OpusDecoder,
    OpusEncoder, OpusError,
};
pub use permission::{MicPermission, MicPermissionProbe, Platform, SystemProbe, silence_notice};
pub use playback::{DEFAULT_BUFFER_MS, MAX_BUFFER_MS, OutputBackend, OutputDevice, Playback};
pub use silence::{DEFAULT_SILENCE_FRAMES, SilenceWatch};
pub use sink::{FRAME_MS, FRAME_SAMPLES, SampleSink};
pub use source::SampleSource;
