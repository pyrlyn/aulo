//! The thread that owns a loaded model and decodes utterances one at a time.
//!
//! Decoding a segment takes about 7 % of its length in CPU time (spike T0.5),
//! far too long for `finish` or `poll`, which must return at once. The model
//! also needs about 1.4 GB, so one worker per factory serves every engine the
//! registry builds instead of each loading its own copy.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::thread;

use aulo_speech::SpeechError;

/// Utterances waiting behind the one being decoded. Bounded so a stalled
/// worker turns into a fallback instead of a growing pile of audio.
const JOB_QUEUE: usize = 4;

/// One finished utterance to decode.
#[derive(Debug)]
pub(crate) struct Job {
    pub(crate) samples: Vec<f32>,
    /// Set by `cancel`, so an utterance nobody waits for any more is skipped
    /// instead of delaying the next one.
    pub(crate) cancelled: Arc<AtomicBool>,
    pub(crate) reply: SyncSender<Reply>,
}

/// The transcript, plus the audio buffer handed back so the engine reuses
/// its allocation for the next utterance.
#[derive(Debug)]
pub(crate) struct Reply {
    pub(crate) text: Option<String>,
    pub(crate) samples: Vec<f32>,
}

/// A one-slot reply channel: the worker sends exactly one reply per job.
pub(crate) fn reply_channel() -> (SyncSender<Reply>, Receiver<Reply>) {
    sync_channel(1)
}

#[derive(Debug, Clone)]
pub(crate) struct Worker {
    jobs: SyncSender<Job>,
}

impl Worker {
    /// Starts the thread and waits for `load` to finish there, so a model that
    /// fails to load is reported to the caller instead of to nobody. The
    /// thread exits when the last `Worker` clone is dropped.
    pub(crate) fn spawn<L, D>(load: L) -> Result<Self, SpeechError>
    where
        L: FnOnce() -> Result<D, SpeechError> + Send + 'static,
        D: FnMut(&[f32]) -> Option<String>,
    {
        let (jobs, queue) = sync_channel::<Job>(JOB_QUEUE);
        let (ready, loaded) = sync_channel(1);
        thread::Builder::new()
            .name("aulo-sherpa-stt".into())
            .spawn(move || {
                let mut decode = match load() {
                    Ok(decode) => {
                        let _ = ready.send(Ok(()));
                        decode
                    }
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                for job in queue {
                    if job.cancelled.load(Ordering::Acquire) {
                        continue;
                    }
                    let text = decode(&job.samples);
                    // The engine may have moved on; its receiver is gone then.
                    let _ = job.reply.try_send(Reply {
                        text,
                        samples: job.samples,
                    });
                }
            })
            .map_err(|_| SpeechError::unavailable("cannot start the speech worker thread"))?;
        loaded
            .recv()
            .map_err(|_| SpeechError::failed("speech worker stopped while loading"))??;
        Ok(Self { jobs })
    }

    pub(crate) fn submit(&self, job: Job) -> Result<(), SpeechError> {
        self.jobs.try_send(job).map_err(|error| match error {
            TrySendError::Full(_) => SpeechError::failed("speech worker is busy"),
            TrySendError::Disconnected(_) => SpeechError::unavailable("speech worker stopped"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Never = Result<fn(&[f32]) -> Option<String>, SpeechError>;

    #[test]
    fn a_failed_load_reaches_the_caller() {
        let error =
            Worker::spawn(|| -> Never { Err(SpeechError::unavailable("bad model")) }).unwrap_err();
        assert_eq!(error, SpeechError::unavailable("bad model"));
    }

    #[test]
    fn a_panicking_load_is_an_error_not_a_hang() {
        let error = Worker::spawn(|| -> Never { panic!("loader bug") }).unwrap_err();
        assert!(matches!(error, SpeechError::Failed(_)));
    }
}
