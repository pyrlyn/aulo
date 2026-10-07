//! Audio as it crosses the engine boundary. A frame borrows the caller's
//! samples, so pushing audio into an engine never copies or allocates on the
//! way in; the format travels with the frame so an engine can refuse audio it
//! cannot take instead of mis-decoding it.

use crate::SpeechError;

/// Sample rate of the capture pipeline (spec §4.3). aulo-audio resamples every
/// input to it, so engines that accept only this rate need no resampler.
pub const PIPELINE_SAMPLE_RATE_HZ: u32 = 16_000;

const MS_PER_SECOND: u64 = 1_000;
const MONO: u8 = 1;

/// Sample rate and channel count of interleaved `f32` samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioFormat {
    sample_rate_hz: u32,
    channels: u8,
}

impl AudioFormat {
    /// The format of every capture frame: 16 kHz mono.
    pub const PIPELINE: Self = Self {
        sample_rate_hz: PIPELINE_SAMPLE_RATE_HZ,
        channels: MONO,
    };

    /// Rejects a zero rate or zero channels here, so time conversions below
    /// can never divide by zero.
    pub fn new(sample_rate_hz: u32, channels: u8) -> Result<Self, SpeechError> {
        if sample_rate_hz == 0 {
            return Err(SpeechError::invalid("audio format", "sample rate is zero"));
        }
        if channels == 0 {
            return Err(SpeechError::invalid(
                "audio format",
                "channel count is zero",
            ));
        }
        Ok(Self {
            sample_rate_hz,
            channels,
        })
    }

    pub fn sample_rate_hz(self) -> u32 {
        self.sample_rate_hz
    }

    pub fn channels(self) -> u8 {
        self.channels
    }

    /// Converts a stream position (per-channel samples since the stream
    /// started) to milliseconds, the unit events carry.
    pub fn position_to_ms(self, position: u64) -> u64 {
        position.saturating_mul(MS_PER_SECOND) / u64::from(self.sample_rate_hz)
    }
}

/// A run of interleaved samples and where it sits in its stream.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioFrame<'a> {
    samples: &'a [f32],
    format: AudioFormat,
    position: u64,
}

impl<'a> AudioFrame<'a> {
    /// `position` is the per-channel index of the first sample since the
    /// stream started; detectors stamp their events with it.
    pub fn new(
        samples: &'a [f32],
        format: AudioFormat,
        position: u64,
    ) -> Result<Self, SpeechError> {
        if !samples.len().is_multiple_of(usize::from(format.channels)) {
            return Err(SpeechError::invalid(
                "audio frame",
                "sample count is not a multiple of the channel count",
            ));
        }
        Ok(Self {
            samples,
            format,
            position,
        })
    }

    pub fn samples(&self) -> &'a [f32] {
        self.samples
    }

    pub fn format(&self) -> AudioFormat {
        self.format
    }

    pub fn position(&self) -> u64 {
        self.position
    }

    /// Per-channel samples in this frame.
    pub fn len(&self) -> usize {
        self.samples.len() / usize::from(self.format.channels)
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Position of the first sample after this frame, which is where the next
    /// contiguous frame starts.
    pub fn end_position(&self) -> u64 {
        // usize always fits u64 on the targets aulo builds for.
        self.position.saturating_add(self.len() as u64)
    }

    pub fn at_ms(&self) -> u64 {
        self.format.position_to_ms(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWENTY_MS_AT_16K: usize = 320;

    #[test]
    fn zero_rate_or_channels_is_rejected() {
        assert!(AudioFormat::new(0, 1).is_err());
        assert!(AudioFormat::new(PIPELINE_SAMPLE_RATE_HZ, 0).is_err());
    }

    #[test]
    fn interleaved_frame_must_hold_whole_sample_groups() {
        let stereo = AudioFormat::new(48_000, 2).unwrap();
        assert!(AudioFrame::new(&[0.0; 3], stereo, 0).is_err());
        assert_eq!(AudioFrame::new(&[0.0; 4], stereo, 0).unwrap().len(), 2);
    }

    #[test]
    fn positions_convert_to_milliseconds_at_the_frame_rate() {
        let samples = [0.0; TWENTY_MS_AT_16K];
        let frame = AudioFrame::new(&samples, AudioFormat::PIPELINE, 16_000).unwrap();
        assert_eq!(frame.at_ms(), 1_000);
        assert_eq!(frame.end_position(), 16_320);
        assert_eq!(
            AudioFormat::PIPELINE.position_to_ms(frame.end_position()),
            1_020
        );
    }
}
