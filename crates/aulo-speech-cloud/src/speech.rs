//! The background half of the TTS engine: one JSON POST per pushed text, run
//! one after another so the audio keeps the order of the text.

use std::mem;
use std::sync::Arc;
use std::time::Duration;

use aulo_speech::SpeechError;
use reqwest::Response;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::net::{net_error, status_error, timed_out};
use crate::tts::Shared;

const NOUN: &str = "speech";
/// 100 ms of 24 kHz 16-bit mono. Small chunks keep `cancel` and first audio quick.
pub(crate) const CHUNK_BYTES: usize = 4800;

/// What the background request reports back to `poll`.
#[derive(Debug)]
pub(crate) enum Msg {
    /// Whole 16-bit samples, little endian.
    Audio(Vec<u8>),
    End,
    Failed(SpeechError),
}

pub(crate) struct Job {
    pub(crate) model: String,
    pub(crate) voice: String,
    pub(crate) speed: f64,
}

enum Stop {
    /// The reply was cancelled; nobody wants the rest.
    Closed,
    Failed(SpeechError),
}

impl From<SpeechError> for Stop {
    fn from(error: SpeechError) -> Self {
        Self::Failed(error)
    }
}

pub(crate) async fn run(
    shared: Arc<Shared>,
    job: Job,
    mut texts: mpsc::Receiver<String>,
    audio: mpsc::Sender<Msg>,
) {
    let mut total = 0usize;
    let verdict = loop {
        let Some(text) = texts.recv().await else {
            break Msg::End;
        };
        match synthesize(&shared, &job, &text, &audio, &mut total).await {
            Ok(()) => {}
            Err(Stop::Closed) => return,
            Err(Stop::Failed(error)) => break Msg::Failed(error),
        }
    };
    let _ = audio.send(verdict).await;
}

async fn synthesize(
    shared: &Shared,
    job: &Job,
    text: &str,
    audio: &mpsc::Sender<Msg>,
    total: &mut usize,
) -> Result<(), Stop> {
    let body = serde_json::to_vec(&json!({
        "model": job.model,
        "input": text,
        "voice": job.voice,
        // Raw 24 kHz 16-bit mono samples: no container to parse, so chunks play as they arrive.
        "response_format": "pcm",
        "speed": job.speed,
    }))
    .map_err(|_| SpeechError::failed("cannot build the speech request"))?;
    let mut post = shared
        .client
        .post(shared.endpoint.clone())
        .header(CONTENT_TYPE, "application/json")
        .body(body);
    if let Some(bearer) = &shared.bearer {
        post = post.header(AUTHORIZATION, bearer.clone());
    }
    let mut response = within(shared.timeout, post.send()).await?;
    let status = response.status();
    if !status.is_success() {
        // The reply body is not read: its text is untrusted and 401s quote the key.
        return Err(status_error(status).into());
    }
    let mut pending = Vec::with_capacity(CHUNK_BYTES);
    while let Some(chunk) = timeout(shared.timeout, response.chunk())
        .await
        .map_err(|_| timed_out(NOUN))?
        .map_err(|e| net_error(NOUN, e))?
    {
        *total += chunk.len();
        if *total > shared.max_audio_bytes {
            return Err(SpeechError::failed("audio is too long").into());
        }
        let mut rest = chunk.as_ref();
        while !rest.is_empty() {
            let (take, tail) = rest.split_at(rest.len().min(CHUNK_BYTES - pending.len()));
            pending.extend_from_slice(take);
            rest = tail;
            if pending.len() == CHUNK_BYTES {
                send(audio, &mut pending).await?;
            }
        }
        // Waiting for a full chunk would delay audio the server already sent.
        send(audio, &mut pending).await?;
    }
    // A half sample at the end of a reply is noise, and would shift the next reply.
    pending.clear();
    Ok(())
}

/// Sends the whole samples of `pending` and keeps a trailing odd byte, which
/// is the first half of a sample that straddles two network chunks.
async fn send(audio: &mpsc::Sender<Msg>, pending: &mut Vec<u8>) -> Result<(), Stop> {
    let even = pending.len() & !1;
    if even == 0 {
        return Ok(());
    }
    let odd = pending.get(even).copied();
    pending.truncate(even);
    let chunk = mem::replace(pending, Vec::with_capacity(CHUNK_BYTES));
    // A full queue makes this wait, which stops reading the body: the server is slowed, not buffered.
    audio
        .send(Msg::Audio(chunk))
        .await
        .map_err(|_| Stop::Closed)?;
    pending.extend(odd);
    Ok(())
}

async fn within(
    limit: Duration,
    request: impl Future<Output = Result<Response, reqwest::Error>>,
) -> Result<Response, SpeechError> {
    timeout(limit, request)
        .await
        .map_err(|_| timed_out(NOUN))?
        .map_err(|e| net_error(NOUN, e))
}
