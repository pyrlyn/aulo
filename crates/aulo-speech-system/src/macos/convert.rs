//! Sample math between AVFoundation buffers and the engine's output: reading
//! a buffer as mono, resampling to the declared rate, and mapping aulo's
//! speech rate onto AVSpeechUtterance's. Kept apart from the FFI so every
//! step is unit-tested with plain Rust data.

use std::ptr::NonNull;

use aulo_speech::SpeechRate;

/// Full scale of signed 16-bit PCM.
const I16_SCALE: f32 = 32_768.0;
/// Rates closer than this are treated as equal and passed through untouched.
const SAME_RATE_EPSILON_HZ: f64 = 0.5;

pub(super) fn i16_to_f32(sample: i16) -> f32 {
    f32::from(sample) / I16_SCALE
}

/// Reads the first channel of `frames` frames. Speech voices are mono; for a
/// multi-channel buffer the first channel is the whole voice, so it is taken
/// rather than mixed. `stride` (`AVAudioPCMBuffer.stride`) is the distance
/// between frames, so the same read works interleaved or not.
///
/// # Safety
///
/// `plane` must be valid for `frames * stride` samples while the iterator is
/// used.
pub(super) unsafe fn first_channel<T: Copy>(
    plane: NonNull<T>,
    stride: usize,
    frames: usize,
    to_f32: impl Fn(T) -> f32,
) -> impl Iterator<Item = f32> {
    // SAFETY: the caller guarantees every offset is in bounds.
    (0..frames).map(move |frame| to_f32(unsafe { plane.add(frame * stride).read() }))
}

/// Streaming linear-interpolation resampler. Voices are declared at their own
/// rate, so this only runs when a buffer disagrees with the declared format;
/// linear is enough for speech.
#[derive(Debug, Clone)]
pub(super) struct Resampler {
    /// Input samples per output sample; `None` passes samples through.
    step: Option<f64>,
    /// Time of the next output sample, in input samples after `prev`.
    pos: f64,
    prev: Option<f32>,
}

impl Resampler {
    pub fn new(input_hz: f64, output_hz: u32) -> Self {
        let output_hz = f64::from(output_hz);
        let differs = (input_hz - output_hz).abs() >= SAME_RATE_EPSILON_HZ && output_hz > 0.0;
        Self {
            step: differs.then(|| input_hz / output_hz),
            pos: 0.0,
            prev: None,
        }
    }

    pub fn push(&mut self, sample: f32, mut emit: impl FnMut(f32)) {
        let Some(step) = self.step else {
            return emit(sample);
        };
        if let Some(prev) = self.prev {
            while self.pos < 1.0 {
                emit(prev + (sample - prev) * self.pos as f32);
                self.pos += step;
            }
            self.pos -= 1.0;
        }
        self.prev = Some(sample);
    }
}

/// AVSpeechUtterance's rate is not a multiple: it runs from a minimum to a
/// maximum around a default. aulo's 0.5×..2× maps linearly onto it around the
/// default and is held inside the platform's bounds; `max`/`min` rather than
/// `clamp` so odd bounds from the OS can never panic.
pub(super) fn av_rate(rate: SpeechRate, default: f32, min: f32, max: f32) -> f32 {
    (default * rate.get()).max(min).min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deinterleaved_int16_scales_to_unit_range() {
        let mut plane = [i16::MIN, 0, -16_384, 16_384];
        // SAFETY: four samples at stride 1, alive for the call.
        let mono: Vec<f32> =
            unsafe { first_channel(NonNull::from(&mut plane[0]), 1, 4, i16_to_f32) }.collect();
        assert_eq!(mono, [-1.0, 0.0, -0.5, 0.5]);
    }

    #[test]
    fn interleaved_stereo_reads_the_first_channel_at_the_stride() {
        let mut data = [0.5_f32, -0.5, 1.0, 0.0];
        // SAFETY: two stereo frames at stride 2, alive for the call.
        let mono: Vec<f32> =
            unsafe { first_channel(NonNull::from(&mut data[0]), 2, 2, |s| s) }.collect();
        assert_eq!(mono, [0.5, 1.0]);
    }

    fn run(resampler: &mut Resampler, input: &[f32]) -> Vec<f32> {
        let mut out = Vec::new();
        for &sample in input {
            resampler.push(sample, |s| out.push(s));
        }
        out
    }

    #[test]
    fn equal_rates_pass_samples_through() {
        let input = [0.1, -0.2, 0.3];
        assert_eq!(run(&mut Resampler::new(22_050.0, 22_050), &input), input);
    }

    #[test]
    fn upsampling_interpolates_between_input_samples() {
        let out = run(&mut Resampler::new(16_000.0, 32_000), &[0.0, 1.0, 2.0, 3.0]);
        assert_eq!(out, [0.0, 0.5, 1.0, 1.5, 2.0, 2.5]);
    }

    #[test]
    fn resampling_keeps_the_duration_and_the_level() {
        let input = vec![0.25; 16_000];
        let out = run(&mut Resampler::new(16_000.0, 22_050), &input);
        assert!(out.len().abs_diff(22_050) <= 2, "{} samples", out.len());
        assert!(out.iter().all(|&s| (s - 0.25).abs() < 1e-6));
    }

    #[test]
    fn rate_maps_around_the_platform_default_within_its_bounds() {
        let (default, min, max) = (0.5, 0.0, 1.0);
        let at = |r| av_rate(SpeechRate::new(r).unwrap(), default, min, max);
        assert_eq!(at(1.0), 0.5);
        assert_eq!(at(0.5), 0.25);
        assert_eq!(at(2.0), 1.0);
        assert_eq!(av_rate(SpeechRate::NORMAL, 0.9, 0.0, 0.6), 0.6);
    }
}
