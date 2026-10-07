//! Decoding the WAV stream a system synthesizer writes into mono samples at
//! the engine's rate. The header says what the voice produced, so nothing about
//! it is assumed: voices differ, and a wrong rate plays at the wrong pitch.

#![cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]

use std::io::Read;

use aulo_speech::SpeechError;
use hound::{SampleFormat, WavReader};

use crate::resample::Resampler;

/// Rates outside these are not speech; the floor also bounds how many output
/// samples one input sample can become.
const MIN_RATE_HZ: u32 = 8_000;
const MAX_RATE_HZ: u32 = 192_000;
/// Full scale of signed 16-bit PCM.
const I16_SCALE: f32 = 32_768.0;

/// Reads the header of `reader` and accepts only 16-bit PCM in a usable
/// format. `source` names the synthesizer in error messages. Wrap `reader` in a
/// `BufReader` first: hound reads a sample at a time.
pub(crate) fn open<R: Read>(reader: R, source: &str) -> Result<WavReader<R>, SpeechError> {
    let wav = WavReader::new(reader).map_err(|_| damaged(source, "no valid WAV header"))?;
    let spec = wav.spec();
    if spec.sample_format != SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err(damaged(source, "audio that is not 16-bit PCM"));
    }
    if spec.channels == 0 || !(MIN_RATE_HZ..=MAX_RATE_HZ).contains(&spec.sample_rate) {
        return Err(damaged(source, "an unusable audio format"));
    }
    Ok(wav)
}

/// Streams `wav` through `sink` as mono f32 at `output_hz`, at most
/// `chunk_samples` input samples at a time, so memory use is one chunk however
/// long the sentence is and a cancel is noticed between chunks. The sink
/// returns false to stop. Returns whether the stream ran to its end. A
/// multi-channel stream contributes its first channel.
///
/// With `pipe`, a read error ends the stream quietly: a pipe cannot know its
/// length, so its end is a short read that hound reports as an I/O error, and
/// the caller tells a finished child from a dead one by its exit status.
pub(crate) fn stream<R: Read>(
    mut wav: WavReader<R>,
    source: &str,
    output_hz: u32,
    chunk_samples: usize,
    pipe: bool,
    mut sink: impl FnMut(&[f32]) -> bool,
) -> Result<bool, SpeechError> {
    let spec = wav.spec();
    let channels = usize::from(spec.channels);
    let mut resampler = Resampler::new(f64::from(spec.sample_rate), output_hz);
    let expansion = output_hz.div_ceil(MIN_RATE_HZ) as usize + 1;
    let mut out = Vec::with_capacity(chunk_samples * expansion);
    let mut taken = 0;
    for (index, sample) in wav.samples::<i16>().enumerate() {
        let sample = match sample {
            Ok(sample) => sample,
            Err(hound::Error::IoError(_)) if pipe => break,
            Err(_) => return Err(damaged(source, "a damaged sample stream")),
        };
        if index % channels != 0 {
            continue;
        }
        resampler.push(f32::from(sample) / I16_SCALE, |s| out.push(s));
        taken += 1;
        if taken == chunk_samples {
            if !sink(&out) {
                return Ok(false);
            }
            out.clear();
            taken = 0;
        }
    }
    Ok(out.is_empty() || sink(&out))
}

fn damaged(source: &str, what: &str) -> SpeechError {
    SpeechError::failed(&format!("{source} wrote {what}"))
}

/// A 16-bit PCM WAV file, as a synthesizer would return one.
#[cfg(test)]
pub(crate) fn wav_bytes(rate: u32, channels: u16, frames: &[i16]) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut bytes = std::io::Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut bytes, spec).unwrap();
    for &s in frames {
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();
    bytes.into_inner()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use hound::{WavSpec, WavWriter};

    use super::*;

    const CHUNK: usize = 1024;

    fn decode(
        bytes: Vec<u8>,
        hz: u32,
        sink: impl FnMut(&[f32]) -> bool,
    ) -> Result<bool, SpeechError> {
        stream(
            open(Cursor::new(bytes), "test")?,
            "test",
            hz,
            CHUNK,
            false,
            sink,
        )
    }

    fn collect(bytes: Vec<u8>, hz: u32) -> Result<Vec<f32>, SpeechError> {
        let mut all = Vec::new();
        decode(bytes, hz, |chunk| {
            all.extend_from_slice(chunk);
            true
        })
        .map(|_| all)
    }

    #[test]
    fn samples_scale_to_unit_range_at_the_header_rate() {
        let bytes = wav_bytes(22_050, 1, &[i16::MIN, 0, 16_384]);
        assert_eq!(collect(bytes, 22_050).unwrap(), [-1.0, 0.0, 0.5]);
    }

    #[test]
    fn a_different_header_rate_is_resampled_to_the_declared_one() {
        let bytes = wav_bytes(16_000, 1, &vec![8_192; 16_000]);
        let out = collect(bytes, 22_050).unwrap();
        assert!(out.len().abs_diff(22_050) <= 2, "{} samples", out.len());
        assert!(out.iter().all(|&s| (s - 0.25).abs() < 1e-6));
    }

    #[test]
    fn extra_channels_are_dropped() {
        let bytes = wav_bytes(22_050, 2, &[16_384, 1, -16_384, 2]);
        assert_eq!(collect(bytes, 22_050).unwrap(), [0.5, -0.5]);
    }

    #[test]
    fn output_arrives_in_bounded_chunks_and_the_sink_can_stop_it() {
        let bytes = wav_bytes(22_050, 1, &vec![1; CHUNK * 2 + 5]);
        let mut sizes = Vec::new();
        let ended = decode(bytes.clone(), 22_050, |c| {
            sizes.push(c.len());
            true
        });
        assert!(ended.unwrap());
        assert_eq!(sizes, [CHUNK, CHUNK, 5]);
        let ended = decode(bytes, 22_050, |_| false);
        assert!(!ended.unwrap());
    }

    #[test]
    fn damaged_input_is_a_failure_not_a_panic() {
        assert!(collect(b"not a wav file at all".to_vec(), 22_050).is_err());
        let mut truncated = wav_bytes(22_050, 1, &[1, 2, 3, 4]);
        truncated.truncate(truncated.len() - 3);
        assert!(collect(truncated, 22_050).is_err());
        assert!(collect(wav_bytes(1_000, 1, &[1]), 22_050).is_err());
        let float = Cursor::new(Vec::new());
        let spec = WavSpec {
            channels: 1,
            sample_rate: 22_050,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        };
        let mut bytes = float;
        let mut writer = WavWriter::new(&mut bytes, spec).unwrap();
        writer.write_sample(0.5_f32).unwrap();
        writer.finalize().unwrap();
        assert!(collect(bytes.into_inner(), 22_050).is_err());
    }
}
