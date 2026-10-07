//! Manual checks against the real default microphone and speaker; ignored by
//! default because CI has neither. On macOS the first capture run asks the
//! terminal for microphone access. The playback check makes a quiet tone.
//!
//! `cargo test -p aulo-audio --test real_device -- --ignored --nocapture --test-threads=1`

use std::time::{Duration, Instant};

use aulo_audio::{
    Capture, CpalBackend, DEFAULT_BUFFER_MS, DEFAULT_RING_FRAMES, DeviceSelector, Playback,
};
use aulo_speech::{AudioFormat, AudioFrame};

#[test]
#[ignore = "needs a microphone"]
fn the_default_microphone_delivers_16_khz_frames() {
    let mut capture =
        Capture::start(&CpalBackend, &DeviceSelector::Default, DEFAULT_RING_FRAMES).unwrap();
    println!("device: {}", capture.device_name());

    let mut frames = 0u32;
    let mut energy = 0.0f64;
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        while let Some(frame) = capture.next_frame() {
            frames += 1;
            energy += frame
                .samples()
                .iter()
                .map(|s| f64::from(s * s))
                .sum::<f64>();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let rms = (energy / f64::from(frames.max(1) * 320)).sqrt();
    println!(
        "{frames} frames in 1 s, rms {rms:.4}, overflow {}",
        capture.overflowed_samples()
    );

    // About 50 frames a second; allow for the stream starting late.
    assert!((35..=55).contains(&frames), "{frames} frames");
    assert_eq!(capture.overflowed_samples(), 0);
}

#[test]
#[ignore = "plays a quiet tone on the default speaker"]
fn the_default_speaker_plays_a_tone_and_stops_on_demand() {
    let mut playback =
        Playback::start(&CpalBackend, &DeviceSelector::Default, DEFAULT_BUFFER_MS).unwrap();
    println!("device: {}", playback.device_name());
    playback.set_volume(0.1).unwrap();

    // Three seconds of 440 Hz at the pipeline rate, so the rate conversion runs too.
    let tone: Vec<f32> = (0..48_000)
        .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 16_000.0).sin() * 0.5)
        .collect();
    let frame = AudioFrame::new(&tone, AudioFormat::PIPELINE, 0).unwrap();
    assert_eq!(playback.write(frame), tone.len());

    std::thread::sleep(Duration::from_millis(800));
    let before_stop = playback.played_ms();
    println!("played {before_stop} ms after 800 ms");
    assert!(
        (400..=900).contains(&before_stop),
        "played {before_stop} ms"
    );

    playback.stop();
    std::thread::sleep(Duration::from_millis(300));
    let after_stop = playback.played_ms();
    println!(
        "played {after_stop} ms after the stop, underruns {}",
        playback.underrun_samples()
    );
    // A stop takes effect within one callback, so at most a few buffers more play.
    assert!(
        after_stop - before_stop <= 100,
        "kept playing {after_stop} ms"
    );
    assert_eq!(playback.queued_samples(), 0);
}
