//! Helpers the engine tests share: drive an engine to the end and build the
//! PCM it should play.

// Each test crate uses a different part of this module.
#![allow(dead_code, clippy::unwrap_used, clippy::panic)]

use std::time::Duration;

use aulo_speech::{SpeechError, SpeechRate, TtsEngine, TtsPoll, TtsRequest, TurnId};

pub const POLL_PAUSE: Duration = Duration::from_millis(10);
pub const POLL_LIMIT: usize = 500;

pub fn request<'a>(voice: Option<&'a str>, rate: f32) -> TtsRequest<'a> {
    TtsRequest {
        turn_id: TurnId::default(),
        voice,
        language: None,
        rate: SpeechRate::new(rate).unwrap(),
    }
}

/// Little-endian samples `start`, `start + 1`, ... so a lost or reordered byte shows.
pub fn pcm(start: i16, samples: usize) -> Vec<u8> {
    (0..samples)
        .flat_map(|i| (start + i as i16).to_le_bytes())
        .collect()
}

pub fn expected(start: i16, samples: usize) -> Vec<f32> {
    (0..samples)
        .map(|i| f32::from(start + i as i16) / 32768.0)
        .collect()
}

pub async fn collect(engine: &mut dyn TtsEngine, slice: usize) -> Result<Vec<f32>, SpeechError> {
    let mut out = vec![0.0; slice];
    let mut audio = Vec::new();
    for _ in 0..POLL_LIMIT {
        match engine.poll(&mut out)? {
            TtsPoll::Audio { samples } => {
                assert!(samples <= slice);
                audio.extend_from_slice(&out[..samples]);
            }
            TtsPoll::Done => return Ok(audio),
            TtsPoll::Pending => tokio::time::sleep(POLL_PAUSE).await,
        }
    }
    panic!("the reply did not finish in time");
}
