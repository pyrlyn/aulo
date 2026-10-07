//! Sample math between AVFoundation buffers and the engine's output: reading
//! a buffer as mono and mapping aulo's speech rate onto AVSpeechUtterance's.
//! Kept apart from the FFI so every step is unit-tested with plain Rust data.

use std::ptr::NonNull;

use aulo_speech::SpeechRate;

/// Full scale of signed 16-bit PCM.
const I16_SCALE: f32 = 32_768.0;

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
