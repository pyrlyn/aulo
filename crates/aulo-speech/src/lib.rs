//! The contract every speech engine implements: speech-to-text, text-to-speech,
//! voice activity, keyword spotting and end of turn. It holds traits and the
//! value types they exchange, and nothing that does I/O, so built-in engines,
//! cloud clients and gRPC plugin proxies all implement the same surface and the
//! registry can hold any of them as `Box<dyn …>`.
//!
//! # Why the traits are synchronous: push, then poll
//!
//! Every engine is driven the same way: the pipeline pushes input (audio
//! frames or text), then polls for output into a buffer it owns. No method is
//! `async`, and no method may block on I/O.
//!
//! - **No allocation per frame.** Object-safe async methods need
//!   `async_trait`, which boxes a new future on every call. At 50 frames a
//!   second per engine that is a steady stream of allocations on the audio
//!   path, which the project rules forbid. Plain `&mut self` methods on a
//!   `dyn` engine cost one indirect call.
//! - **It matches the engines.** sherpa-onnx streams are push-then-decode,
//!   VADs and keyword spotters answer per frame, and TTS engines produce
//!   audio in chunks. A poll API maps onto them one to one.
//! - **Network engines still fit.** A cloud or plugin engine owns a background
//!   task and two bounded queues. `push` is a non-blocking `try_send` that
//!   reports [`SpeechError::Overflow`] when the queue is full instead of
//!   waiting; `poll` is a non-blocking `try_recv`. The bounded queues are the
//!   backpressure, so nothing on the audio path can grow without limit.
//! - **Output goes into caller buffers.** STT writes into a reused
//!   [`Transcript`]; TTS writes samples into a caller slice. Steady-state
//!   polling therefore allocates nothing on the caller side.
//!
//! Local inference can still take tens of milliseconds of CPU in `poll` or
//! `finish`. That is why the pipeline drives engines from its own speech
//! worker, never from the audio device callback, which only fills a ring
//! buffer.
//!
//! # Failing open
//!
//! Errors are one shared [`SpeechError`] rather than an associated type per
//! engine: the registry holds engines of different origins side by side, and
//! a gRPC plugin can only report a category and a message anyway.
//! [`SpeechError::should_fall_back`] tells the registry when to skip to the
//! next engine in the fallback list.

mod audio;
mod detect;
mod engine;
mod error;
mod stt;
mod tts;

pub use audio::{AudioFormat, AudioFrame, PIPELINE_SAMPLE_RATE_HZ};
pub use aulo_types::{TranscriptKind, TurnId};
pub use detect::{KeywordHit, KeywordSpotter, TurnDecision, TurnDetector, Vad, VadEvent};
pub use engine::{Capabilities, EngineId, EngineInfo, EngineKind, LanguageSupport};
pub use error::{ErrorDetail, MAX_ERROR_DETAIL_BYTES, SpeechError};
pub use stt::{SttEngine, SttPoll, Transcript};
pub use tts::{SpeechRate, TtsEngine, TtsPoll, TtsRequest, Voice};
