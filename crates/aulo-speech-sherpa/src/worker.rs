//! The thread that owns a loaded model and runs jobs one at a time.
//!
//! Decoding a segment takes about 7 % of its length in CPU time and Kokoro
//! needs about a quarter of the audio's length (spike T0.5), far too long for
//! `finish`, `push_text` or `poll`, which must return at once. Each model also
//! needs about 1 GB, so one worker per factory serves every engine the
//! registry builds instead of each loading its own copy.

use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::thread;

use aulo_speech::SpeechError;

/// Most threads a model may use; more than the cores only adds contention.
pub const MAX_THREADS: u16 = 64;

pub(crate) fn check_threads(threads: u16) -> Result<(), SpeechError> {
    if threads == 0 || threads > MAX_THREADS {
        return Err(SpeechError::invalid("threads", "1 to 64"));
    }
    Ok(())
}

/// One unit of work: an utterance to decode or a sentence to speak.
pub(crate) trait Job: Send + 'static {
    /// Set by `cancel`, so work nobody waits for any more is skipped instead
    /// of delaying the next reply.
    fn cancelled(&self) -> bool;
}

#[derive(Debug)]
pub(crate) struct Worker<J> {
    jobs: SyncSender<J>,
}

// A derive would demand `J: Clone`, but only the sender is cloned.
impl<J> Clone for Worker<J> {
    fn clone(&self) -> Self {
        Self {
            jobs: self.jobs.clone(),
        }
    }
}

impl<J: Job> Worker<J> {
    /// Starts the thread and waits for `load` to finish there, so a model that
    /// fails to load is reported to the caller instead of to nobody. `queue`
    /// jobs may wait behind the one running; bounded so a stalled worker turns
    /// into a fallback instead of a growing pile of work. The thread exits
    /// when the last `Worker` clone is dropped.
    pub(crate) fn spawn<L, R>(name: &str, queue: usize, load: L) -> Result<Self, SpeechError>
    where
        L: FnOnce() -> Result<R, SpeechError> + Send + 'static,
        R: FnMut(J),
    {
        let (jobs, waiting) = sync_channel::<J>(queue);
        let (ready, loaded) = sync_channel(1);
        thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let mut run = match load() {
                    Ok(run) => {
                        let _ = ready.send(Ok(()));
                        run
                    }
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                for job in waiting {
                    if !job.cancelled() {
                        run(job);
                    }
                }
            })
            .map_err(|_| SpeechError::unavailable("cannot start the speech worker thread"))?;
        loaded
            .recv()
            .map_err(|_| SpeechError::failed("speech worker stopped while loading"))??;
        Ok(Self { jobs })
    }

    /// Never blocks; a full queue hands the job back so the engine decides
    /// whether that is a failure or an overflow.
    pub(crate) fn submit(&self, job: J) -> Result<(), TrySendError<J>> {
        self.jobs.try_send(job)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::Sender;

    use super::*;

    #[derive(Debug)]
    struct Echo(u8, bool, Sender<u8>);

    impl Job for Echo {
        fn cancelled(&self) -> bool {
            self.1
        }
    }

    type Never = Result<fn(Echo), SpeechError>;

    #[test]
    fn a_failed_load_reaches_the_caller() {
        let error = Worker::<Echo>::spawn("t", 1, || -> Never {
            Err(SpeechError::unavailable("bad model"))
        })
        .unwrap_err();
        assert_eq!(error, SpeechError::unavailable("bad model"));
    }

    #[test]
    fn a_panicking_load_is_an_error_not_a_hang() {
        let error =
            Worker::<Echo>::spawn("t", 1, || -> Never { panic!("loader bug") }).unwrap_err();
        assert!(matches!(error, SpeechError::Failed(_)));
    }

    #[test]
    fn cancelled_jobs_are_skipped_and_order_is_kept() {
        let worker = Worker::spawn("t", 4, || {
            Ok(|job: Echo| {
                let _ = job.2.send(job.0);
            })
        })
        .unwrap();
        let (done, seen) = std::sync::mpsc::channel();
        for (n, skip) in [(1, false), (2, true), (3, false)] {
            worker.submit(Echo(n, skip, done.clone())).unwrap();
        }
        drop(done);
        assert_eq!(seen.iter().collect::<Vec<_>>(), [1, 3]);
    }

    #[test]
    fn threads_are_bounded() {
        assert!(check_threads(0).is_err());
        assert!(check_threads(MAX_THREADS + 1).is_err());
        assert!(check_threads(4).is_ok());
    }
}
