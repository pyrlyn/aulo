//! Acoustic echo cancellation for Linux (T6.5).
//!
//! WebRTC AEC3, with the high-pass filter, noise suppression and a digital
//! gain controller. The far end is what was played: pass each 10 ms of the
//! playback stream to [`EchoCanceller::render`], then the microphone frame
//! from the same moment to [`EchoCanceller::capture`]. Render does not modify
//! its buffer, so the speaker path can keep the samples.
//!
//! The `aec3` feature is off by default. The C++ library is built on Linux
//! only: the bundled MSVC build fails (tonarino/webrtc-audio-processing#102,
//! still open), and macOS uses Voice Processing I/O (T6.4). Elsewhere
//! [`EchoCanceller::new`] returns an error.
//!
//! Do not call this from an audio callback. The wrapper allocates a channel
//! pointer, and a callback must not allocate or block.
//!
//! `webrtc-audio-processing` 2.1.0 is BSD-3-Clause. The bundled build also
//! compiles abseil-cpp (Apache-2.0), which meson downloads.

use crate::AudioError;

/// Sample rate of every frame. AEC3 and the voice pipeline both use 16 kHz.
pub const ECHO_SAMPLE_RATE_HZ: u32 = 16_000;
/// One frame is 10 ms. WebRTC refuses any other length.
pub const ECHO_FRAME_SAMPLES: usize = ECHO_SAMPLE_RATE_HZ as usize / 100;

/// AEC3 plus noise suppression and digital gain control.
///
/// One instance is enough for a capture thread and a render thread: the
/// underlying processor is `Send` and `Sync`.
pub struct EchoCanceller {
    #[cfg(target_os = "linux")]
    inner: webrtc_audio_processing::Processor,
    /// Stops other crates building a value that has no processor.
    #[cfg(not(target_os = "linux"))]
    _private: (),
}

impl std::fmt::Debug for EchoCanceller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EchoCanceller").finish_non_exhaustive()
    }
}

impl EchoCanceller {
    /// Delay estimation, the high-pass filter, noise suppression and digital AGC.
    pub fn new() -> Result<Self, AudioError> {
        Self::with_delay(None)
    }

    /// `delay_ms` is how long playback takes to reach the microphone.
    /// `None` lets AEC3 estimate it.
    pub fn with_delay(delay_ms: Option<u16>) -> Result<Self, AudioError> {
        #[cfg(target_os = "linux")]
        {
            start(delay_ms)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = delay_ms;
            Err(unavailable())
        }
    }

    /// Records one 10 ms far-end frame. `frame` is not modified.
    pub fn render(&self, frame: &[f32]) -> Result<(), AudioError> {
        check_frame(frame)?;
        self.feed_render(frame)
    }

    /// Removes the echo from one 10 ms microphone frame, in place.
    pub fn capture(&self, frame: &mut [f32]) -> Result<(), AudioError> {
        check_frame(frame)?;
        self.feed_capture(frame)
    }
}

fn check_frame(frame: &[f32]) -> Result<(), AudioError> {
    if frame.len() != ECHO_FRAME_SAMPLES {
        return Err(AudioError::Invalid {
            what: "echo canceller frame",
            reason: "expected 160 samples, 10 ms at 16 kHz",
        });
    }
    if frame.iter().any(|sample| !sample.is_finite()) {
        return Err(AudioError::Invalid {
            what: "echo canceller frame",
            reason: "samples must be finite",
        });
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn unavailable() -> AudioError {
    AudioError::Invalid {
        what: "echo canceller",
        reason: "AEC3 is built for Linux only",
    }
}

#[cfg(target_os = "linux")]
fn start(delay_ms: Option<u16>) -> Result<EchoCanceller, AudioError> {
    let inner =
        webrtc_audio_processing::Processor::new(ECHO_SAMPLE_RATE_HZ).map_err(processing_error)?;
    inner.set_config(config(delay_ms));
    Ok(EchoCanceller { inner })
}

#[cfg(target_os = "linux")]
fn config(delay_ms: Option<u16>) -> webrtc_audio_processing::Config {
    use webrtc_audio_processing::config::{
        AdaptiveDigital, EchoCanceller, FixedDigital, GainController, GainController2,
        HighPassFilter, NoiseSuppression,
    };
    webrtc_audio_processing::Config {
        high_pass_filter: Some(HighPassFilter {
            apply_in_full_band: true,
        }),
        echo_canceller: Some(EchoCanceller::Full {
            stream_delay_ms: delay_ms,
        }),
        noise_suppression: Some(NoiseSuppression::default()),
        // AGC2 after the canceller. No input-volume controller: that one asks
        // the audio HAL for an analog gain, which this process does not have.
        // The adaptive stage will not lift the noise floor above -50 dBFS.
        gain_controller: Some(GainController::GainController2(GainController2 {
            input_volume_controller_enabled: false,
            adaptive_digital: Some(AdaptiveDigital::default()),
            fixed_digital: FixedDigital { gain_db: 0.0 },
        })),
        ..webrtc_audio_processing::Config::default()
    }
}

#[cfg(target_os = "linux")]
fn processing_error(err: webrtc_audio_processing::Error) -> AudioError {
    let reason = match err {
        webrtc_audio_processing::Error::BadSampleRate => "sample rate is not supported",
        webrtc_audio_processing::Error::BadDataLength => "frame length is not 10 ms",
        webrtc_audio_processing::Error::BadNumberChannels => "echo canceller takes one channel",
        webrtc_audio_processing::Error::InitializationFailed => "echo canceller failed to start",
        _ => "echo canceller rejected the frame",
    };
    AudioError::Invalid {
        what: "echo canceller",
        reason,
    }
}

#[cfg(target_os = "linux")]
impl EchoCanceller {
    fn feed_render(&self, frame: &[f32]) -> Result<(), AudioError> {
        self.inner
            .analyze_render_frame([frame])
            .map_err(processing_error)
    }

    fn feed_capture(&self, frame: &mut [f32]) -> Result<(), AudioError> {
        self.inner
            .process_capture_frame([frame])
            .map_err(processing_error)
    }
}

#[cfg(not(target_os = "linux"))]
impl EchoCanceller {
    fn feed_render(&self, _frame: &[f32]) -> Result<(), AudioError> {
        Err(unavailable())
    }

    fn feed_capture(&self, _frame: &mut [f32]) -> Result<(), AudioError> {
        Err(unavailable())
    }
}
