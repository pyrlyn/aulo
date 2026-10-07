//! The background half of the engine: one WebSocket per utterance.

use std::time::Duration;

use aulo_speech::SpeechError;
use futures_util::{SinkExt, StreamExt};
use reqwest::Url;
use reqwest::header::{AUTHORIZATION, HeaderValue};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::time::{Instant, MissedTickBehavior, interval_at, sleep_until, timeout};
use tokio_tungstenite::connect_async_tls_with_config;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::protocol::frame::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

use crate::engine::{Event, MAX_TRANSCRIPT_BYTES};
use crate::http::{capped, push_capped};
use crate::net;

const NOUN: &str = "transcription";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// A `Results` message carries per-word timings, so it is far larger than its
/// transcript; the cap stops a server that never ends a message.
const MAX_MESSAGE_BYTES: usize = 8 * MAX_TRANSCRIPT_BYTES;
/// Everything the server may send over one utterance. Each message is parsed
/// and dropped, so this bounds time and bandwidth rather than memory.
const MAX_RECEIVED_BYTES: usize = 32 * 1024 * 1024;
const KEEP_ALIVE: &str = r#"{"type":"KeepAlive"}"#;
const CLOSE_STREAM: &str = r#"{"type":"CloseStream"}"#;

pub(super) struct Session {
    /// Carries the query only; the key is in `auth`, never in the URL.
    pub(super) url: Url,
    pub(super) auth: Option<HeaderValue>,
    pub(super) keepalive: Duration,
    pub(super) finish_timeout: Duration,
}

pub(super) async fn run(
    session: Session,
    audio: mpsc::Receiver<Vec<u8>>,
    events: mpsc::Sender<Event>,
) {
    let last = match converse(&session, audio, &events).await {
        Ok(text) => Event::Final(text),
        Err(error) => Event::Failed(error),
    };
    // A closed channel means the utterance was cancelled; nobody wants the result.
    let _ = events.send(last).await;
}

#[derive(Deserialize)]
struct Reply {
    #[serde(rename = "type")]
    kind: Option<String>,
    is_final: Option<bool>,
    channel: Option<Channel>,
}

#[derive(Deserialize)]
struct Channel {
    alternatives: Vec<Alternative>,
}

#[derive(Deserialize)]
struct Alternative {
    transcript: String,
}

/// What the server has said so far: the `is_final` results appended, then the
/// latest preliminary one for the audio after them.
#[derive(Default)]
struct Heard {
    committed: String,
    interim: String,
}

impl Heard {
    /// The cumulative text after `reply`, or `None` when it changes nothing.
    fn absorb(&mut self, reply: Reply) -> Option<String> {
        let transcript = reply.channel?.alternatives.into_iter().next()?.transcript;
        let piece = transcript.trim();
        if reply.is_final == Some(true) {
            self.interim.clear();
            if piece.is_empty() {
                return None;
            }
            append(&mut self.committed, piece);
        } else if piece.is_empty() && self.interim.is_empty() {
            return None;
        } else {
            self.interim.clear();
            push_capped(&mut self.interim, piece);
        }
        Some(self.text())
    }

    fn text(&self) -> String {
        let mut text = self.committed.clone();
        if !self.interim.is_empty() {
            append(&mut text, &self.interim);
        }
        capped(&text).trim().to_owned()
    }
}

fn append(buffer: &mut String, piece: &str) {
    if !buffer.is_empty() {
        push_capped(buffer, " ");
    }
    push_capped(buffer, piece);
}

async fn converse(
    session: &Session,
    mut audio: mpsc::Receiver<Vec<u8>>,
    events: &mpsc::Sender<Event>,
) -> Result<String, SpeechError> {
    let mut request = session
        .url
        .as_str()
        .into_client_request()
        .map_err(|_| SpeechError::failed("cannot build the transcription request"))?;
    if let Some(auth) = &session.auth {
        request.headers_mut().insert(AUTHORIZATION, auth.clone());
    }
    let limits = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_MESSAGE_BYTES));
    let (socket, _) = timeout(
        CONNECT_TIMEOUT,
        connect_async_tls_with_config(request, Some(limits), false, None),
    )
    .await
    .map_err(|_| unreachable_server())?
    .map_err(connect_error)?;
    let (mut sink, mut stream) = socket.split();

    let mut heard = Heard::default();
    let mut received = 0usize;
    let mut closing = false;
    let mut deadline = Instant::now();
    let mut keepalive = interval_at(Instant::now() + session.keepalive, session.keepalive);
    keepalive.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            incoming = stream.next() => {
                let message = match incoming {
                    Some(Ok(message)) => message,
                    Some(Err(error)) => return Err(stream_error(&error)),
                    None => return Err(SpeechError::failed("the transcription stream ended early")),
                };
                if let Message::Text(text) = &message {
                    received = received.saturating_add(text.len());
                    if received > MAX_RECEIVED_BYTES {
                        return Err(SpeechError::failed("stream is too long"));
                    }
                }
                if handle(message, &mut heard, events, closing)? {
                    return Ok(heard.text());
                }
            }
            chunk = audio.recv(), if !closing => match chunk {
                Some(bytes) => {
                    send(&mut sink, Message::binary(bytes)).await?;
                    keepalive.reset();
                }
                None => {
                    // The queue is drained, so everything said is on the wire.
                    send(&mut sink, Message::text(CLOSE_STREAM)).await?;
                    closing = true;
                    deadline = Instant::now() + session.finish_timeout;
                }
            },
            _ = keepalive.tick(), if !closing => send(&mut sink, Message::text(KEEP_ALIVE)).await?,
            () = sleep_until(deadline), if closing => return Err(net::timed_out(NOUN)),
        }
    }
}

async fn send<S>(sink: &mut S, message: Message) -> Result<(), SpeechError>
where
    S: futures_util::Sink<Message, Error = WsError> + Unpin,
{
    timeout(WRITE_TIMEOUT, sink.send(message))
        .await
        .map_err(|_| net::timed_out(NOUN))?
        .map_err(|error| stream_error(&error))
}

/// True once the utterance is complete.
fn handle(
    message: Message,
    heard: &mut Heard,
    events: &mpsc::Sender<Event>,
    closing: bool,
) -> Result<bool, SpeechError> {
    match message {
        Message::Text(text) => {
            let reply: Reply = serde_json::from_str(&text)
                .map_err(|_| SpeechError::failed("the server sent an unreadable message"))?;
            match reply.kind.as_deref() {
                Some("Results") => {
                    if let Some(text) = heard.absorb(reply) {
                        // A newer cumulative partial supersedes a dropped one.
                        let _ = events.try_send(Event::Partial(text));
                    }
                    Ok(false)
                }
                // The server sends it last, after the results `CloseStream` flushed.
                Some("Metadata") => Ok(closing),
                // The server text is untrusted and may quote the request.
                Some("Error") => Err(SpeechError::failed("the server reported an error")),
                _ => Ok(false),
            }
        }
        Message::Close(frame) => closed(frame, closing).map(|()| true),
        Message::Binary(_) => Err(SpeechError::failed("the server sent an unexpected message")),
        _ => Ok(false),
    }
}

/// A close is fine once `CloseStream` was sent and the server hung up normally.
fn closed(frame: Option<CloseFrame>, closing: bool) -> Result<(), SpeechError> {
    match frame {
        Some(frame) if !(closing && frame.code == CloseCode::Normal) => {
            // Only the numeric code: the reason text is untrusted.
            Err(SpeechError::failed(&format!(
                "the server closed the stream (code {})",
                u16::from(frame.code)
            )))
        }
        None if !closing => Err(SpeechError::failed("the server closed the stream early")),
        _ => Ok(()),
    }
}

fn unreachable_server() -> SpeechError {
    SpeechError::unavailable("cannot reach the transcription server")
}

fn connect_error(error: WsError) -> SpeechError {
    match error {
        WsError::Http(response) => {
            // The reply body is not read: its text is untrusted and 401s can quote the key.
            let status = response.status();
            if status == reqwest::StatusCode::PAYMENT_REQUIRED {
                SpeechError::unavailable("the account is out of credit (HTTP 402)")
            } else {
                net::status_error(status)
            }
        }
        WsError::Io(_) | WsError::Tls(_) => unreachable_server(),
        _ => SpeechError::failed("the transcription handshake failed"),
    }
}

fn stream_error(error: &WsError) -> SpeechError {
    match error {
        WsError::Capacity(_) => SpeechError::failed("the server sent a message that is too large"),
        WsError::Io(_) | WsError::ConnectionClosed | WsError::AlreadyClosed => {
            SpeechError::failed("the transcription connection was lost")
        }
        _ => SpeechError::failed("the transcription stream broke"),
    }
}
