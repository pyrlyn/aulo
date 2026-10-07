//! aulo audio I/O. Capture today: a microphone, by name or the default, as
//! 20 ms frames of 16 kHz mono `f32` in a fixed-size lock-free ring (spec §4.3,
//! §5.3). The audio callback never blocks or allocates, and a full ring drops
//! whole frames and counts them.
//!
//! The platform sits behind [`InputBackend`]; [`CpalBackend`] is the real one
//! and `testkit::FakeBackend` (feature `testkit`) the scriptable fake.
//!
//! `speech-capture` (T1.13) records to memory for push-to-talk and takes only
//! the default device, so it cannot stream; this crate owns the streaming path
//! until that crate grows one.

mod capture;
mod cpal_input;
mod error;
mod sink;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use capture::{
    Capture, DEFAULT_RING_FRAMES, DeviceSelector, InputBackend, InputDevice, StreamGuard,
};
pub use cpal_input::{CpalBackend, CpalDevice};
pub use error::AudioError;
pub use sink::{FRAME_MS, FRAME_SAMPLES, SampleSink};
