//! The background half of the engine: one multipart POST per utterance.

use std::num::NonZeroUsize;
use std::sync::Arc;

use aulo_speech::SpeechError;
use reqwest::Response;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use sse_core::{SseDecoder, SseEvent};
use tokio::sync::mpsc;

use crate::config::ResponseShape;
use crate::engine::{Event, MAX_TRANSCRIPT_BYTES, Shared};
use crate::net::{net_error, status_error};

/// A plain reply is `{"text": ...}` plus optional extras, so this is generous.
const MAX_JSON_BYTES: usize = 4 * MAX_TRANSCRIPT_BYTES;
/// One event is a token or a short text; the limit stops a server that never ends a line.
const MAX_EVENT_BYTES: NonZeroUsize = match NonZeroUsize::new(2 * MAX_TRANSCRIPT_BYTES) {
    Some(limit) => limit,
    None => NonZeroUsize::MIN,
};
/// The whole stream, since the per-event limit does not bound the event count.
const MAX_STREAM_BYTES: usize = 16 * MAX_TRANSCRIPT_BYTES;
const NOUN: &str = "transcription";
const DELTA_EVENT: &str = "transcript.text.delta";
const DONE_EVENT: &str = "transcript.text.done";

pub(crate) struct Request {
    pub(crate) wav: Vec<u8>,
    pub(crate) model: String,
    /// ISO 639-1, or `None` to let the server detect it.
    pub(crate) language: Option<String>,
}

#[derive(Deserialize)]
struct JsonReply {
    text: String,
}

#[derive(Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    kind: String,
    delta: Option<String>,
    text: Option<String>,
}

pub(crate) async fn run(shared: Arc<Shared>, request: Request, events: mpsc::Sender<Event>) {
    let last = match transcribe(&shared, request, &events).await {
        Ok(text) => Event::Final(text),
        Err(error) => Event::Failed(error),
    };
    // A closed channel means the utterance was cancelled; nobody wants the result.
    let _ = events.send(last).await;
}

async fn transcribe(
    shared: &Shared,
    request: Request,
    events: &mpsc::Sender<Event>,
) -> Result<String, SpeechError> {
    let file = Part::bytes(request.wav)
        .file_name("utterance.wav")
        .mime_str("audio/wav")
        .map_err(|_| SpeechError::failed("cannot build the upload"))?;
    let mut form = Form::new()
        .part("file", file)
        .text("model", request.model)
        .text("response_format", "json");
    if shared.shape == ResponseShape::Stream {
        form = form.text("stream", "true");
    }
    if let Some(language) = request.language {
        form = form.text("language", language);
    }
    let mut post = shared.client.post(shared.endpoint.clone()).multipart(form);
    if let Some(bearer) = &shared.bearer {
        post = post.header(AUTHORIZATION, bearer.clone());
    }
    let mut response = post.send().await.map_err(stt_error)?;
    let status = response.status();
    if !status.is_success() {
        // The reply body is not read: its text is untrusted and 401s quote the key.
        return Err(status_error(status));
    }
    // The shape comes from the reply: whisper-1 ignores `stream=true`.
    let is_stream = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/event-stream"));
    if is_stream {
        return read_stream(response, events).await;
    }
    let (body, truncated) = read_capped(&mut response, MAX_JSON_BYTES).await?;
    if truncated {
        return Err(SpeechError::failed("reply is too large"));
    }
    let reply: JsonReply = serde_json::from_slice(&body)
        .map_err(|_| SpeechError::failed("reply is not a transcription"))?;
    Ok(capped(&reply.text).trim().to_owned())
}

async fn read_stream(
    mut response: Response,
    events: &mpsc::Sender<Event>,
) -> Result<String, SpeechError> {
    let mut decoder = SseDecoder::with_limit(MAX_EVENT_BYTES);
    let mut text = String::new();
    let mut received = 0usize;
    while let Some(mut chunk) = response.chunk().await.map_err(stt_error)? {
        received += chunk.len();
        if received > MAX_STREAM_BYTES {
            return Err(SpeechError::failed("stream is too long"));
        }
        while let Some(event) = decoder.next(&mut chunk) {
            let event = event.map_err(|_| SpeechError::failed("stream event is too large"))?;
            let SseEvent::Message(message) = event else {
                continue;
            };
            // Anything that is not an event object (a `[DONE]` marker, a comment) carries no text.
            let Ok(event) = serde_json::from_str::<StreamEvent>(&message.data) else {
                continue;
            };
            match (event.kind.as_str(), event.delta, event.text) {
                (DELTA_EVENT, Some(delta), _) => {
                    push_capped(&mut text, &delta);
                    // A newer cumulative partial supersedes a dropped one.
                    let _ = events.try_send(Event::Partial(text.clone()));
                }
                (DONE_EVENT, _, Some(done)) => return Ok(capped(&done).trim().to_owned()),
                _ => {}
            }
        }
    }
    // A cut-off transcript could make the agent act on half a sentence.
    Err(SpeechError::failed(
        "stream ended before the final transcript",
    ))
}

/// Reads at most `cap` bytes; the flag says whether the body was longer.
async fn read_capped(response: &mut Response, cap: usize) -> Result<(Vec<u8>, bool), SpeechError> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(stt_error)? {
        let room = cap - body.len();
        if chunk.len() > room {
            body.extend_from_slice(&chunk[..room]);
            return Ok((body, true));
        }
        body.extend_from_slice(&chunk);
    }
    Ok((body, false))
}

fn stt_error(error: reqwest::Error) -> SpeechError {
    net_error(NOUN, error)
}

pub(crate) fn capped(text: &str) -> &str {
    &text[..text.floor_char_boundary(MAX_TRANSCRIPT_BYTES)]
}

pub(crate) fn push_capped(buffer: &mut String, piece: &str) {
    let room = MAX_TRANSCRIPT_BYTES.saturating_sub(buffer.len());
    buffer.push_str(&piece[..piece.floor_char_boundary(room)]);
}
