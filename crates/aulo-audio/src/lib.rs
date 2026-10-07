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
//! The real backend wraps the shared `speech-capture` crate, which owns the
//! `cpal` streams.

mod capture;
mod cpal_input;
mod cpal_output;
mod error;
mod playback;
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
pub use playback::{DEFAULT_BUFFER_MS, MAX_BUFFER_MS, OutputBackend, OutputDevice, Playback};
pub use sink::{FRAME_MS, FRAME_SAMPLES, SampleSink};
pub use source::SampleSource;
