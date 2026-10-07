//! Cloud speech engines: speech-to-text over the OpenAI-compatible
//! `POST /v1/audio/transcriptions` endpoint and text-to-speech over
//! `POST /v1/audio/speech`, which OpenAI and local servers such as runa both
//! serve.
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
//! Speech, checked 2026-10-07 against the same document: the `createSpeech`
//! operation and the `CreateSpeechRequest` schema. The request is JSON with
//! `model`, `input` (at most 4096 characters), `voice`, `response_format` and
//! `speed` (0.25 to 4.0); the 200 reply is a chunked `application/octet-stream`.
//! The spec lists the voices `alloy` to `verse` ([`openai_voices`]) and the
//! `pcm` format, but not its layout: 24 kHz, 16-bit signed little-endian mono
//! comes from the text-to-speech guide on platform.openai.com (unverified
//! here, since it refuses automated fetches). Each pushed text is its own
//! request, run in order; the body is cut into 100 ms chunks and read no
//! faster than playback drains them.
//!
//! Speech can also come from ElevenLabs over a WebSocket ([`ElevenLabsTts`]); its
//! protocol and sources are documented in that module.
//!
//! Nothing is retried. A rejected key is reported as
//! [`aulo_speech::SpeechError::Unavailable`] so the registry warns and falls
//! back at once instead of sending every utterance to a server that refuses it.

mod config;
mod elevenlabs;
mod engine;
mod http;
mod net;
mod reply;
mod speech;
mod tts;

pub use config::{
    ApiKey, CloudSttConfig, CloudTtsConfig, DEFAULT_MAX_TEXT_CHARS, MAX_AUDIO_BYTES_LIMIT,
    MAX_TEXT_CHARS_LIMIT, MAX_UTTERANCE_LIMIT, ResponseShape, openai_voices,
};
pub use elevenlabs::{
    DEFAULT_BASE_URL, ElevenLabsTts, ElevenLabsTtsConfig, ElevenLabsTtsFactory,
    MAX_REPLY_CHARS_LIMIT, PCM_SAMPLE_RATES_HZ,
};
pub use engine::{CloudStt, CloudSttFactory, MAX_TRANSCRIPT_BYTES};
pub use tts::{CloudTts, CloudTtsFactory, SAMPLE_RATE_HZ};
