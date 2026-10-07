//! Cloud speech engines. Today: speech-to-text over the OpenAI-compatible
//! `POST /v1/audio/transcriptions` endpoint, which OpenAI and local servers
//! such as runa both serve.
//!
//! Reference, checked 2026-10-07: the `createTranscription` operation and the
//! `TranscriptTextDeltaEvent` / `TranscriptTextDoneEvent` schemas in OpenAI's
//! published OpenAPI document,
//! <https://github.com/openai/openai-openapi/blob/manual_spec/openapi.yaml>
//! (rendered at <https://platform.openai.com/docs/api-reference/audio>, which
//! refuses automated fetches). The request is multipart with `file`, `model`,
//! `response_format=json`, an optional ISO-639-1 `language` and, for the
//! streaming shape, `stream=true`. A plain reply is `{"text": ...}`; a streamed
//! reply is server-sent events whose JSON `type` is `transcript.text.delta`
//! (`delta`) or `transcript.text.done` (`text`). OpenAI ignores `stream` for
//! `whisper-1` and answers with plain JSON, so the reply shape is taken from
//! the response `Content-Type`, not from the request.
//!
//! The engine is non-streaming on the audio side: frames are buffered and the
//! whole utterance is uploaded at `finish`. The streaming shape only makes
//! partial transcripts arrive while the server is still decoding.
//!
//! Nothing is retried. A rejected key is reported as
//! [`aulo_speech::SpeechError::Unavailable`] so the registry warns and falls
//! back at once instead of sending every utterance to a server that refuses it.

mod config;
mod engine;
mod http;

pub use config::{ApiKey, CloudSttConfig, MAX_UTTERANCE_LIMIT, ResponseShape};
pub use engine::{CloudStt, CloudSttFactory, MAX_TRANSCRIPT_BYTES};
