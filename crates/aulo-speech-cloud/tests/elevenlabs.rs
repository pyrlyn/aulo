//! The ElevenLabs engine against a local WebSocket server that checks the
//! messages the client sends and replays server messages in the documented
//! shape: a recorded session, error mapping, the caps, cancel and backpressure.

// Helpers outside `#[test]` fns are not covered by clippy's allow-in-tests.
// `result_large_err`: the handshake callback's `ErrorResponse` is fixed by tokio-tungstenite.
#![allow(clippy::unwrap_used, clippy::panic, clippy::result_large_err)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aulo_speech::{EngineId, EngineSpec, SpeechError, SpeechRate, TtsEngine, TtsPoll};
use aulo_speech_cloud::{ApiKey, ElevenLabsTtsConfig, ElevenLabsTtsFactory};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{Message, Utf8Bytes};
use tokio_tungstenite::{WebSocketStream, accept_hdr_async};

mod common;
use common::{collect, expected, pcm, request};

const KEY: &str = "xi-test-secret-key";
const VOICE: &str = "21m00Tcm4TlvDq8ikWAM";
const MODEL: &str = "eleven_flash_v2_5";
const CHUNK_BYTES: usize = 4800;

type Ws = WebSocketStream<TcpStream>;

/// What the server saw during the handshake and afterwards.
struct Observed {
    uri: String,
    key: Option<String>,
    messages: Vec<Value>,
}

fn voices() -> Vec<aulo_speech::Voice> {
    vec![voice(VOICE), voice("pNInz6obpgDQGcFmaJgB")]
}

fn voice(id: &str) -> aulo_speech::Voice {
    aulo_speech::Voice {
        id: id.to_owned(),
        name: id.to_owned(),
        language: None,
    }
}

fn config(base: &str) -> ElevenLabsTtsConfig {
    let mut config = ElevenLabsTtsConfig::new(MODEL, voices(), VOICE);
    config.tts.base_url = format!("{base}/v1");
    config.tts.api_key = Some(ApiKey::new(KEY));
    config.tts.request_timeout = Duration::from_secs(5);
    config
}

fn engine(config: ElevenLabsTtsConfig) -> Box<dyn TtsEngine> {
    ElevenLabsTtsFactory::new(config, Handle::current())
        .unwrap()
        .build(&EngineSpec {
            engine: EngineId::new("elevenlabs-tts").unwrap(),
            model: None,
            voice: None,
            rate: SpeechRate::NORMAL,
        })
        .unwrap()
}

fn speak(engine: &mut dyn TtsEngine, texts: &[&str]) -> Result<(), SpeechError> {
    engine.begin(&request(None, 1.0))?;
    for text in texts {
        engine.push_text(text)?;
    }
    engine.finish()
}

/// A server frame in the documented shape.
fn audio_frame(bytes: &[u8]) -> Message {
    text(&json!({
        "audio": STANDARD.encode(bytes),
        "alignment": {"charStartTimesMs": [0, 70], "charDurationsMs": [70, 80], "chars": ["H", "i"]},
        "normalizedAlignment": {"charStartTimesMs": [0, 70], "charDurationsMs": [70, 80], "chars": ["H", "i"]},
    }))
}

fn text(value: &Value) -> Message {
    Message::Text(Utf8Bytes::from(value.to_string()))
}

fn final_frame() -> Message {
    text(&json!({"isFinal": true, "audio": null}))
}

fn close(code: CloseCode) -> Message {
    Message::Close(Some(CloseFrame {
        code,
        reason: Utf8Bytes::from_static("secret server reason"),
    }))
}

/// The next client message as JSON; `None` once the client closed.
async fn next_json(ws: &mut Ws) -> Option<Value> {
    loop {
        match ws.next().await? {
            Ok(Message::Text(text)) => return Some(serde_json::from_str(&text).unwrap()),
            Ok(Message::Close(_)) | Err(_) => return None,
            Ok(_) => {}
        }
    }
}

/// Reads client messages up to and including the closing `{"text": ""}`.
async fn until_closed(ws: &mut Ws) -> Vec<Value> {
    let mut messages = Vec::new();
    while let Some(message) = next_json(ws).await {
        let last = message["text"] == "";
        messages.push(message);
        if last {
            break;
        }
    }
    messages
}

/// Accepts one client and runs `script` on it. `reject` answers the handshake
/// with that status and a body that quotes the key, the way a 401 does.
async fn serve_with<F, Fut>(reject: Option<u16>, script: F) -> (String, JoinHandle<Observed>)
where
    F: FnOnce(Ws) -> Fut + Send + 'static,
    Fut: Future<Output = Vec<Value>> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("ws://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let seen = Arc::new(Mutex::new((String::new(), None)));
        let record = Arc::clone(&seen);
        let ws = accept_hdr_async(stream, move |request: &Request, response: Response| {
            let key = request
                .headers()
                .get("xi-api-key")
                .map(|v| v.to_str().unwrap().to_owned());
            *record.lock().unwrap() = (request.uri().to_string(), key);
            if let Some(status) = reject {
                let mut error = ErrorResponse::new(Some(format!("bad key {KEY}")));
                *error.status_mut() = StatusCode::from_u16(status).unwrap();
                return Err(error);
            }
            Ok(response)
        })
        .await;
        let (uri, key) = seen.lock().unwrap().clone();
        let messages = match ws {
            Ok(ws) => script(ws).await,
            Err(_) => Vec::new(),
        };
        Observed { uri, key, messages }
    });
    (base, task)
}

async fn serve<F, Fut>(script: F) -> (String, JoinHandle<Observed>)
where
    F: FnOnce(Ws) -> Fut + Send + 'static,
    Fut: Future<Output = Vec<Value>> + Send + 'static,
{
    serve_with(None, script).await
}

/// Runs one reply against a server that plays `frames` after the client
/// closes, and returns what the engine did with them.
async fn replay(
    frames: Vec<Message>,
    config: impl FnOnce(&str) -> ElevenLabsTtsConfig,
) -> Result<Vec<f32>, SpeechError> {
    let (base, _server) = serve(move |mut ws| async move {
        let messages = until_closed(&mut ws).await;
        for frame in frames {
            if ws.send(frame).await.is_err() {
                break;
            }
        }
        messages
    })
    .await;
    let mut engine = engine(config(&base));
    speak(&mut *engine, &["Hi."]).unwrap();
    collect(&mut *engine, 8192).await
}

#[tokio::test(flavor = "multi_thread")]
async fn a_recorded_session_is_spoken_in_order_and_matches_the_documented_messages() {
    // Awkward cuts: odd byte counts straddle samples, and two chunks and a half in all.
    let samples = CHUNK_BYTES * 5 / 4;
    let all = pcm(-3000, samples);
    let (base, server) = serve(|mut ws| async move {
        let mut messages = Vec::new();
        // Audio for the first sentence arrives before the second one is sent.
        messages.push(next_json(&mut ws).await.unwrap());
        messages.push(next_json(&mut ws).await.unwrap());
        ws.send(audio_frame(&all[..4801])).await.unwrap();
        messages.extend(until_closed(&mut ws).await);
        ws.send(audio_frame(&all[4801..9001])).await.unwrap();
        ws.send(text(&json!({"audio": null, "alignment": null})))
            .await
            .unwrap();
        ws.send(audio_frame(&all[9001..])).await.unwrap();
        ws.send(final_frame()).await.unwrap();
        ws.send(close(CloseCode::Normal)).await.unwrap();
        messages
    })
    .await;
    let mut engine = engine(config(&base));
    let format = engine.begin(&request(None, 1.0)).unwrap();
    assert_eq!((format.sample_rate_hz(), format.channels()), (24_000, 1));
    engine.push_text("Hello there.").unwrap();
    engine.push_text("  Second one.\n").unwrap();
    engine.finish().unwrap();
    assert_eq!(
        collect(&mut *engine, 700).await.unwrap(),
        expected(-3000, samples)
    );
    assert_eq!(engine.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);

    let seen = server.await.unwrap();
    assert_eq!(
        seen.messages,
        [
            json!({"text": " "}),
            json!({"text": "Hello there. ", "flush": true}),
            json!({"text": "Second one. ", "flush": true}),
            json!({"text": ""}),
        ]
    );
    let (path, query) = seen.uri.split_once('?').unwrap();
    assert_eq!(path, format!("/v1/text-to-speech/{VOICE}/stream-input"));
    let mut pairs: Vec<_> = query.split('&').collect();
    pairs.sort_unstable();
    assert_eq!(
        pairs,
        [
            "inactivity_timeout=20",
            &format!("model_id={MODEL}"),
            "output_format=pcm_24000"
        ]
    );
    assert_eq!(seen.key.as_deref(), Some(KEY));
    assert!(!seen.uri.contains(KEY));
}

#[tokio::test(flavor = "multi_thread")]
async fn voice_model_rate_and_speed_come_from_the_request_and_config() {
    let (base, server) = serve(|mut ws| async move {
        let messages = until_closed(&mut ws).await;
        ws.send(final_frame()).await.unwrap();
        messages
    })
    .await;
    let mut custom = config(&base);
    custom.sample_rate_hz = 16_000;
    custom.inactivity_timeout = Duration::from_secs(90);
    let mut engine = engine(custom);
    // Out of the engine's 0.7 to 1.2 range, so it is clamped.
    let format = engine
        .begin(&request(Some("pNInz6obpgDQGcFmaJgB"), 1.5))
        .unwrap();
    assert_eq!(format.sample_rate_hz(), 16_000);
    engine.push_text("Hi.").unwrap();
    engine.finish().unwrap();
    collect(&mut *engine, 64).await.unwrap();
    let seen = server.await.unwrap();
    assert_eq!(
        seen.messages[0],
        json!({"text": " ", "voice_settings": {"speed": 1.2}})
    );
    assert!(
        seen.uri
            .starts_with("/v1/text-to-speech/pNInz6obpgDQGcFmaJgB/stream-input?")
    );
    assert!(seen.uri.contains("output_format=pcm_16000"));
    assert!(seen.uri.contains("inactivity_timeout=90"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_close_without_final_after_the_client_closed_still_ends_the_reply() {
    let audio = vec![audio_frame(&pcm(5, 100)), close(CloseCode::Normal)];
    assert_eq!(replay(audio, config).await.unwrap(), expected(5, 100));
}

#[tokio::test(flavor = "multi_thread")]
async fn nothing_said_sends_no_text_and_ends_at_once() {
    let (base, server) = serve(|mut ws| async move {
        let mut messages = Vec::new();
        while let Some(message) = next_json(&mut ws).await {
            messages.push(message);
        }
        // Reading on completes the close handshake the client started.
        while ws.next().await.is_some() {}
        messages
    })
    .await;
    let mut engine = engine(config(&base));
    speak(&mut *engine, &["", "  \n"]).unwrap();
    // The server never speaks; the reply ends because the client closed the socket.
    assert!(collect(&mut *engine, 16).await.unwrap().is_empty());
    assert_eq!(server.await.unwrap().messages, [json!({"text": " "})]);
}

#[tokio::test(flavor = "multi_thread")]
async fn handshake_failures_map_to_fallback_categories_without_echoing_the_body() {
    for (status, unavailable) in [
        (401, true),
        (403, true),
        (429, true),
        (500, false),
        (404, false),
    ] {
        let (base, _server) = serve_with(Some(status), |_| async { Vec::new() }).await;
        let mut engine = engine(config(&base));
        speak(&mut *engine, &["Hi."]).unwrap();
        let error = collect(&mut *engine, 64).await.unwrap_err();
        let text = error.to_string();
        assert_eq!(
            matches!(error, SpeechError::Unavailable(_)),
            unavailable,
            "{status}: {error:?}"
        );
        assert!(matches!(
            error,
            SpeechError::Unavailable(_) | SpeechError::Failed(_)
        ));
        assert!(!text.contains(KEY) && !text.contains("bad key"), "{text}");
        assert!(text.contains(&status.to_string()), "{text}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unreachable_server_is_unavailable() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("ws://{}", listener.local_addr().unwrap());
    drop(listener);
    let mut engine = engine(config(&base));
    speak(&mut *engine, &["Hi."]).unwrap();
    let error = collect(&mut *engine, 64).await.unwrap_err();
    assert!(matches!(error, SpeechError::Unavailable(_)), "{error:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn server_errors_map_to_categories_and_never_echo_server_text() {
    let cases = [
        (
            json!({"error": "invalid api key xi-test-secret-key", "code": 401}),
            true,
        ),
        (json!({"message": "You have exceeded your quota"}), true),
        (json!({"message": "authorization failed"}), true),
        (json!({"error": "model exploded in secret place"}), false),
        (json!({"error": {"nested": "object"}}), false),
    ];
    for (frame, unavailable) in cases {
        let error = replay(vec![text(&frame)], config).await.unwrap_err();
        assert_eq!(
            matches!(error, SpeechError::Unavailable(_)),
            unavailable,
            "{frame}: {error:?}"
        );
        let message = error.to_string();
        assert!(
            !message.contains(KEY) && !message.contains("secret place"),
            "{message}"
        );
    }
    // A policy close is a refused key or quota; any other early close is a failure.
    let error = replay(vec![close(CloseCode::Policy)], config)
        .await
        .unwrap_err();
    assert!(matches!(error, SpeechError::Unavailable(_)), "{error:?}");
    assert!(!error.to_string().contains("secret server reason"));
}

#[tokio::test(flavor = "multi_thread")]
async fn broken_replies_fail_after_the_audio_before_them_is_delivered() {
    let (base, _server) = serve(|mut ws| async move {
        ws.send(audio_frame(&pcm(9, 300))).await.unwrap();
        ws.send(Message::Text("not json at all".into()))
            .await
            .unwrap();
        Vec::new()
    })
    .await;
    let mut engine = engine(config(&base));
    speak(&mut *engine, &["Hi."]).unwrap();
    let mut out = [0.0; 1024];
    let mut audio = Vec::new();
    let error = loop {
        match engine.poll(&mut out) {
            Ok(TtsPoll::Audio { samples }) => audio.extend_from_slice(&out[..samples]),
            Ok(TtsPoll::Pending) => tokio::time::sleep(Duration::from_millis(10)).await,
            Ok(TtsPoll::Done) => panic!("a broken reply ended cleanly"),
            Err(error) => break error,
        }
    };
    assert_eq!(audio, expected(9, 300));
    assert!(matches!(error, SpeechError::Failed(_)), "{error:?}");
    assert!(!error.to_string().contains("not json"));
    // Other malformed replies.
    let bad: [Message; 3] = [
        text(&json!({"audio": "***not base64***"})),
        Message::Binary(vec![1, 2, 3].into()),
        close(CloseCode::Away),
    ];
    for frame in bad {
        let error = replay(vec![frame], config).await.unwrap_err();
        assert!(matches!(error, SpeechError::Failed(_)), "{error:?}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn too_much_audio_or_a_huge_message_fails_the_reply() {
    let mut limited = |base: &str| {
        let mut config = config(base);
        config.tts.max_audio_bytes = 4800;
        config
    };
    let error = replay(
        vec![audio_frame(&pcm(0, 2000)), audio_frame(&pcm(0, 2000))],
        &mut limited,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, SpeechError::Failed(_)), "{error:?}");
    assert!(error.to_string().contains("too long"), "{error}");
    // One message above the 1 MiB cap is refused before it is decoded.
    let huge = Message::Text(Utf8Bytes::from("a".repeat(2 << 20)));
    let error = replay(vec![huge], config).await.unwrap_err();
    assert!(matches!(error, SpeechError::Failed(_)), "{error:?}");
    assert!(error.to_string().contains("too large"), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn text_caps_reject_before_anything_is_sent() {
    let (base, _server) = serve(|_| async { Vec::new() }).await;
    let mut small = config(&base);
    small.tts.max_text_chars = 10;
    small.max_reply_chars = 15;
    let mut engine = engine(small);
    engine.begin(&request(None, 1.0)).unwrap();
    assert!(matches!(
        engine.push_text("far longer than ten characters"),
        Err(SpeechError::Invalid { .. })
    ));
    engine.push_text("ten chars.").unwrap();
    // 10 + 6 is over the reply total.
    assert!(matches!(
        engine.push_text("sixsix"),
        Err(SpeechError::Invalid { .. })
    ));
    engine.push_text("five.").unwrap();
    assert!(matches!(
        engine.push_text("x"),
        Err(SpeechError::Invalid { .. })
    ));
    engine.cancel();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_full_text_queue_overflows_instead_of_blocking() {
    // The server accepts the socket and never reads, so nothing drains the queue.
    let (base, _server) = serve(|_ws| async {
        tokio::time::sleep(Duration::from_secs(30)).await;
        Vec::new()
    })
    .await;
    let mut engine = engine(config(&base));
    engine.begin(&request(None, 1.0)).unwrap();
    let started = Instant::now();
    let overflowed = (0..500)
        .filter(|_| {
            matches!(
                engine.push_text("Sentence."),
                Err(SpeechError::Overflow { .. })
            )
        })
        .count();
    assert!(overflowed > 400, "{overflowed}");
    assert!(started.elapsed() < Duration::from_secs(1));
    engine.cancel();
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_mid_stream_returns_at_once_and_drops_the_connection() {
    let (base, server) = serve(|mut ws| async move {
        ws.send(audio_frame(&pcm(0, 100))).await.unwrap();
        // The reply never ends; only the client going away ends the read.
        while ws.next().await.is_some_and(|m| m.is_ok()) {}
        Vec::new()
    })
    .await;
    let mut engine = engine(config(&base));
    speak(&mut *engine, &["Hi."]).unwrap();
    let mut out = [0.0; 256];
    let mut first = 0;
    for _ in 0..500 {
        if let TtsPoll::Audio { samples } = engine.poll(&mut out).unwrap() {
            first = samples;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(first, 100);
    let started = Instant::now();
    engine.cancel();
    assert!(started.elapsed() < Duration::from_millis(50));
    assert_eq!(engine.poll(&mut out).unwrap(), TtsPoll::Done);
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("the socket was not dropped")
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_reply_replaces_one_in_flight() {
    let (base, _first) = serve(|mut ws| async move {
        ws.send(audio_frame(&pcm(0, 100))).await.unwrap();
        tokio::time::sleep(Duration::from_secs(30)).await;
        Vec::new()
    })
    .await;
    let mut engine = engine(config(&base));
    speak(&mut *engine, &["Hi."]).unwrap();
    // `begin` implies `cancel`: the second reply starts clean, with nothing of the first.
    // Its own connection is refused, which is all this checks the first one is gone for.
    engine.begin(&request(None, 1.0)).unwrap();
    assert!(engine.poll(&mut [0.0; 8]).is_ok());
    engine.cancel();
    assert!(matches!(
        engine.push_text("late"),
        Err(SpeechError::OutOfOrder(_))
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_slow_consumer_slows_the_server_instead_of_filling_memory() {
    let sent = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&sent);
    let (base, server) = serve(move |mut ws| async move {
        let _ = next_json(&mut ws).await;
        // Half a MiB of audio per message; more than 60 MiB of it would not fit anywhere.
        let frame = audio_frame(&vec![0u8; 500_000]);
        for _ in 0..100 {
            if ws.send(frame.clone()).await.is_err() {
                break;
            }
            counter.fetch_add(1, Ordering::SeqCst);
        }
        Vec::new()
    })
    .await;
    let mut big = config(&base);
    big.tts.max_audio_bytes = 64 * 1024 * 1024;
    let mut engine = engine(big);
    speak(&mut *engine, &["Hi."]).unwrap();
    // Nobody polls: the audio queue fills and the socket stops being read.
    tokio::time::sleep(Duration::from_millis(700)).await;
    let before = sent.load(Ordering::SeqCst);
    assert!(before < 60, "{before} messages went out with no consumer");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        sent.load(Ordering::SeqCst),
        before,
        "the server was not held back"
    );
    // Polling lets it go on.
    let mut out = vec![0.0; 1 << 16];
    for _ in 0..200 {
        engine.poll(&mut out).unwrap();
    }
    engine.cancel();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_goes_silent_after_finish_times_out_after_its_audio() {
    let (base, _server) = serve(|mut ws| async move {
        let messages = until_closed(&mut ws).await;
        ws.send(audio_frame(&pcm(3, 50))).await.unwrap();
        tokio::time::sleep(Duration::from_secs(30)).await;
        messages
    })
    .await;
    let mut short = config(&base);
    short.tts.request_timeout = Duration::from_millis(300);
    let mut engine = engine(short);
    speak(&mut *engine, &["Hi."]).unwrap();
    let mut out = [0.0; 128];
    let mut audio = Vec::new();
    let error = loop {
        match engine.poll(&mut out) {
            Ok(TtsPoll::Audio { samples }) => audio.extend_from_slice(&out[..samples]),
            Ok(_) => tokio::time::sleep(Duration::from_millis(10)).await,
            Err(error) => break error,
        }
    };
    assert_eq!(audio, expected(3, 50));
    assert!(matches!(error, SpeechError::Failed(_)), "{error:?}");
    assert!(error.to_string().contains("timed out"), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn calls_out_of_order_are_errors() {
    let (base, _server) = serve(|_| async { Vec::new() }).await;
    let mut engine = engine(config(&base));
    assert!(matches!(
        engine.push_text("Hi."),
        Err(SpeechError::OutOfOrder(_))
    ));
    assert!(matches!(engine.finish(), Err(SpeechError::OutOfOrder(_))));
    assert_eq!(engine.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
    engine.begin(&request(None, 1.0)).unwrap();
    engine.finish().unwrap();
    assert!(matches!(
        engine.push_text("Hi."),
        Err(SpeechError::OutOfOrder(_))
    ));
    assert!(matches!(
        engine.begin(&request(Some("not-a-listed-voice"), 1.0)),
        Err(SpeechError::Unsupported(_))
    ));
}

#[tokio::test]
async fn the_config_is_validated_before_any_connection() {
    let build = |edit: &dyn Fn(&mut ElevenLabsTtsConfig)| {
        let mut config = ElevenLabsTtsConfig::new(MODEL, voices(), VOICE);
        config.tts.api_key = Some(ApiKey::new(KEY));
        edit(&mut config);
        ElevenLabsTtsFactory::new(config, Handle::current())
    };
    assert!(build(&|_| {}).is_ok());
    assert!(build(&|c| c.tts.base_url = "ws://127.0.0.1:9/v1".into()).is_ok());
    assert!(build(&|c| c.tts.base_url = "ws://localhost:9/v1".into()).is_ok());
    // A key never goes over plain ws to another host, nor over a non-socket scheme.
    for url in [
        "ws://api.example.com/v1",
        "http://127.0.0.1/v1",
        "https://api.elevenlabs.io/v1",
        "nonsense",
    ] {
        assert!(
            matches!(
                build(&|c| c.tts.base_url = url.into()),
                Err(SpeechError::Invalid { .. })
            ),
            "{url}"
        );
    }
    // Without a key plain ws is allowed anywhere, as for the HTTP engines.
    let keyless = {
        let mut config = ElevenLabsTtsConfig::new(MODEL, voices(), VOICE);
        config.tts.base_url = "ws://api.example.com/v1".into();
        ElevenLabsTtsFactory::new(config, Handle::current())
    };
    assert!(keyless.is_ok());
    assert!(build(&|c| c.sample_rate_hz = 12_345).is_err());
    assert!(build(&|c| c.inactivity_timeout = Duration::ZERO).is_err());
    assert!(build(&|c| c.inactivity_timeout = Duration::from_secs(181)).is_err());
    assert!(build(&|c| c.max_reply_chars = 0).is_err());
    // Voice ids go into the URL path.
    assert!(build(&|c| c.tts.voices.push(voice("../../v1/user"))).is_err());
    assert!(build(&|c| c.tts.voices.push(voice("a?b"))).is_err());
    assert!(build(&|c| c.tts.default_voice = "missing".into()).is_err());
}

#[tokio::test]
async fn the_key_stays_out_of_debug_output() {
    let factory = ElevenLabsTtsFactory::new(config("ws://127.0.0.1:9"), Handle::current()).unwrap();
    assert!(!format!("{factory:?}").contains(KEY));
}
