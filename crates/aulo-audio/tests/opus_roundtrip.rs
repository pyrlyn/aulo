//! Opus round trip of 20 ms PCM16 frames (T6.7): the decoded signal must stay
//! close to the input, and the decoder must refuse hostile packets.
#![cfg(feature = "opus")]
// The shared helpers sit outside `#[test]` functions, where clippy still denies `unwrap`.
#![allow(clippy::unwrap_used)]

use std::f32::consts::TAU;

use aulo_audio::{
    DEFAULT_BITRATE_BPS, FRAME_SAMPLES, MAX_FRAMES_PER_PACKET, MAX_PACKET_BYTES,
    MAX_PACKET_SAMPLES, OpusDecoder, OpusEncoder, OpusError,
};

const SAMPLE_RATE: f32 = 16_000.0;
/// Frames to leave out of the SNR while the codec settles.
const WARMUP_FRAMES: usize = 5;

/// The minimum SNR for a round trip at the default bitrate. Opus is a
/// perceptual codec, so waveform SNR is modest by design: on this signal it
/// measures 10.4 dB at 24 kbit/s and stays between 9.8 and 10.5 dB from 12 to
/// 64 kbit/s, so the codec, not the rate, sets the ceiling. The target leaves
/// about 2 dB of margin for libopus updates and platform float differences.
const MIN_SNR_DB: f64 = 8.0;

/// Voiced speech stand-in: a 120 Hz pulse train with vibrato, shaped by three
/// formant resonators and a 4 Hz syllable envelope. Deterministic, so the test
/// needs no fixture file.
fn speech_like(seconds: f32) -> Vec<i16> {
    let total = (seconds * SAMPLE_RATE) as usize;
    let formants = [
        (700.0_f32, 0.96_f32, 1.0_f32),
        (1200.0, 0.95, 0.6),
        (2600.0, 0.93, 0.3),
    ];
    let mut state = [[0.0_f32; 2]; 3];
    let mut phase = 0.0_f32;
    let mut out = Vec::with_capacity(total);
    for n in 0..total {
        let t = n as f32 / SAMPLE_RATE;
        phase += (120.0 + 8.0 * (TAU * 5.0 * t).sin()) / SAMPLE_RATE;
        let pulse = if phase.fract() < 0.02 { 1.0 } else { 0.0 };
        phase = phase.fract() + phase.floor();
        let mut sample = 0.0;
        for ((freq, radius, gain), s) in formants.iter().zip(state.iter_mut()) {
            let y = pulse + 2.0 * radius * (TAU * freq / SAMPLE_RATE).cos() * s[0]
                - radius * radius * s[1];
            s[1] = s[0];
            s[0] = y;
            sample += gain * y * (1.0 - radius);
        }
        let envelope = 0.55 + 0.45 * (TAU * 4.0 * t).sin();
        out.push((sample * envelope * 9_000.0) as i16);
    }
    out
}

fn round_trip(input: &[i16], bitrate: u32) -> (Vec<i16>, usize, usize) {
    let mut encoder = OpusEncoder::new(bitrate).unwrap();
    let mut decoder = OpusDecoder::new().unwrap();
    let delay = encoder.lookahead().unwrap();
    let mut output = Vec::with_capacity(input.len());
    let mut bytes = 0;
    for frame in input.as_chunks::<FRAME_SAMPLES>().0 {
        let packet = encoder.encode(frame).unwrap();
        bytes += packet.len();
        output.extend_from_slice(decoder.decode(packet).unwrap());
    }
    (output, delay, bytes)
}

/// SNR in dB of `output` against `input` delayed by the codec's lookahead.
fn snr_db(input: &[i16], output: &[i16], delay: usize) -> f64 {
    let skip = WARMUP_FRAMES * FRAME_SAMPLES;
    let (mut signal, mut noise) = (0.0_f64, 0.0_f64);
    for n in skip..input.len() - delay {
        let reference = f64::from(input[n]);
        let error = reference - f64::from(output[n + delay]);
        signal += reference * reference;
        noise += error * error;
    }
    10.0 * (signal / noise).log10()
}

#[test]
fn round_trip_keeps_the_signal_within_the_snr_target() {
    let input = speech_like(3.0);
    let (output, delay, bytes) = round_trip(&input, DEFAULT_BITRATE_BPS);
    assert_eq!(output.len(), input.len());
    let snr = snr_db(&input, &output, delay);
    let kbps = bytes as f64 * 8.0 / 3.0 / 1000.0;
    println!("opus round trip: lookahead {delay} samples, {kbps:.1} kbit/s, SNR {snr:.1} dB");
    assert!(
        snr >= MIN_SNR_DB,
        "SNR {snr:.1} dB is below {MIN_SNR_DB} dB"
    );
}

#[test]
fn opus_is_much_smaller_than_pcm16() {
    let input = speech_like(1.0);
    let (_, _, bytes) = round_trip(&input, DEFAULT_BITRATE_BPS);
    assert!(
        bytes * 8 < input.len() * 2,
        "opus should be under a quarter of PCM16"
    );
}

#[test]
fn encoder_rejects_a_frame_of_the_wrong_size() {
    let mut encoder = OpusEncoder::new(DEFAULT_BITRATE_BPS).unwrap();
    assert_eq!(
        encoder.encode(&[0; FRAME_SAMPLES - 1]).unwrap_err(),
        OpusError::FrameSize(FRAME_SAMPLES - 1)
    );
}

#[test]
fn decoder_rejects_an_empty_packet() {
    let mut decoder = OpusDecoder::new().unwrap();
    assert_eq!(decoder.decode(&[]).unwrap_err(), OpusError::EmptyPacket);
}

#[test]
fn decoder_caps_the_packet_size() {
    let mut decoder = OpusDecoder::new().unwrap();
    let packet = vec![0_u8; MAX_PACKET_BYTES + 1];
    assert_eq!(
        decoder.decode(&packet).unwrap_err(),
        OpusError::PacketTooLarge(MAX_PACKET_BYTES + 1)
    );
}

#[test]
fn decoder_caps_the_frames_per_packet() {
    // TOC 0x03 is code 3 (an arbitrary number of frames of the same size), and
    // the next byte holds the frame count in its low six bits.
    let frames = MAX_FRAMES_PER_PACKET + 1;
    let mut packet = vec![0x03, u8::try_from(frames).unwrap()];
    packet.extend(std::iter::repeat_n(0, frames));
    let mut decoder = OpusDecoder::new().unwrap();
    assert_eq!(
        decoder.decode(&packet).unwrap_err(),
        OpusError::TooManyFrames(frames)
    );
}

#[test]
fn decoder_accepts_a_packet_at_the_frame_cap() {
    let frames = MAX_FRAMES_PER_PACKET;
    let mut packet = vec![0x03, u8::try_from(frames).unwrap()];
    packet.extend(std::iter::repeat_n(0, frames));
    let mut decoder = OpusDecoder::new().unwrap();
    // TOC config 0 is a 10 ms frame, so six of them are 60 ms.
    assert_eq!(decoder.decode(&packet).unwrap().len(), frames * 160);
}

#[test]
fn decoder_survives_arbitrary_bytes() {
    let mut decoder = OpusDecoder::new().unwrap();
    let mut seed = 0x2545_f491_4f6c_dd1d_u64;
    for len in 1..=MAX_PACKET_BYTES {
        let packet: Vec<u8> = (0..len)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (seed >> 32) as u8
            })
            .collect();
        // Either an error or audio within the fixed buffer; never a panic.
        if let Ok(samples) = decoder.decode(&packet) {
            assert!(samples.len() <= MAX_PACKET_SAMPLES);
        }
    }
}
