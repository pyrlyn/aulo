//! The playback seam. A platform implements [`OutputBackend`] (cpal today); the
//! voice pipeline only sees [`Playback`]: it writes the audio a TTS engine
//! polled out, sets the volume, stops on barge-in and reads how much has been
//! played. A fixed-size lock-free ring carries the samples to the audio thread,
//! which polls it, so the thread never waits on the pipeline.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use aulo_speech::{AudioFormat, AudioFrame};
use aulo_speech_system::Resampler;
use ringbuf::traits::{Observer, Producer, Split};
use ringbuf::{HeapProd, HeapRb};

use crate::AudioError;
use crate::capture::{DeviceSelector, StreamGuard};
use crate::source::{SampleSource, Shared};

/// Ring length when the caller has no preference: enough for a long sentence
/// that a TTS engine produces faster than real time.
pub const DEFAULT_BUFFER_MS: u32 = 2_000;
/// Longest ring accepted; a bigger one is a mistake, not a setting.
pub const MAX_BUFFER_MS: u32 = 30_000;

const MS_PER_SECOND: u64 = 1_000;

/// A source of output devices.
pub trait OutputBackend {
    type Device: OutputDevice;

    /// Finds the device without opening a stream, so a missing device is
    /// reported before anything plays.
    fn find(&self, selector: &DeviceSelector) -> Result<Self::Device, AudioError>;
}

/// One output device, found but not streaming.
pub trait OutputDevice {
    fn name(&self) -> &str;

    /// Native rate of the mono samples this device wants.
    fn sample_rate_hz(&self) -> u32;

    /// Starts pulling mono samples from `source` on the platform's audio
    /// thread. The callback must neither block nor allocate.
    fn start(self, source: SampleSource) -> Result<StreamGuard, AudioError>;
}

/// A running playback stream. Audio of any format goes in; mono at the device
/// rate comes out of the speaker.
pub struct Playback {
    producer: HeapProd<f32>,
    shared: Arc<Shared>,
    resampler: Resampler,
    /// The format `resampler` was built for; a new reply may arrive in another.
    input: Option<AudioFormat>,
    format: AudioFormat,
    /// Samples pushed into the ring since the start.
    written: u64,
    device_name: String,
    _stream: StreamGuard,
}

impl std::fmt::Debug for Playback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Playback")
            .field("device_name", &self.device_name)
            .field("written", &self.written)
            .finish_non_exhaustive()
    }
}

impl Playback {
    /// Opens the selected device and starts streaming silence, with a ring of
    /// `buffer_ms` milliseconds ([`DEFAULT_BUFFER_MS`] is a good choice).
    pub fn start<B: OutputBackend>(
        backend: &B,
        selector: &DeviceSelector,
        buffer_ms: u32,
    ) -> Result<Self, AudioError> {
        if !(1..=MAX_BUFFER_MS).contains(&buffer_ms) {
            return Err(AudioError::Invalid {
                what: "playback buffer",
                reason: "must be between 1 ms and 30 s",
            });
        }
        let device = backend.find(selector)?;
        let device_name = device.name().to_owned();
        let rate = device.sample_rate_hz();
        // The format check also rejects a zero rate, which would stall the resampler.
        let format = AudioFormat::new(rate, 1).map_err(|_| AudioError::Invalid {
            what: "output device",
            reason: "sample rate is zero",
        })?;
        let capacity = (u64::from(rate) * u64::from(buffer_ms) / MS_PER_SECOND).max(1);
        let capacity = usize::try_from(capacity).map_err(|_| AudioError::Invalid {
            what: "playback buffer",
            reason: "too large for this platform",
        })?;
        let shared = Arc::new(Shared::new(1.0));
        let (producer, consumer) = HeapRb::<f32>::new(capacity).split();
        let source = SampleSource::new(consumer, Arc::clone(&shared));
        let stream = device.start(source)?;
        Ok(Self {
            producer,
            shared,
            resampler: Resampler::new(f64::from(rate), rate),
            input: None,
            format,
            written: 0,
            device_name,
            _stream: stream,
        })
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// What the speaker plays: mono at the device's native rate. Positions
    /// below are in this format.
    pub fn format(&self) -> AudioFormat {
        self.format
    }

    /// Queues as much of `frame` as the ring has room for and returns how many
    /// of its frames (per-channel samples) that was. The caller keeps the rest
    /// and offers it again later: the ring is bounded, and waiting loses less
    /// than dropping half a sentence. Channels are averaged to mono and the
    /// rate is converted to the device's, here and not on the audio thread.
    pub fn write(&mut self, frame: AudioFrame<'_>) -> usize {
        let format = frame.format();
        // Each input frame yields at most ceil(device / input) samples; one more
        // covers float rounding at an exact ratio.
        let ratio = f64::from(self.format.sample_rate_hz()) / f64::from(format.sample_rate_hz());
        let per_frame = ratio.ceil() as usize + 1;
        if self.producer.vacant_len() < per_frame {
            return 0;
        }
        if self.input != Some(format) {
            self.resampler = Resampler::new(
                f64::from(format.sample_rate_hz()),
                self.format.sample_rate_hz(),
            );
            self.input = Some(format);
        }
        self.shared.open.store(true, Ordering::Relaxed);
        let channels = usize::from(format.channels());
        let (mut accepted, mut pushed) = (0, 0u64);
        for group in frame.samples().chunks_exact(channels) {
            if self.producer.vacant_len() < per_frame {
                break;
            }
            let mono = group.iter().sum::<f32>() / channels as f32;
            let producer = &mut self.producer;
            self.resampler.push(mono, |sample| {
                // The room was checked above, so a push cannot fail.
                if producer.try_push(sample).is_ok() {
                    pushed += 1;
                }
            });
            accepted += 1;
        }
        self.written += pushed;
        accepted
    }

    /// Marks the end of a reply: from here a dry ring is silence between
    /// replies, not an underrun. Queued audio still plays out.
    pub fn finish(&mut self) {
        self.shared.open.store(false, Ordering::Relaxed);
    }

    /// Barge-in: silences the speaker at the next audio callback and throws
    /// away everything queued. Audio written after this call plays normally.
    /// The speaker falls silent within one callback (a few milliseconds) plus
    /// whatever the driver already holds.
    pub fn stop(&mut self) {
        self.shared
            .discard_until
            .store(self.written, Ordering::Release);
        self.shared.open.store(false, Ordering::Relaxed);
        // The next reply may have another rate, and the resampler remembers a sample.
        self.input = None;
    }

    /// Sets the gain, from `0.0` (mute) to `1.0` (as written). It takes effect
    /// at the next audio callback, ramped across it so it does not click.
    pub fn set_volume(&self, volume: f32) -> Result<(), AudioError> {
        // The range check also rejects NaN, which compares false both ways.
        if !(0.0..=1.0).contains(&volume) {
            return Err(AudioError::Invalid {
                what: "volume",
                reason: "must be within 0.0..=1.0",
            });
        }
        self.shared
            .volume
            .store(volume.to_bits(), Ordering::Relaxed);
        Ok(())
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.shared.volume.load(Ordering::Relaxed))
    }

    /// Samples handed to the device so far, counted in [`format`](Self::format)
    /// and only for audio that came from the ring: silence and audio thrown
    /// away by `stop` do not count. It leads the sound by the driver's own
    /// latency, which is the same on every run; an echo canceller that needs
    /// the far end aligned with the microphone adds it.
    pub fn played_samples(&self) -> u64 {
        self.shared.played.load(Ordering::Relaxed)
    }

    /// [`played_samples`](Self::played_samples) in milliseconds: how much of
    /// the reply the user has heard, which is where a barge-in cut the text.
    pub fn played_ms(&self) -> u64 {
        self.format.position_to_ms(self.played_samples())
    }

    /// Samples waiting for the device, not counting audio `stop` has doomed.
    pub fn queued_samples(&self) -> u64 {
        let consumed = self.shared.consumed.load(Ordering::Acquire);
        let doomed = self.shared.discard_until.load(Ordering::Relaxed);
        self.written.saturating_sub(consumed.max(doomed))
    }

    /// Samples the device wanted in the middle of a reply that the ring did
    /// not have, heard as a gap. Grows when the TTS engine falls behind.
    pub fn underrun_samples(&self) -> u64 {
        self.shared.underrun.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::testkit::{FakeDeviceSpec, FakeOutputBackend};

    const RATE: u32 = 16_000;

    fn backend() -> FakeOutputBackend {
        FakeOutputBackend::new(vec![
            FakeDeviceSpec::new("Built-in Speakers", RATE),
            FakeDeviceSpec::new("USB Headset", 48_000),
        ])
    }

    fn start(fake: &FakeOutputBackend) -> Playback {
        Playback::start(fake, &DeviceSelector::Default, DEFAULT_BUFFER_MS).unwrap()
    }

    fn mono(samples: &[f32], rate: u32) -> AudioFrame<'_> {
        AudioFrame::new(samples, AudioFormat::new(rate, 1).unwrap(), 0).unwrap()
    }

    #[test]
    fn written_audio_plays_in_order_and_silence_follows() {
        let fake = backend();
        let mut playback = start(&fake);
        let samples: Vec<f32> = (1..=100).map(|i| i as f32 / 200.0).collect();
        assert_eq!(playback.write(mono(&samples, RATE)), 100);
        assert_eq!(playback.queued_samples(), 100);
        assert_eq!(fake.pull(60), samples[..60]);
        assert_eq!(playback.queued_samples(), 40);
        let rest = fake.pull(60);
        assert_eq!(rest[..40], samples[60..]);
        assert!(rest[40..].iter().all(|s| *s == 0.0));
        assert_eq!(
            (playback.played_samples(), playback.queued_samples()),
            (100, 0)
        );
    }

    #[test]
    fn a_stereo_reply_at_another_rate_is_averaged_and_converted_to_the_device_rate() {
        let fake = backend();
        let mut playback =
            Playback::start(&fake, &DeviceSelector::Named("USB Headset".into()), 1_000).unwrap();
        assert_eq!(playback.format(), AudioFormat::new(48_000, 1).unwrap());
        // One second of 24 kHz stereo: left 0.5, right 0.1, so mono 0.3.
        let stereo = AudioFormat::new(24_000, 2).unwrap();
        let samples: Vec<f32> = (0..24_000).flat_map(|_| [0.5, 0.1]).collect();
        let frame = AudioFrame::new(&samples, stereo, 0).unwrap();
        assert_eq!(playback.write(frame), 24_000);
        let out = fake.pull(48_000);
        let audible = out.iter().filter(|s| **s != 0.0).count();
        assert!(audible.abs_diff(48_000) <= 4, "{audible} samples");
        assert!(out[..audible].iter().all(|s| (s - 0.3).abs() < 1e-6));
        assert_eq!(playback.played_ms(), 1_000 * audible as u64 / 48_000);
    }

    #[test]
    fn a_new_reply_in_another_format_gets_its_own_conversion() {
        let fake = backend();
        let mut playback = start(&fake);
        playback.write(mono(&[0.5; 160], 8_000));
        playback.finish();
        playback.write(mono(&[0.25; 160], RATE));
        let out = fake.pull(600);
        let doubled = out.iter().filter(|s| **s == 0.5).count();
        assert!(doubled.abs_diff(320) <= 2, "{doubled}");
        assert_eq!(out.iter().filter(|s| **s == 0.25).count(), 160);
    }

    #[test]
    fn volume_scales_the_audio_and_ramps_across_the_buffer() {
        let fake = backend();
        let mut playback = start(&fake);
        playback.write(mono(&[1.0; 500], RATE));
        playback.set_volume(0.5).unwrap();
        let ramp = fake.pull(200);
        assert!(ramp[0] > 0.9 && (ramp[199] - 0.5).abs() < 1e-6, "{ramp:?}");
        assert!(ramp.windows(2).all(|w| w[1] <= w[0]));
        assert!(fake.pull(100).iter().all(|s| (s - 0.5).abs() < 1e-6));
        playback.set_volume(0.0).unwrap();
        assert_eq!(playback.volume(), 0.0);
        let fade = fake.pull(100);
        assert!(fade.windows(2).all(|w| w[1] <= w[0]) && fade[99].abs() < 1e-6);
        assert!(fake.pull(100).iter().all(|s| s.abs() < 1e-6));
        assert_eq!(playback.played_samples(), 500, "muted audio still plays");
    }

    #[test]
    fn volumes_outside_zero_to_one_or_nan_are_rejected() {
        let fake = backend();
        let playback = start(&fake);
        for volume in [-0.1, 1.01, f32::NAN, f32::INFINITY] {
            assert!(playback.set_volume(volume).is_err(), "{volume} accepted");
        }
        assert_eq!(playback.volume(), 1.0);
    }

    #[test]
    fn stop_silences_the_next_callback_and_drops_what_was_queued() {
        let fake = backend();
        let mut playback = start(&fake);
        playback.write(mono(&[0.7; 8_000], RATE));
        assert!(fake.pull(160).iter().all(|s| *s == 0.7));
        playback.stop();
        assert_eq!(playback.queued_samples(), 0);
        assert!(fake.pull(160).iter().all(|s| *s == 0.0));
        assert_eq!(playback.played_samples(), 160);
        assert!(fake.pull(8_000).iter().all(|s| *s == 0.0));
        assert_eq!(playback.played_samples(), 160);
    }

    #[test]
    fn audio_written_right_after_a_stop_survives_the_discard() {
        let fake = backend();
        let mut playback = start(&fake);
        playback.write(mono(&[0.7; 4_000], RATE));
        playback.stop();
        // The audio thread has not run since the stop.
        playback.write(mono(&[0.2; 100], RATE));
        assert_eq!(playback.queued_samples(), 100);
        let out = fake.pull(200);
        assert!(out[..100].iter().all(|s| *s == 0.2));
        assert!(out[100..].iter().all(|s| *s == 0.0));
        // Many stop and write rounds never lose or replay a sample.
        for round in 0..20 {
            let value = round as f32 / 100.0 + 0.01;
            playback.write(mono(&[0.9; 50], RATE));
            playback.stop();
            playback.write(mono(&[value; 30], RATE));
            let out = fake.pull(100);
            assert_eq!(out[..30], [value; 30], "round {round}");
            assert!(out[30..].iter().all(|s| *s == 0.0), "round {round}");
        }
    }

    /// Plays 10 ms buffers in real time on its own thread, as a sound card would.
    #[test]
    fn stop_silences_a_real_time_sink_within_50_ms() {
        let fake = backend();
        let mut playback = start(&fake);
        let running = Arc::new(AtomicBool::new(true));
        let last_audible = Arc::new(Mutex::new(Instant::now()));
        let sink = {
            let (fake, running, last_audible) = (
                fake.clone(),
                Arc::clone(&running),
                Arc::clone(&last_audible),
            );
            std::thread::spawn(move || {
                while running.load(Ordering::Relaxed) {
                    if fake.pull(160).iter().any(|s| *s != 0.0) {
                        *last_audible.lock().unwrap() = Instant::now();
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            })
        };
        // More audio than the sink can play, so it is busy when the stop comes.
        playback.write(mono(&[0.5; RATE as usize], RATE));
        std::thread::sleep(Duration::from_millis(120));
        let stopped_at = Instant::now();
        playback.stop();
        std::thread::sleep(Duration::from_millis(150));
        running.store(false, Ordering::Relaxed);
        sink.join().unwrap();

        let audible_after_stop = last_audible
            .lock()
            .unwrap()
            .saturating_duration_since(stopped_at);
        assert!(
            audible_after_stop < Duration::from_millis(50),
            "{audible_after_stop:?}"
        );
    }

    #[test]
    fn the_played_position_counts_only_audio_that_reached_the_device() {
        let fake = backend();
        let mut playback = start(&fake);
        fake.pull(500);
        assert_eq!(playback.played_samples(), 0);
        playback.write(mono(&[0.1; 1_600], RATE));
        fake.pull(800);
        assert_eq!((playback.played_samples(), playback.played_ms()), (800, 50));
        playback.stop();
        fake.pull(800);
        assert_eq!(playback.played_ms(), 50);
        playback.write(mono(&[0.1; 800], RATE));
        fake.pull(800);
        assert_eq!(playback.played_ms(), 100);
    }

    #[test]
    fn a_dry_ring_mid_reply_counts_an_underrun_but_not_between_replies() {
        let fake = backend();
        let mut playback = start(&fake);
        fake.pull(100);
        assert_eq!(playback.underrun_samples(), 0, "idle is not an underrun");
        playback.write(mono(&[0.1; 100], RATE));
        fake.pull(160);
        assert_eq!(playback.underrun_samples(), 60);
        playback.write(mono(&[0.1; 100], RATE));
        playback.finish();
        fake.pull(160);
        assert_eq!(playback.underrun_samples(), 60, "the end of a reply");
        playback.write(mono(&[0.1; 100], RATE));
        playback.stop();
        fake.pull(160);
        assert_eq!(playback.underrun_samples(), 60, "a stop");
    }

    #[test]
    fn a_full_ring_accepts_a_prefix_and_loses_nothing() {
        let fake = backend();
        // 100 ms at 16 kHz: 1 600 samples.
        let mut playback = Playback::start(&fake, &DeviceSelector::Default, 100).unwrap();
        let samples: Vec<f32> = (1..=5_000).map(|i| i as f32).collect();
        let first = playback.write(mono(&samples, RATE));
        assert!((1..samples.len()).contains(&first), "{first}");
        let mut offered = first;
        let mut heard = Vec::new();
        while offered < samples.len() {
            heard.extend(fake.pull(400));
            offered += playback.write(mono(&samples[offered..], RATE));
        }
        heard.extend(fake.pull(1_600));
        heard.retain(|s| *s != 0.0);
        assert_eq!(heard, samples);
    }

    #[test]
    fn bad_devices_and_buffers_fail_before_any_stream_starts() {
        let fake = backend();
        let err = Playback::start(&fake, &DeviceSelector::Named("Nope".into()), 100).unwrap_err();
        assert!(matches!(err, AudioError::OutputDeviceNotFound(_)), "{err}");
        let none = FakeOutputBackend::new(Vec::new());
        let err = Playback::start(&none, &DeviceSelector::Default, 100).unwrap_err();
        assert_eq!(err, AudioError::NoOutputDevice);
        for ms in [0, MAX_BUFFER_MS + 1] {
            let err = Playback::start(&fake, &DeviceSelector::Default, ms).unwrap_err();
            assert!(matches!(err, AudioError::Invalid { .. }));
        }
        let dead = FakeOutputBackend::new(vec![FakeDeviceSpec::new("dead", 0)]);
        let err = Playback::start(&dead, &DeviceSelector::Default, 100).unwrap_err();
        assert!(matches!(err, AudioError::Invalid { .. }));
        assert!(fake.started_devices().is_empty() && !fake.is_streaming());
    }

    #[test]
    fn dropping_the_playback_stops_the_stream() {
        let fake = backend();
        let playback = start(&fake);
        assert!(fake.is_streaming());
        drop(playback);
        assert!(!fake.is_streaming());
    }
}
