//! Manual check against the real default microphone; ignored by default
//! because CI has none. On macOS the first run asks the terminal for
//! microphone access.
//!
//! `cargo test -p aulo-audio --test real_device -- --ignored --nocapture`

use std::time::{Duration, Instant};

use aulo_audio::{Capture, CpalBackend, DEFAULT_RING_FRAMES, DeviceSelector};

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
