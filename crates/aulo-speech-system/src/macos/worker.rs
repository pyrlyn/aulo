//! The thread that owns `AVSpeechSynthesizer`, and the callback that turns
//! its buffers into samples.
//!
//! Threading, as measured on macOS 27 (arm64):
//! - The synthesizer is not `Send`, so one worker thread creates it and makes
//!   every call on it. `writeUtterance` returns at once.
//! - The buffer callback runs on the **main** dispatch queue, never on the
//!   worker, even when the worker runs its own run loop. The host keeps the
//!   main run loop alive (see the crate docs).
//! - Synthesis runs several times faster than real time, and
//!   `stopSpeakingAtBoundary` does not stop buffers already being written.
//!   So the worker feeds the synthesizer one sentence at a time and only when
//!   the ring has room, and cancel works by generation: a callback whose
//!   generation is stale writes nothing.

use std::collections::VecDeque;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use aulo_speech::SpeechRate;
use block2::RcBlock;
use objc2_avf_audio::{
    AVAudioBuffer, AVAudioCommonFormat, AVAudioPCMBuffer, AVSpeechBoundary, AVSpeechSynthesisVoice,
    AVSpeechSynthesizer, AVSpeechUtterance, AVSpeechUtteranceDefaultSpeechRate as DEFAULT_RATE,
    AVSpeechUtteranceMaximumSpeechRate as MAX_RATE, AVSpeechUtteranceMinimumSpeechRate as MIN_RATE,
};
use objc2_foundation::NSString;
use ringbuf::traits::{Observer, Producer};
use ringbuf::{HeapProd, HeapRb, Obs};

use super::convert::{av_rate, first_channel, i16_to_f32};
use crate::resample::Resampler;

/// How often a worker with queued sentences looks for ring space again.
const DISPATCH_TICK: Duration = Duration::from_millis(10);
/// How long an idle worker sleeps between checks; it wakes on any command.
const IDLE_WAIT: Duration = Duration::from_secs(1);

/// The producer side of the ring and what the callback needs to fill it. A
/// mutex guards it so a cancel can fence off callbacks of the old generation.
/// Only the callback and the engine's `begin`/`cancel` take it, never the
/// worker, and the callback only `try_lock`s, so it never waits.
pub(super) struct Writer {
    pub producer: HeapProd<f32>,
    pub output_hz: u32,
    /// Utterance the resampler state belongs to.
    utterance: u64,
    resampler: Resampler,
}

pub(super) struct Shared {
    pub writer: Mutex<Writer>,
    /// Bumped by every cancel while it holds the writer lock, so a callback
    /// that checks it under the lock never writes for a stale reply.
    pub generation: AtomicU64,
    /// Utterances of the current generation whose last buffer arrived. Stored
    /// after their samples, so a reader that sees it also sees them.
    pub ended: AtomicU64,
    /// Per-channel samples lost because the ring was full.
    pub dropped: AtomicU64,
    /// Any callback ever ran, which proves the main run loop is serviced.
    pub heard: AtomicBool,
}

impl Writer {
    /// Pushes mono samples through the resampler into the ring and returns
    /// how many did not fit.
    pub fn fill(&mut self, samples: impl Iterator<Item = f32>) -> u64 {
        let mut lost = 0;
        for sample in samples {
            self.resampler.push(sample, |s| {
                lost += u64::from(self.producer.try_push(s).is_err());
            });
        }
        lost
    }
}

impl Shared {
    pub fn new(producer: HeapProd<f32>, output_hz: u32) -> Self {
        let writer = Writer {
            producer,
            output_hz,
            utterance: 0,
            resampler: Resampler::new(f64::from(output_hz), output_hz),
        };
        Self {
            writer: Mutex::new(writer),
            generation: AtomicU64::new(0),
            ended: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            heard: AtomicBool::new(false),
        }
    }

    /// For the engine and the worker, which may wait the few microseconds a
    /// callback holds the lock. Nothing panics under the lock, and a poisoned
    /// one would still hold valid ring indices, so it is used anyway.
    pub fn lock(&self) -> MutexGuard<'_, Writer> {
        self.writer.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub(super) struct Utterance {
    pub generation: u64,
    pub id: u64,
    pub text: String,
    pub voice: String,
    pub rate: SpeechRate,
}

pub(super) enum Command {
    Speak(Utterance),
    /// Drop queued sentences now instead of on the next generation check.
    Stop,
}

/// Runs until the engine drops its sender. `ring` observes the ring's fill
/// level without the writer lock, which the callback must find free.
pub(super) fn run(
    shared: Arc<Shared>,
    ring: Obs<Arc<HeapRb<f32>>>,
    commands: Receiver<Command>,
    min_vacant: usize,
) {
    // SAFETY: plain allocation; the object never leaves this thread.
    let synthesizer = unsafe { AVSpeechSynthesizer::new() };
    let mut pending: VecDeque<Utterance> = VecDeque::new();
    let (mut generation, mut dispatched) = (0, 0);
    loop {
        let wait = if pending.is_empty() {
            IDLE_WAIT
        } else {
            DISPATCH_TICK
        };
        match commands.recv_timeout(wait) {
            Ok(Command::Speak(utterance)) => pending.push_back(utterance),
            Ok(Command::Stop) => {
                pending.clear();
                // SAFETY: a plain message to the synthesizer this thread owns.
                unsafe { synthesizer.stopSpeakingAtBoundary(AVSpeechBoundary::Immediate) };
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        let current = shared.generation.load(Ordering::Acquire);
        pending.retain(|u| u.generation == current);
        if generation != current {
            (generation, dispatched) = (current, 0);
        }
        // One sentence in flight at a time: synthesis outruns playback, and
        // the ring must not fill with a whole reply.
        if shared.ended.load(Ordering::Acquire) >= dispatched
            && ring.vacant_len() >= min_vacant
            && let Some(utterance) = pending.pop_front()
        {
            speak(&synthesizer, &shared, utterance);
            dispatched += 1;
        }
    }
}

fn speak(synthesizer: &AVSpeechSynthesizer, shared: &Arc<Shared>, utterance: Utterance) {
    let text = NSString::from_str(&utterance.text);
    let voice_id = NSString::from_str(&utterance.voice);
    // SAFETY: class constructors with valid strings, setters on the new
    // utterance, and reads of immutable framework constants. A voice removed
    // since loading falls back to the system default.
    let request = unsafe {
        let request = AVSpeechUtterance::speechUtteranceWithString(&text);
        let voice = AVSpeechSynthesisVoice::voiceWithIdentifier(&voice_id);
        request.setVoice(voice.as_deref());
        request.setRate(av_rate(utterance.rate, DEFAULT_RATE, MIN_RATE, MAX_RATE));
        request
    };
    let shared = Arc::clone(shared);
    let (generation, id) = (utterance.generation, utterance.id);
    let block = RcBlock::new(move |buffer: NonNull<AVAudioBuffer>| {
        // SAFETY: AVFoundation passes a live buffer for the duration of the call.
        on_buffer(&shared, generation, id, unsafe { buffer.as_ref() });
    });
    // SAFETY: the block is a valid heap block; AVFoundation copies (retains)
    // it, so dropping ours after the call is fine.
    unsafe { synthesizer.writeUtterance_toBufferCallback(&request, RcBlock::as_ptr(&block)) };
}

/// The main-queue callback. No allocation, no waiting: it `try_lock`s the
/// writer and pushes into the fixed ring, counting what does not fit.
fn on_buffer(shared: &Shared, generation: u64, utterance: u64, buffer: &AVAudioBuffer) {
    shared.heard.store(true, Ordering::Relaxed);
    let Some(pcm) = buffer.downcast_ref::<AVAudioPCMBuffer>() else {
        return;
    };
    // SAFETY: property getters on a live buffer.
    let (frames, stride, format) =
        unsafe { (pcm.frameLength() as usize, pcm.stride(), pcm.format()) };
    // SAFETY: property getters on the buffer's live format.
    let (common, input_hz) = unsafe { (format.commonFormat(), format.sampleRate()) };
    // Only a cancel holds the lock for longer than a callback; audio that
    // meets it belongs to the cancelled reply anyway.
    let Ok(mut writer) = shared.writer.try_lock() else {
        shared.dropped.fetch_add(frames as u64, Ordering::Relaxed);
        return;
    };
    if shared.generation.load(Ordering::Acquire) != generation {
        return;
    }
    if frames == 0 {
        // A zero-length buffer is AVFoundation's end-of-utterance marker.
        shared.ended.fetch_add(1, Ordering::Release);
        return;
    }
    if writer.utterance != utterance {
        writer.utterance = utterance;
        writer.resampler = Resampler::new(input_hz, writer.output_hz);
    }
    // SAFETY (both arms): the channel-data pointer matches the buffer's common
    // format, its first plane holds `frameLength` frames at `stride`, and the
    // buffer outlives this call.
    let lost = unsafe {
        match common {
            AVAudioCommonFormat::PCMFormatFloat32 => NonNull::new(pcm.floatChannelData())
                .map(|p| writer.fill(first_channel(p.read(), stride, frames, |s| s))),
            AVAudioCommonFormat::PCMFormatInt16 => NonNull::new(pcm.int16ChannelData())
                .map(|p| writer.fill(first_channel(p.read(), stride, frames, i16_to_f32))),
            _ => None,
        }
    };
    shared
        .dropped
        .fetch_add(lost.unwrap_or(frames as u64), Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use ringbuf::HeapRb;
    use ringbuf::traits::{Consumer, Split};

    use super::*;

    #[test]
    fn a_full_ring_counts_the_overflow_instead_of_growing() {
        let (producer, mut consumer) = HeapRb::<f32>::new(4).split();
        let shared = Shared::new(producer, 16_000);
        assert_eq!(shared.lock().fill([0.1; 6].into_iter()), 2);
        assert_eq!(consumer.occupied_len(), 4);
        assert_eq!(consumer.try_pop(), Some(0.1));
    }
}
