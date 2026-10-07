//! What the speech engines share between the caller's thread and the network
//! task: the bounded text and audio queues, the 100 ms chunking and the
//! conversion of 16-bit PCM to samples.

use std::mem;

use aulo_speech::{SpeechError, TtsPoll};
use tokio::runtime::Handle;
use tokio::sync::mpsc::{self, error::TryRecvError};
use tokio_util::task::AbortOnDropHandle;

/// Sentences waiting for the network side. A full queue is `Overflow` rather
/// than a block, because `push_text` must not wait.
const TEXT_QUEUE: usize = 64;
/// Chunks the network side may run ahead of playback: with the chunk size
/// this bounds audio memory to under a second, and stops reading the reply
/// when `poll` falls behind.
const AUDIO_QUEUE: usize = 8;
const PCM_SCALE: f32 = 32768.0;

/// What the network task reports back to `poll`.
#[derive(Debug)]
pub(crate) enum Msg {
    /// Whole 16-bit samples, little endian.
    Audio(Vec<u8>),
    End,
    Failed(SpeechError),
}

/// Why a network task stopped early.
pub(crate) enum Stop {
    /// The reply was cancelled; nobody wants the rest.
    Closed,
    Failed(SpeechError),
}

impl From<SpeechError> for Stop {
    fn from(error: SpeechError) -> Self {
        Self::Failed(error)
    }
}

/// Tells `poll` how the task ended, unless the reply is gone.
pub(crate) async fn conclude(audio: &mpsc::Sender<Msg>, outcome: Result<(), Stop>) {
    let verdict = match outcome {
        Ok(()) => Msg::End,
        Err(Stop::Closed) => return,
        Err(Stop::Failed(error)) => Msg::Failed(error),
    };
    let _ = audio.send(verdict).await;
}

/// Bytes of 16-bit mono PCM in 100 ms. Small chunks keep `cancel` and first audio quick.
pub(crate) fn chunk_bytes(sample_rate_hz: u32) -> usize {
    (sample_rate_hz / 10) as usize * 2
}

/// Cuts a PCM byte stream into queue messages of at most one chunk.
pub(crate) struct Chunker {
    size: usize,
    pending: Vec<u8>,
}

impl Chunker {
    pub(crate) fn new(size: usize) -> Self {
        Self {
            size,
            pending: Vec::with_capacity(size),
        }
    }

    /// Queues `bytes`, waiting while the queue is full: that is what slows the server.
    pub(crate) async fn feed(
        &mut self,
        mut bytes: &[u8],
        audio: &mpsc::Sender<Msg>,
    ) -> Result<(), Stop> {
        while !bytes.is_empty() {
            let (take, tail) = bytes.split_at(bytes.len().min(self.size - self.pending.len()));
            self.pending.extend_from_slice(take);
            bytes = tail;
            if self.pending.len() == self.size {
                self.send(audio).await?;
            }
        }
        // Waiting for a full chunk would delay audio the server already sent.
        self.send(audio).await
    }

    /// A half sample at the end of a reply is noise, and would shift the next reply.
    pub(crate) fn clear(&mut self) {
        self.pending.clear();
    }

    /// Sends the whole samples and keeps a trailing odd byte, which is the
    /// first half of a sample that straddles two network chunks.
    async fn send(&mut self, audio: &mpsc::Sender<Msg>) -> Result<(), Stop> {
        let even = self.pending.len() & !1;
        if even == 0 {
            return Ok(());
        }
        let odd = self.pending.get(even).copied();
        self.pending.truncate(even);
        let chunk = mem::replace(&mut self.pending, Vec::with_capacity(self.size));
        audio
            .send(Msg::Audio(chunk))
            .await
            .map_err(|_| Stop::Closed)?;
        self.pending.extend(odd);
        Ok(())
    }
}

/// Longest text one reply accepts.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TextLimits {
    /// For one pushed text.
    pub(crate) each: usize,
    /// For all of them together.
    pub(crate) total: usize,
}

/// How one `poll` ended.
pub(crate) enum Polled {
    Audio(usize),
    Pending,
    /// The reply is over, cleanly or not; the engine goes idle.
    Ended(Result<(), SpeechError>),
}

/// One reply. Dropping it aborts the task, which is how `cancel` and `begin`
/// stay instant.
#[derive(Debug)]
pub(crate) struct Reply {
    /// `None` once `finish` closed the text side.
    texts: Option<mpsc::Sender<String>>,
    audio: mpsc::Receiver<Msg>,
    _task: AbortOnDropHandle<()>,
    limits: TextLimits,
    sent_chars: usize,
    chunk: Vec<u8>,
    offset: usize,
    /// How the reply ended, held back until the audio before it is delivered.
    verdict: Option<Result<(), SpeechError>>,
}

impl Reply {
    /// Runs `task` on `runtime` with the two queues' far ends.
    pub(crate) fn start<F>(
        runtime: &Handle,
        limits: TextLimits,
        task: impl FnOnce(mpsc::Receiver<String>, mpsc::Sender<Msg>) -> F,
    ) -> Self
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let (texts, text_queue) = mpsc::channel(TEXT_QUEUE);
        let (audio_sender, audio) = mpsc::channel(AUDIO_QUEUE);
        Self {
            texts: Some(texts),
            audio,
            _task: AbortOnDropHandle::new(runtime.spawn(task(text_queue, audio_sender))),
            limits,
            sent_chars: 0,
            chunk: Vec::new(),
            offset: 0,
            verdict: None,
        }
    }

    pub(crate) fn push_text(&mut self, text: &str) -> Result<(), SpeechError> {
        let Some(texts) = &self.texts else {
            return Err(SpeechError::OutOfOrder("text outside a reply"));
        };
        let text = text.trim();
        if text.is_empty() {
            // Nothing to say; a request for it would only bill and fail.
            return Ok(());
        }
        // `nth` stops at the limit, so a huge string is not walked to its end.
        if text.chars().nth(self.limits.each).is_some() {
            return Err(SpeechError::invalid(
                "text",
                "longer than the engine allows",
            ));
        }
        let chars = text.chars().count();
        if self.sent_chars.saturating_add(chars) > self.limits.total {
            return Err(SpeechError::invalid("text", "reply is too long"));
        }
        match texts.try_send(text.to_owned()) {
            Ok(()) => {
                self.sent_chars += chars;
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(text)) => Err(SpeechError::Overflow {
                dropped: text.chars().count(),
            }),
            // The task already failed; `poll` reports why.
            Err(mpsc::error::TrySendError::Closed(_)) => Ok(()),
        }
    }

    pub(crate) fn finish(&mut self) {
        self.texts = None;
    }

    fn poll(&mut self, out: &mut [f32]) -> Polled {
        let mut written = 0;
        while written < out.len() {
            if self.offset < self.chunk.len() {
                written += self.drain_into(&mut out[written..]);
                continue;
            }
            if self.verdict.is_some() {
                break;
            }
            match self.audio.try_recv() {
                Ok(Msg::Audio(bytes)) => {
                    self.chunk = bytes;
                    self.offset = 0;
                }
                Ok(Msg::End) => self.verdict = Some(Ok(())),
                Ok(Msg::Failed(error)) => self.verdict = Some(Err(error)),
                Err(TryRecvError::Empty) => break,
                // The task ended without a verdict: the runtime shut down under it.
                Err(TryRecvError::Disconnected) => {
                    self.verdict = Some(Err(SpeechError::failed("speech task ended early")));
                }
            }
        }
        if written > 0 {
            return Polled::Audio(written);
        }
        match self.verdict.take() {
            Some(verdict) => Polled::Ended(verdict),
            None => Polled::Pending,
        }
    }

    /// Converts as many whole samples as fit; the chunk always holds whole ones.
    fn drain_into(&mut self, out: &mut [f32]) -> usize {
        let pending = self.chunk.get(self.offset..).unwrap_or_default();
        let mut written = 0;
        let (pairs, _) = pending.as_chunks::<2>();
        for (slot, pair) in out.iter_mut().zip(pairs) {
            let sample = i16::from_le_bytes(*pair);
            *slot = f32::from(sample) / PCM_SCALE;
            written += 1;
        }
        self.offset += written * 2;
        written
    }
}

/// The reply an engine is speaking, if any: the `TtsEngine` methods that only
/// move text and audio between the caller and the network task.
#[derive(Debug, Default)]
pub(crate) struct Slot(Option<Reply>);

impl Slot {
    /// Replaces the reply in flight, which aborts it.
    pub(crate) fn set(&mut self, reply: Reply) {
        self.0 = Some(reply);
    }

    pub(crate) fn cancel(&mut self) {
        self.0 = None;
    }

    pub(crate) fn push_text(&mut self, text: &str) -> Result<(), SpeechError> {
        match &mut self.0 {
            Some(reply) => reply.push_text(text),
            None => Err(SpeechError::OutOfOrder("text outside a reply")),
        }
    }

    pub(crate) fn finish(&mut self) -> Result<(), SpeechError> {
        let Some(reply) = &mut self.0 else {
            return Err(SpeechError::OutOfOrder("finish before begin"));
        };
        reply.finish();
        Ok(())
    }

    pub(crate) fn poll(&mut self, out: &mut [f32]) -> Result<TtsPoll, SpeechError> {
        let Some(reply) = &mut self.0 else {
            return Ok(TtsPoll::Done);
        };
        match reply.poll(out) {
            Polled::Audio(samples) => Ok(TtsPoll::Audio { samples }),
            Polled::Pending => Ok(TtsPoll::Pending),
            Polled::Ended(verdict) => {
                self.0 = None;
                verdict.map(|()| TtsPoll::Done)
            }
        }
    }
}
