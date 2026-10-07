//! The consumer half of the playback ring: what runs on the platform's audio
//! thread. It never blocks and never allocates: it pops into the slice the
//! driver gave it and talks to the rest of the program through atomics only.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use ringbuf::HeapCons;
use ringbuf::traits::Consumer;

/// State the audio thread and the owner of the [`Playback`](crate::Playback)
/// both touch. Plain atomics, so neither side ever waits for the other.
#[derive(Debug)]
pub(crate) struct Shared {
    /// Output gain as `f32` bits.
    pub(crate) volume: AtomicU32,
    /// Samples the ring was handed when `stop` ran. The audio thread throws
    /// away everything before that count, so audio written after a stop is
    /// never mistaken for audio to discard.
    pub(crate) discard_until: AtomicU64,
    /// Samples taken out of the ring so far, played or discarded.
    pub(crate) consumed: AtomicU64,
    /// Samples handed to the device that came from the ring.
    pub(crate) played: AtomicU64,
    /// Samples the device asked for in the middle of a reply and the ring did
    /// not have.
    pub(crate) underrun: AtomicU64,
    /// A reply is being written; a dry ring counts as an underrun only then.
    pub(crate) open: AtomicBool,
}

impl Shared {
    pub(crate) fn new(volume: f32) -> Self {
        Self {
            volume: AtomicU32::new(volume.to_bits()),
            discard_until: AtomicU64::new(0),
            consumed: AtomicU64::new(0),
            played: AtomicU64::new(0),
            underrun: AtomicU64::new(0),
            open: AtomicBool::new(false),
        }
    }
}

/// Fills the driver's buffers with mono samples at the device rate. A device
/// calls [`SampleSource::fill`] from its audio callback.
pub struct SampleSource {
    consumer: HeapCons<f32>,
    shared: Arc<Shared>,
    consumed: u64,
    /// Gain at the end of the previous buffer, so a volume change ramps over
    /// one buffer instead of clicking.
    gain: f32,
}

impl std::fmt::Debug for SampleSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleSource")
            .field("consumed", &self.consumed)
            .finish_non_exhaustive()
    }
}

impl SampleSource {
    pub(crate) fn new(consumer: HeapCons<f32>, shared: Arc<Shared>) -> Self {
        let gain = f32::from_bits(shared.volume.load(Ordering::Relaxed));
        Self {
            consumer,
            shared,
            consumed: 0,
            gain,
        }
    }

    /// Writes `out.len()` mono samples: queued audio at the current volume,
    /// then silence for whatever the ring could not supply.
    pub fn fill(&mut self, out: &mut [f32]) {
        self.discard_stopped_audio();
        let got = self.consumer.pop_slice(out);
        let (audio, silence) = out.split_at_mut(got);
        silence.fill(0.0);
        self.apply_volume(audio);

        let got = got as u64;
        self.consumed += got;
        self.shared.consumed.store(self.consumed, Ordering::Release);
        self.shared.played.fetch_add(got, Ordering::Relaxed);
        if !silence.is_empty() && self.shared.open.load(Ordering::Relaxed) {
            self.shared
                .underrun
                .fetch_add(silence.len() as u64, Ordering::Relaxed);
        }
    }

    fn discard_stopped_audio(&mut self) {
        let until = self.shared.discard_until.load(Ordering::Acquire);
        if until > self.consumed {
            let wanted = usize::try_from(until - self.consumed).unwrap_or(usize::MAX);
            self.consumed += self.consumer.skip(wanted) as u64;
            self.shared.consumed.store(self.consumed, Ordering::Release);
        }
    }

    fn apply_volume(&mut self, audio: &mut [f32]) {
        let target = f32::from_bits(self.shared.volume.load(Ordering::Relaxed));
        if audio.is_empty() {
            self.gain = target;
            return;
        }
        let step = (target - self.gain) / audio.len() as f32;
        for sample in audio {
            self.gain += step;
            *sample *= self.gain;
        }
        self.gain = target;
    }
}
