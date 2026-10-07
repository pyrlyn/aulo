//! The thread that owns the synthesis backend, and the ring it fills.
//!
//! WinRT synthesizes a whole sentence before returning its audio, so the
//! worker runs one sentence at a time and pushes the audio into the ring as
//! fast as the pipeline drains it: a full ring makes the worker wait, and no
//! sample is dropped. Cancel works by generation: the engine bumps it, and the
//! worker abandons whatever it holds the next time it looks.

use std::collections::VecDeque;
use std::io::BufReader;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use aulo_speech::{SpeechError, SpeechRate, Voice};
use ringbuf::HeapProd;
use ringbuf::traits::Producer;

use crate::wav;

/// The rate every reply is declared at. WinRT voices differ and report theirs
/// only in the WAV header, after the engine has had to name a format, so
/// `wav` converts whatever arrives to this.
pub(super) const OUTPUT_HZ: u32 = 22_050;
/// Bytes pulled from a WinRT stream per call: each is a call into the system,
/// and hound reads a sample at a time.
const READ_BYTES: usize = 8 * 1024;
/// Input samples per ring write, so a cancel is noticed within a few ms.
const CHUNK_SAMPLES: usize = 1024;
const SOURCE: &str = "Windows speech";
/// How long a worker facing a full ring waits before looking again.
const FULL_RING_WAIT: Duration = Duration::from_millis(5);

/// What the worker needs from the system: its voices, and a sentence's audio
/// as a WAV stream. It is created on the worker thread and never leaves it.
pub(super) trait Backend {
    fn catalog(&self) -> Result<Catalog, SpeechError>;

    /// Synthesizes `text` and returns its WAV stream, or `None` once
    /// `cancelled` reports true. It must poll `cancelled` while it waits.
    fn synthesize(
        &mut self,
        text: &str,
        voice: &str,
        rate: SpeechRate,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<Box<dyn std::io::Read>>, SpeechError>;
}

#[derive(Debug, Clone)]
pub(super) struct Catalog {
    pub voices: Vec<Voice>,
    /// Index of the voice the system speaks with when none is asked for.
    pub default: Option<usize>,
}

pub(super) struct Utterance {
    pub generation: u64,
    pub text: String,
    pub voice: String,
    pub rate: SpeechRate,
}

struct State {
    producer: HeapProd<f32>,
    failure: Option<SpeechError>,
}

/// Shared by the engine and the worker. The mutex is held only for a ring
/// write, a failure note, or a cancel, so neither side waits long.
pub(super) struct Shared {
    state: Mutex<State>,
    /// Bumped by every cancel while it holds the lock, so a worker that checks
    /// it under the lock never writes for a stale reply.
    pub generation: AtomicU64,
    /// Sentences of the current generation whose audio is all in the ring.
    /// Stored after their samples, so a reader that sees it also sees them.
    pub ended: AtomicU64,
}

impl Shared {
    pub fn new(producer: HeapProd<f32>) -> Self {
        let state = State {
            producer,
            failure: None,
        };
        Self {
            state: Mutex::new(state),
            generation: AtomicU64::new(0),
            ended: AtomicU64::new(0),
        }
    }

    /// Nothing panics under the lock, and a poisoned one would still hold
    /// valid ring indices, so it is used anyway.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Starts a new generation. Every worker write for an older one fails
    /// from here on, so the engine can clear the ring right after.
    pub fn reset(&self) {
        let mut state = self.lock();
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.ended.store(0, Ordering::Release);
        state.failure = None;
    }

    pub fn take_failure(&self) -> Option<SpeechError> {
        self.lock().failure.take()
    }

    fn is_stale(&self, generation: u64) -> bool {
        self.generation.load(Ordering::Acquire) != generation
    }

    /// Pushes `samples`, waiting while the ring is full. False once the
    /// reply was cancelled.
    fn push(&self, generation: u64, mut samples: &[f32]) -> bool {
        while !samples.is_empty() {
            {
                let mut state = self.lock();
                if self.is_stale(generation) {
                    return false;
                }
                samples = &samples[state.producer.push_slice(samples)..];
            }
            if !samples.is_empty() {
                thread::sleep(FULL_RING_WAIT);
            }
        }
        true
    }

    fn settle(&self, generation: u64, outcome: Result<(), SpeechError>) {
        let mut state = self.lock();
        if self.is_stale(generation) {
            return;
        }
        match outcome {
            Ok(()) => {
                self.ended.fetch_add(1, Ordering::Release);
            }
            Err(error) => state.failure = Some(error),
        }
    }
}

/// Runs until the engine drops its sender.
pub(super) fn run<B: Backend>(mut backend: B, shared: &Shared, utterances: &Receiver<Utterance>) {
    let mut pending = VecDeque::new();
    while let Ok(first) = utterances.recv() {
        pending.push_back(first);
        loop {
            pending.extend(utterances.try_iter());
            let current = shared.generation.load(Ordering::Acquire);
            pending.retain(|u: &Utterance| u.generation == current);
            let Some(utterance) = pending.pop_front() else {
                break;
            };
            speak(&mut backend, shared, &utterance);
        }
    }
}

fn speak<B: Backend>(backend: &mut B, shared: &Shared, utterance: &Utterance) {
    let generation = utterance.generation;
    let cancelled = || shared.is_stale(generation);
    let outcome = backend
        .synthesize(
            &utterance.text,
            &utterance.voice,
            utterance.rate,
            &cancelled,
        )
        .and_then(|stream| match stream {
            None => Ok(()),
            Some(stream) => {
                let wav = wav::open(BufReader::with_capacity(READ_BYTES, stream), SOURCE)?;
                let push = |chunk: &[f32]| shared.push(generation, chunk);
                wav::stream(wav, SOURCE, OUTPUT_HZ, CHUNK_SAMPLES, false, push).map(drop)
            }
        });
    shared.settle(generation, outcome);
}
