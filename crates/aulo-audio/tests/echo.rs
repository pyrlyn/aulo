//! Far-end tone through AEC3 (T6.5). The microphone frame is that tone delayed
//! and scaled, the way a speaker leaks into a mic. After the canceller settles,
//! the last second must be quieter than the echo by the target below.
#![cfg(feature = "aec3")]
// The shared helper sits outside `#[test]` functions, where clippy still denies `unwrap`.
#![allow(clippy::unwrap_used, clippy::expect_used)]
// The fixture only runs on Linux; elsewhere its helpers and imports are unused.
#![cfg_attr(not(target_os = "linux"), allow(unused_imports, dead_code))]

use std::f32::consts::TAU;

use aulo_audio::{ECHO_FRAME_SAMPLES, ECHO_SAMPLE_RATE_HZ, EchoCanceller};

/// Playback-to-microphone delay of the fixture. AEC3 is told the same number.
const DELAY_MS: u16 = 100;
const DELAY_FRAMES: usize = DELAY_MS as usize / 10;
/// Let the filter and the delay estimate settle before measuring.
const WARMUP_FRAMES: usize = 200;
/// One second of steady state.
const MEASURE_FRAMES: usize = 100;
const TONE_HZ: f32 = 440.0;
const TONE_AMPLITUDE: f32 = 0.4;
const ECHO_GAIN: f32 = 0.5;

#[cfg(target_os = "linux")]
// Floor for the fixture. A 100 ms delayed 440 Hz tone measured 42.7 dB ERLE
// here, with the delay both hinted and estimated. 20 dB leaves room for
// another architecture without accepting a canceller that barely moves.
const MIN_ERLE_DB: f32 = 20.0;

#[cfg(target_os = "linux")]
fn tone(frame_index: usize) -> [f32; ECHO_FRAME_SAMPLES] {
    let mut frame = [0.0; ECHO_FRAME_SAMPLES];
    for (i, sample) in frame.iter_mut().enumerate() {
        let t = (frame_index * ECHO_FRAME_SAMPLES + i) as f32;
        let phase = t * TONE_HZ * TAU / ECHO_SAMPLE_RATE_HZ as f32;
        *sample = phase.sin() * TONE_AMPLITUDE;
    }
    frame
}

#[cfg(target_os = "linux")]
fn energy(frame: &[f32]) -> f32 {
    frame.iter().map(|sample| sample * sample).sum()
}

#[cfg(target_os = "linux")]
fn erle(delay_ms: Option<u16>) -> f32 {
    let canceller = EchoCanceller::with_delay(delay_ms).expect("canceller");
    let mut history = [[0.0; ECHO_FRAME_SAMPLES]; DELAY_FRAMES];
    let mut echo_energy = 0.0_f32;
    let mut out_energy = 0.0_f32;
    let total = WARMUP_FRAMES + MEASURE_FRAMES;
    for n in 0..total {
        let far = tone(n);
        canceller.render(&far).expect("render");
        let mut near = history[n % DELAY_FRAMES];
        for sample in &mut near {
            *sample *= ECHO_GAIN;
        }
        history[n % DELAY_FRAMES] = far;
        if n >= WARMUP_FRAMES {
            echo_energy += energy(&near);
            canceller.capture(&mut near).expect("capture");
            out_energy += energy(&near);
        } else {
            canceller.capture(&mut near).expect("capture");
        }
    }
    assert!(
        echo_energy > 1.0,
        "the fixture echo is silent ({echo_energy})"
    );
    if out_energy <= 1e-12 {
        120.0
    } else {
        10.0 * (echo_energy / out_energy).log10()
    }
}

#[cfg(target_os = "linux")]
#[test]
fn far_end_tone_is_attenuated() {
    let known = erle(Some(DELAY_MS));
    let estimated = erle(None);
    assert!(
        known >= MIN_ERLE_DB,
        "hinted delay ERLE {known:.1} dB, want at least {MIN_ERLE_DB} dB"
    );
    assert!(
        estimated >= MIN_ERLE_DB,
        "estimated delay ERLE {estimated:.1} dB, want at least {MIN_ERLE_DB} dB"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn render_leaves_the_reference_unchanged() {
    let canceller = EchoCanceller::new().expect("canceller");
    let far = tone(0);
    let before = far;
    canceller.render(&far).expect("render");
    assert_eq!(far, before);
}

#[cfg(target_os = "linux")]
#[test]
fn rejects_a_short_frame_and_a_non_finite_sample() {
    let canceller = EchoCanceller::new().expect("canceller");
    let err = canceller.render(&[0.0; 10]).expect_err("short");
    assert!(err.to_string().contains("160"));
    let mut bad = [0.0; ECHO_FRAME_SAMPLES];
    bad[3] = f32::NAN;
    let err = canceller.capture(&mut bad).expect_err("nan");
    assert!(err.to_string().contains("finite"));
}

#[cfg(not(target_os = "linux"))]
#[test]
fn aec3_is_not_built_off_linux() {
    let err = EchoCanceller::new().expect_err("unavailable");
    assert!(err.to_string().contains("Linux"));
}
