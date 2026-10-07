//! The background half of the TTS engine: one JSON POST per pushed text, run
//! one after another so the audio keeps the order of the text.

use std::sync::Arc;
use std::time::Duration;

use aulo_speech::SpeechError;
use reqwest::Response;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::net::{net_error, status_error, timed_out};
use crate::reply::{Chunker, Msg, Stop, chunk_bytes, conclude};
use crate::tts::{SAMPLE_RATE_HZ, Shared};

const NOUN: &str = "speech";
pub(crate) struct Job {
    pub(crate) model: String,
    pub(crate) voice: String,
    pub(crate) speed: f64,
}

pub(crate) async fn run(
    shared: Arc<Shared>,
    job: Job,
    mut texts: mpsc::Receiver<String>,
    audio: mpsc::Sender<Msg>,
) {
    let mut total = 0usize;
    let outcome = async {
        while let Some(text) = texts.recv().await {
            synthesize(&shared, &job, &text, &audio, &mut total).await?;
        }
        Ok(())
    }
    .await;
    conclude(&audio, outcome).await;
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
    let mut chunker = Chunker::new(chunk_bytes(SAMPLE_RATE_HZ));
    while let Some(chunk) = timeout(shared.timeout, response.chunk())
        .await
        .map_err(|_| timed_out(NOUN))?
        .map_err(|e| net_error(NOUN, e))?
    {
        *total += chunk.len();
        if *total > shared.max_audio_bytes {
            return Err(SpeechError::failed("audio is too long").into());
        }
        // A full queue makes this wait, which stops reading the body: the server is slowed, not buffered.
        chunker.feed(&chunk, audio).await?;
    }
    chunker.clear();
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
