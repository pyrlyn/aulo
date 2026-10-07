//! Realtime speech-to-speech fronts (spec §4.4): a WebSocket client for
//! OpenAI's voice models that maps their events to `aulo-types`, and ephemeral
//! client secrets so a remote client never sees the API key.
//!
//! Two protocols, both checked 2026-10-07 against OpenAI's documentation
//! (`platform.openai.com/docs` redirects to <https://developers.openai.com/api/docs>):
//! - **Realtime** (`gpt-realtime-2.1`): the WebSocket guide
//!   <https://developers.openai.com/api/docs/guides/voice-websockets?api=realtime>
//!   (URL `wss://api.openai.com/v1/realtime?model=...`, `Authorization: Bearer`),
//!   the client and server event references
//!   <https://developers.openai.com/api/reference/resources/realtime/client-events.md>
//!   and `.../server-events.md`, and the conversations guide
//!   <https://developers.openai.com/api/docs/guides/realtime-conversations>.
//!   Audio is base64 in `input_audio_buffer.append` and
//!   `response.output_audio.delta`; the PCM format is 24 kHz mono 16-bit.
//! - **GPT-Live** (`gpt-live-1`): <https://developers.openai.com/api/docs/guides/voice-websockets?api=live>
//!   and the session guide <https://developers.openai.com/api/docs/guides/live-conversations>
//!   (URL `wss://api.openai.com/v1/live/sessions` without a query, first message
//!   `session.start`, audio in `session.input_audio.append` and
//!   `session.output_audio.delta`, transcript fragments, `session.close` answered
//!   by `session.closed` with the usage). The openai-python SDK
//!   (<https://github.com/openai/openai-python>, v3.26.0, `resources/live/live.py`)
//!   uses the same path.
//!
//! Client secrets: `POST /v1/realtime/client_secrets` with `expires_after` and
//! `session`, answered with `value` and `expires_at`
//! (<https://developers.openai.com/api/reference/resources/realtime/subresources/client_secrets/methods/create.md>,
//! and `resources/realtime/client_secrets.py` in openai-python). OpenAI's
//! OpenAPI document (`manual_spec`) still lists only the older
//! `/realtime/sessions`, so it is not used here.
//!
//! Unverified, because the documentation does not say: close codes, the
//! rejection of a bad key at the handshake (taken to be HTTP 401), and how GPT-Live
//! reports audio cut-off by barge-in (it sends no speech boundaries or
//! output-done event, so no voice state or speech events come from it).
//!
//! Nothing is retried, and no server text is echoed except transcripts and tool
//! calls: errors quote keys and account details. All sizes are capped.
//! The remote-client direction is only the minting; the daemon keeps the
//! socket to OpenAI and relays, so the front's tool calls can pass policy.

mod config;
mod decode;
mod error;
mod secret;
mod session;
mod wire;

pub use config::{
    ApiKey, DEFAULT_BASE_URL, DEFAULT_REST_URL, Dialect, RealtimeConfig, SAMPLE_RATE_HZ, Tool,
};
pub use decode::{ServerEvent, ToolCall};
pub use error::RealtimeError;
pub use secret::{ClientSecret, ClientSecretMinter, ClientSecretRequest};
pub use session::{Item, Session};
