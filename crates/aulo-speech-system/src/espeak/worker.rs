//! The worker thread: speaks queued utterances one after another, each in its
//! own espeak-ng process, and feeds the samples into a fixed-size ring.

use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use aulo_speech::SpeechError;
use ringbuf::HeapProd;
use ringbuf::traits::Producer;

use super::process;

/// How long the ring may stay full before the pipeline is taken to have
/// stopped polling, and the rest of the utterance is dropped.
pub(super) const STALL_LIMIT: Duration = Duration::from_secs(10);
const RETRY_PAUSE: Duration = Duration::from_millis(2);

pub(super) struct Utterance {
    pub generation: u64,
    pub text: String,
    pub voice: Option<String>,
    pub words_per_minute: u32,
}

pub(super) struct Config {
    pub binary: PathBuf,
    pub sample_rate_hz: u32,
    pub stall_limit: Duration,
}

pub(super) struct Shared {
    /// Held by every writer of the ring and by cancel, which also holds the
    /// first failure of the reply, so a stale utterance can never put samples
    /// or a failure into the reply that replaced it.
    state: Mutex<Option<SpeechError>>,
    pub generation: AtomicU64,
    /// Utterances of the current generation that are over, failed or not.
    /// Stored after their samples, so a reader that sees it also sees them.
    pub ended: AtomicU64,
    /// Samples lost to a full ring since the engine loaded.
    pub dropped: AtomicU64,
    /// The running process, so a cancel can kill it from another thread.
    child: Mutex<Option<Child>>,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(None),
            generation: AtomicU64::new(0),
            ended: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            child: Mutex::new(None),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, Option<SpeechError>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn child(&self) -> MutexGuard<'_, Option<Child>> {
        self.child.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Kills the running process. The worker sees the pipe close and reaps it.
    pub fn kill_child(&self) {
        if let Some(child) = self.child().as_mut() {
            // Fails only when it already exited.
            let _ = child.kill();
        }
    }

    fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::Acquire) == generation
    }

    /// Writes all of `samples`, waiting for the consumer when the ring is
    /// full, which stops the child through its full pipe instead of dropping
    /// audio. False when the reply was cancelled or the consumer stalled.
    fn push(
        &self,
        ring: &mut HeapProd<f32>,
        generation: u64,
        mut samples: &[f32],
        stall_limit: Duration,
    ) -> bool {
        let mut stalled_since = None;
        while !samples.is_empty() {
            {
                let _writer = self.lock();
                if !self.is_current(generation) {
                    return false;
                }
                let written = ring.push_slice(samples);
                samples = &samples[written..];
                if written > 0 {
                    stalled_since = None;
                }
            }
            if samples.is_empty() {
                break;
            }
            let since = *stalled_since.get_or_insert_with(Instant::now);
            if since.elapsed() > stall_limit {
                self.dropped
                    .fetch_add(samples.len() as u64, Ordering::Relaxed);
                return false;
            }
            thread::sleep(RETRY_PAUSE);
        }
        true
    }

    fn end_utterance(&self, generation: u64, outcome: Result<(), SpeechError>) {
        let mut failure = self.lock();
        if !self.is_current(generation) {
            return;
        }
        if let Err(error) = outcome {
            failure.get_or_insert(error);
        }
        self.ended.fetch_add(1, Ordering::Release);
    }
}

pub(super) fn run(
    shared: Arc<Shared>,
    mut ring: HeapProd<f32>,
    utterances: Receiver<Utterance>,
    config: Config,
) {
    // The loop ends when the engine is dropped and the channel closes.
    while let Ok(utterance) = utterances.recv() {
        if !shared.is_current(utterance.generation) {
            continue;
        }
        let outcome = speak(&shared, &mut ring, &config, &utterance);
        shared.end_utterance(utterance.generation, outcome);
    }
}

fn speak(
    shared: &Shared,
    ring: &mut HeapProd<f32>,
    config: &Config,
    utterance: &Utterance,
) -> Result<(), SpeechError> {
    let generation = utterance.generation;
    let (child, stdout) = process::spawn_synth(
        &config.binary,
        utterance.voice.as_deref(),
        utterance.words_per_minute,
        &utterance.text,
    )?;
    {
        // Checked under the same lock cancel kills under, so a cancel is
        // either seen here or kills the child stored here.
        let mut slot = shared.child();
        if !shared.is_current(generation) {
            drop(slot);
            process::reap(child, true);
            return Ok(());
        }
        *slot = Some(child);
    }
    let streamed = process::stream_samples(stdout, config.sample_rate_hz, |samples| {
        shared.push(ring, generation, samples, config.stall_limit)
    });
    let child = shared.child().take();
    let status = child.and_then(|child| process::reap(child, !matches!(streamed, Ok(true))));
    if !shared.is_current(generation) {
        return Ok(());
    }
    // Stopped early with the current reply still wanted: the consumer stalled,
    // and `push` already counted what it dropped.
    if !streamed? {
        return Ok(());
    }
    match status {
        Some(status) if status.success() => Ok(()),
        _ => Err(SpeechError::failed("espeak-ng exited with an error")),
    }
}
