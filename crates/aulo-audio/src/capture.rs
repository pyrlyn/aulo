//! The capture seam. A platform implements [`InputBackend`] (cpal today); the
//! voice pipeline only sees [`Capture`], a stream of 20 ms 16 kHz mono frames
//! polled from a lock-free ring. Polling keeps the audio thread free of any
//! wake-up work: the consumer decides when to look.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use aulo_speech::{AudioFormat, AudioFrame};
use ringbuf::traits::{Consumer, Observer, Split};
use ringbuf::{HeapCons, HeapRb};

use crate::sink::{FRAME_SAMPLES, SampleSink};
use crate::{AudioError, MicPermissionProbe};

/// Frames the ring holds when the caller has no preference: 3 s of audio,
/// enough to ride out a busy consumer without letting the backlog grow stale.
pub const DEFAULT_RING_FRAMES: usize = 150;

/// Which microphone to open.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DeviceSelector {
    /// The system default input device.
    #[default]
    Default,
    /// The input device with exactly this name.
    Named(String),
}

impl DeviceSelector {
    /// From an optional config value; an absent or blank name means the default.
    pub fn from_config(name: Option<&str>) -> Self {
        match name.map(str::trim) {
            Some(name) if !name.is_empty() => Self::Named(name.to_owned()),
            _ => Self::Default,
        }
    }
}

/// Keeps a stream running; dropping it stops the stream.
pub type StreamGuard = Box<dyn Send>;

/// A source of input devices.
pub trait InputBackend {
    type Device: InputDevice;

    /// Finds the device without opening a stream, so a missing device is
    /// reported before anything records.
    fn find(&self, selector: &DeviceSelector) -> Result<Self::Device, AudioError>;
}

/// One input device, found but not streaming.
pub trait InputDevice {
    fn name(&self) -> &str;

    /// Native rate of the mono samples this device will deliver.
    fn sample_rate_hz(&self) -> u32;

    /// Starts delivering mono samples to `sink` from the platform's audio
    /// thread. The callback must neither block nor allocate.
    fn start(self, sink: SampleSink) -> Result<StreamGuard, AudioError>;
}

/// A running capture. Frames are 20 ms of 16 kHz mono `f32`.
pub struct Capture {
    consumer: HeapCons<f32>,
    overflow: Arc<AtomicU64>,
    frame: [f32; FRAME_SAMPLES],
    position: u64,
    device_name: String,
    _stream: StreamGuard,
}

impl std::fmt::Debug for Capture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Capture")
            .field("device_name", &self.device_name)
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}

impl Capture {
    /// [`Capture::start`] after asking `probe`: a refused or never-asked
    /// microphone fails with [`AudioError::Permission`] before any device is
    /// opened, instead of recording silence or prompting from the daemon.
    pub fn start_checked<P: MicPermissionProbe, B: InputBackend>(
        probe: &P,
        backend: &B,
        selector: &DeviceSelector,
        ring_frames: usize,
    ) -> Result<Self, AudioError> {
        let permission = probe.status();
        if permission.blocks_capture() {
            return Err(AudioError::Permission(permission));
        }
        Self::start(backend, selector, ring_frames)
    }

    /// Opens the selected device and starts streaming into a ring of
    /// `ring_frames` frames.
    pub fn start<B: InputBackend>(
        backend: &B,
        selector: &DeviceSelector,
        ring_frames: usize,
    ) -> Result<Self, AudioError> {
        // `HeapRb` panics on a zero capacity.
        let capacity = ring_frames
            .checked_mul(FRAME_SAMPLES)
            .filter(|c| *c > 0)
            .ok_or(AudioError::Invalid {
                what: "ring size",
                reason: "must hold at least one frame",
            })?;
        let device = backend.find(selector)?;
        let device_name = device.name().to_owned();
        let overflow = Arc::new(AtomicU64::new(0));
        let (producer, consumer) = HeapRb::<f32>::new(capacity).split();
        let sink = SampleSink::new(device.sample_rate_hz(), producer, Arc::clone(&overflow))?;
        let stream = device.start(sink)?;
        Ok(Self {
            consumer,
            overflow,
            frame: [0.0; FRAME_SAMPLES],
            position: 0,
            device_name,
            _stream: stream,
        })
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Samples (at 16 kHz) dropped so far because the ring was full. A drop
    /// loses whole frames, so the position of later frames runs behind the
    /// wall clock by this much.
    pub fn overflowed_samples(&self) -> u64 {
        self.overflow.load(Ordering::Relaxed)
    }

    /// Whole frames waiting in the ring.
    pub fn buffered_frames(&self) -> usize {
        self.consumer.occupied_len() / FRAME_SAMPLES
    }

    /// The next frame, or `None` when none is complete yet. The position
    /// counts delivered samples since the capture started.
    pub fn next_frame(&mut self) -> Option<AudioFrame<'_>> {
        // The producer only ever pushes whole frames, so the ring never holds
        // a partial one.
        if self.consumer.occupied_len() < FRAME_SAMPLES {
            return None;
        }
        self.consumer.pop_slice(&mut self.frame);
        let position = self.position;
        self.position += FRAME_SAMPLES as u64;
        // A whole mono frame is always valid, so this cannot be `None`.
        AudioFrame::new(&self.frame, AudioFormat::PIPELINE, position).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FRAME_MS;
    use crate::MicPermission;
    use crate::testkit::{FakeBackend, FakeDeviceSpec, FakeProbe};

    fn backend() -> FakeBackend {
        FakeBackend::new(vec![
            FakeDeviceSpec::new("Built-in Microphone", 16_000),
            FakeDeviceSpec::new("USB Headset", 48_000),
        ])
    }

    fn start(backend: &FakeBackend, ring_frames: usize) -> Capture {
        Capture::start(backend, &DeviceSelector::Default, ring_frames).unwrap()
    }

    fn tone(rate: u32, samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.5)
            .collect()
    }

    #[test]
    fn a_frame_is_20_ms_of_16_khz_mono() {
        assert_eq!(FRAME_MS, 20);
        assert_eq!(FRAME_SAMPLES, 320);
        let fake = backend();
        let mut capture = start(&fake, 4);
        fake.feed(&[0.25; FRAME_SAMPLES]);
        let frame = capture.next_frame().unwrap();
        assert_eq!(frame.format(), AudioFormat::PIPELINE);
        assert_eq!(frame.len(), FRAME_SAMPLES);
        assert_eq!(frame.at_ms(), 0);
    }

    #[test]
    fn the_default_selector_takes_the_first_device_and_a_name_picks_another() {
        let fake = backend();
        assert_eq!(start(&fake, 1).device_name(), "Built-in Microphone");
        let named = Capture::start(
            &fake,
            &DeviceSelector::from_config(Some(" USB Headset ")),
            1,
        );
        assert_eq!(named.unwrap().device_name(), "USB Headset");
        assert_eq!(
            fake.started_devices(),
            ["Built-in Microphone", "USB Headset"]
        );
    }

    #[test]
    fn a_blank_config_name_means_the_default() {
        assert_eq!(DeviceSelector::from_config(None), DeviceSelector::Default);
        assert_eq!(
            DeviceSelector::from_config(Some("  ")),
            DeviceSelector::Default
        );
    }

    #[test]
    fn a_missing_device_fails_before_any_stream_starts() {
        let fake = backend();
        let err = Capture::start(&fake, &DeviceSelector::Named("Nope".into()), 1).unwrap_err();
        assert!(matches!(err, AudioError::DeviceNotFound(_)), "{err}");
        let none = FakeBackend::new(Vec::new());
        let err = Capture::start(&none, &DeviceSelector::Default, 1).unwrap_err();
        assert_eq!(err, AudioError::NoInputDevice);
        assert!(fake.started_devices().is_empty() && !fake.is_streaming());
    }

    #[test]
    fn a_zero_ring_or_a_zero_rate_is_rejected() {
        let fake = backend();
        let err = Capture::start(&fake, &DeviceSelector::Default, 0).unwrap_err();
        assert!(matches!(err, AudioError::Invalid { .. }));
        let broken = FakeBackend::new(vec![FakeDeviceSpec::new("dead", 0)]);
        let err = Capture::start(&broken, &DeviceSelector::Default, 1).unwrap_err();
        assert!(matches!(err, AudioError::Invalid { .. }));
        assert!(Capture::start(&fake, &DeviceSelector::Default, usize::MAX).is_err());
    }

    #[test]
    fn samples_arrive_in_order_in_whole_frames_with_running_positions() {
        let fake = backend();
        let mut capture = start(&fake, 4);
        let input: Vec<f32> = (0..FRAME_SAMPLES * 2).map(|i| i as f32 / 1_000.0).collect();
        // A callback boundary in the middle of a frame must not lose or move samples.
        fake.feed(&input[..100]);
        assert!(capture.next_frame().is_none());
        fake.feed(&input[100..]);
        let first = capture.next_frame().unwrap();
        assert_eq!(
            (first.position(), first.samples()),
            (0, &input[..FRAME_SAMPLES])
        );
        let second = capture.next_frame().unwrap();
        assert_eq!(
            (second.position(), second.samples()),
            (FRAME_SAMPLES as u64, &input[FRAME_SAMPLES..])
        );
        assert!(capture.next_frame().is_none());
    }

    #[test]
    fn a_48_khz_device_is_resampled_to_16_khz_keeping_duration_and_level() {
        let fake = backend();
        let mut capture =
            Capture::start(&fake, &DeviceSelector::Named("USB Headset".into()), 100).unwrap();
        fake.feed(&tone(48_000, 48_000));
        let mut frames = 0;
        let mut peak = 0.0f32;
        while let Some(frame) = capture.next_frame() {
            frames += 1;
            peak = frame.samples().iter().fold(peak, |p, s| p.max(s.abs()));
        }
        assert!((49..=50).contains(&frames), "{frames} frames");
        assert!((0.45..=0.5).contains(&peak), "peak {peak}");
        assert_eq!(capture.overflowed_samples(), 0);
    }

    #[test]
    fn a_full_ring_drops_whole_frames_counts_them_and_keeps_the_oldest() {
        let fake = backend();
        let mut capture = start(&fake, 2);
        for value in 1..=5 {
            fake.feed(&[value as f32; FRAME_SAMPLES]);
        }
        assert_eq!(capture.buffered_frames(), 2);
        assert_eq!(capture.overflowed_samples(), 3 * FRAME_SAMPLES as u64);
        assert_eq!(capture.next_frame().unwrap().samples()[0], 1.0);
        assert_eq!(capture.next_frame().unwrap().samples()[0], 2.0);
        assert!(capture.next_frame().is_none());
        // Draining makes room again and the count stays put.
        fake.feed(&[6.0; FRAME_SAMPLES]);
        assert_eq!(capture.next_frame().unwrap().samples()[0], 6.0);
        assert_eq!(capture.overflowed_samples(), 3 * FRAME_SAMPLES as u64);
    }

    #[test]
    fn a_refused_microphone_fails_before_any_device_is_opened() {
        let fake = backend();
        for refusal in [
            MicPermission::Denied,
            MicPermission::Restricted,
            MicPermission::NotDetermined,
        ] {
            let probe = FakeProbe::new(refusal);
            let err =
                Capture::start_checked(&probe, &fake, &DeviceSelector::Default, 1).unwrap_err();
            assert_eq!(err, AudioError::Permission(refusal));
            assert_eq!(probe.asked(), 1);
        }
        assert!(fake.started_devices().is_empty() && !fake.is_streaming());
    }

    #[test]
    fn every_other_permission_starts_the_capture() {
        let fake = backend();
        for answer in [MicPermission::Granted, MicPermission::Unknown] {
            let probe = FakeProbe::new(answer);
            let capture = Capture::start_checked(&probe, &fake, &DeviceSelector::Default, 1);
            assert_eq!(capture.unwrap().device_name(), "Built-in Microphone");
        }
    }

    #[test]
    fn dropping_the_capture_stops_the_stream() {
        let fake = backend();
        let capture = start(&fake, 1);
        assert!(fake.is_streaming());
        drop(capture);
        assert!(!fake.is_streaming());
    }
}
