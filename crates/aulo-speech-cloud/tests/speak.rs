//! The TTS engine against local HTTP servers: chunked PCM streaming, the
//! request the server sees, error mapping, the caps and cancel.

// Helpers outside `#[test]` fns are not covered by clippy's allow-in-tests.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use aulo_speech::{EngineId, EngineSpec, SpeechError, SpeechRate, TtsEngine, TtsPoll};
use aulo_speech_cloud::{ApiKey, CloudTtsConfig, CloudTtsFactory, SAMPLE_RATE_HZ, openai_voices};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Handle;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

mod common;
use common::{POLL_LIMIT, POLL_PAUSE, collect, expected, pcm, request};

const KEY: &str = "sk-test-secret-key";
const MODEL: &str = "gpt-test-tts";
const CHUNK_BYTES: usize = 4800;
const PCM: &str = "application/octet-stream";

fn config(base: &str) -> CloudTtsConfig {
    let mut config = CloudTtsConfig::new(
        "Test",
        format!("{base}/v1"),
        MODEL,
        openai_voices(),
        "alloy",
    );
    config.api_key = Some(ApiKey::new(KEY));
    config
}

fn spec(voice: Option<&str>) -> EngineSpec {
    EngineSpec {
        engine: EngineId::new("openai-tts").unwrap(),
        model: None,
        voice: voice.map(str::to_owned),
        rate: SpeechRate::NORMAL,
    }
}

fn engine(config: CloudTtsConfig) -> Box<dyn TtsEngine> {
    CloudTtsFactory::new(config, Handle::current())
        .unwrap()
        .build(&spec(None))
        .unwrap()
}

fn speak(engine: &mut dyn TtsEngine, texts: &[&str]) -> Result<(), SpeechError> {
    engine.begin(&request(None, 1.0))?;
    for text in texts {
        engine.push_text(text)?;
    }
    engine.finish()
}

async fn serve(response: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/speech"))
        .respond_with(response)
        .mount(&server)
        .await;
    server
}

#[tokio::test(flavor = "multi_thread")]
async fn a_body_arrives_as_bounded_chunks_in_order() {
    // Two and a half chunks, and a poll slice that divides neither.
    let samples = CHUNK_BYTES * 5 / 4;
    let server = serve(ResponseTemplate::new(200).set_body_raw(pcm(-1000, samples), PCM)).await;
    let mut engine = engine(config(&server.uri()));
    let format = engine.begin(&request(None, 1.0)).unwrap();
    assert_eq!(
        (format.sample_rate_hz(), format.channels()),
        (SAMPLE_RATE_HZ, 1)
    );
    engine.push_text("Hello there.").unwrap();
    engine.finish().unwrap();
    assert_eq!(
        collect(&mut *engine, 700).await.unwrap(),
        expected(-1000, samples)
    );
    // The reply is over; polling keeps saying so.
    assert_eq!(engine.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
}

#[tokio::test(flavor = "multi_thread")]
async fn each_pushed_text_is_its_own_request_played_in_order() {
    let server = MockServer::start().await;
    for (text, start) in [("First.", 0), ("Second.", 100)] {
        Mock::given(method("POST"))
            .and(path("/v1/audio/speech"))
            .and(body_json(json!({
                "model": MODEL, "input": text, "voice": "alloy",
                "response_format": "pcm", "speed": 1.0,
            })))
            .respond_with(ResponseTemplate::new(200).set_body_raw(pcm(start, 50), PCM))
            .expect(1)
            .mount(&server)
            .await;
    }
    let mut engine = engine(config(&server.uri()));
    speak(&mut *engine, &["First.", "Second."]).unwrap();
    let audio = collect(&mut *engine, 4096).await.unwrap();
    assert_eq!(audio, [expected(0, 50), expected(100, 50)].concat());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_request_carries_voice_speed_text_and_the_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/speech"))
        .and(header("authorization", format!("Bearer {KEY}").as_str()))
        .and(header("content-type", "application/json"))
        .and(body_json(json!({
            "model": MODEL, "input": "Привет, мир.", "voice": "nova",
            "response_format": "pcm", "speed": 1.5,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(pcm(0, 10), PCM))
        .expect(1)
        .mount(&server)
        .await;
    let mut engine = engine(config(&server.uri()));
    engine.begin(&request(Some("nova"), 1.5)).unwrap();
    // The text is trimmed: whitespace is not speech.
    engine.push_text("  Привет, мир.\n").unwrap();
    engine.finish().unwrap();
    assert_eq!(collect(&mut *engine, 64).await.unwrap().len(), 10);
}

#[tokio::test(flavor = "multi_thread")]
async fn voices_come_from_config_and_the_spec_voice_is_the_default() {
    let server = serve(ResponseTemplate::new(200).set_body_raw(pcm(0, 4), PCM)).await;
    let mut custom = config(&server.uri());
    custom.voices.truncate(2);
    custom.default_voice = custom.voices[0].id.clone();
    let factory = CloudTtsFactory::new(custom, Handle::current()).unwrap();

    let mut engine = factory.build(&spec(None)).unwrap();
    let listed: Vec<_> = engine.voices().iter().map(|v| v.id.as_str()).collect();
    assert_eq!(listed, ["alloy", "ash"]);
    assert_eq!(engine.info().id.as_str(), "openai-tts");
    assert!(engine.info().capabilities.needs_network);
    assert!(matches!(
        engine.begin(&request(Some("nova"), 1.0)),
        Err(SpeechError::Unsupported(_))
    ));
    assert!(matches!(
        factory.build(&spec(Some("nova"))),
        Err(SpeechError::Unsupported(_))
    ));
    // The spec's voice is what a request without one gets.
    let mut ash = factory.build(&spec(Some("ash"))).unwrap();
    speak(&mut *ash, &["Hi."]).unwrap();
    collect(&mut *ash, 16).await.unwrap();
    let seen = &server.received_requests().await.unwrap();
    assert!(String::from_utf8_lossy(&seen[0].body).contains("\"voice\":\"ash\""));
}

#[tokio::test]
async fn invalid_config_is_rejected_up_front() {
    let handle = Handle::current();
    let build = |edit: fn(&mut CloudTtsConfig)| {
        let mut config = config("https://api.example.com");
        edit(&mut config);
        CloudTtsFactory::new(config, handle.clone()).unwrap_err()
    };
    for error in [
        build(|c| c.default_voice = "missing".into()),
        build(|c| c.base_url = "http://api.example.com/v1".into()),
        build(|c| c.max_text_chars = 0),
        build(|c| c.max_audio_bytes = usize::MAX),
        build(|c| c.request_timeout = Duration::ZERO),
    ] {
        assert!(matches!(error, SpeechError::Invalid { .. }), "{error:?}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn http_failures_map_to_fallback_categories_without_echoing_the_body() {
    for (status, unavailable) in [
        (401, true),
        (403, true),
        (429, true),
        (400, false),
        (404, false),
        (500, false),
    ] {
        let quoted = format!("incorrect key {KEY} forged-line");
        let server = serve(ResponseTemplate::new(status).set_body_string(quoted)).await;
        let mut engine = engine(config(&server.uri()));
        speak(&mut *engine, &["Hi."]).unwrap();
        let error = collect(&mut *engine, 64).await.unwrap_err();
        assert_eq!(
            matches!(error, SpeechError::Unavailable(_)),
            unavailable,
            "{status}"
        );
        assert!(matches!(
            error,
            SpeechError::Unavailable(_) | SpeechError::Failed(_)
        ));
        assert!(
            error.to_string().contains(&format!("HTTP {status}")),
            "{error}"
        );
        let dump = format!("{error:?}{error}");
        assert!(!dump.contains(KEY) && !dump.contains("forged"), "{dump}");
        // The reply is over after the error.
        assert_eq!(engine.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unreachable_server_is_unavailable() {
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        format!("http://{}", listener.local_addr().unwrap())
    };
    let mut engine = engine(config(&closed));
    speak(&mut *engine, &["Hi."]).unwrap();
    let error = collect(&mut *engine, 64).await.unwrap_err();
    assert!(matches!(error, SpeechError::Unavailable(_)), "{error:?}");
    assert!(!format!("{error:?}").contains(KEY));
}

#[tokio::test(flavor = "multi_thread")]
async fn text_longer_than_the_cap_is_refused_and_never_sent() {
    let server = serve(ResponseTemplate::new(200).set_body_raw(pcm(0, 4), PCM)).await;
    let mut limited = config(&server.uri());
    limited.max_text_chars = 10;
    let mut engine = engine(limited);
    engine.begin(&request(None, 1.0)).unwrap();
    // Ten characters in more than ten bytes fit; eleven do not.
    engine.push_text("ёёёёёёёёёё").unwrap();
    let error = engine.push_text("ёёёёёёёёёёё").unwrap_err();
    assert!(matches!(error, SpeechError::Invalid { what: "text", .. }));
    engine.finish().unwrap();
    collect(&mut *engine, 16).await.unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn too_much_audio_fails_the_reply() {
    let server = serve(ResponseTemplate::new(200).set_body_raw(pcm(0, 6000), PCM)).await;
    let mut limited = config(&server.uri());
    limited.max_audio_bytes = 4800;
    let mut engine = engine(limited);
    speak(&mut *engine, &["Hi."]).unwrap();
    let error = collect(&mut *engine, 8192).await.unwrap_err();
    assert!(matches!(error, SpeechError::Failed(_)), "{error:?}");
    assert!(error.to_string().contains("too long"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_full_text_queue_overflows_instead_of_blocking() {
    // The first request hangs, so nothing drains the queue.
    let server = serve(
        ResponseTemplate::new(200)
            .set_body_raw(pcm(0, 4), PCM)
            .set_delay(Duration::from_secs(30)),
    )
    .await;
    let mut engine = engine(config(&server.uri()));
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
async fn empty_text_sends_nothing_and_ends_at_once() {
    let server = serve(ResponseTemplate::new(200).set_body_raw(pcm(0, 4), PCM)).await;
    let mut engine = engine(config(&server.uri()));
    speak(&mut *engine, &["", "  \n"]).unwrap();
    assert!(collect(&mut *engine, 16).await.unwrap().is_empty());
    speak(&mut *engine, &[]).unwrap();
    assert!(collect(&mut *engine, 16).await.unwrap().is_empty());
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn calls_out_of_order_are_errors() {
    let server = serve(ResponseTemplate::new(200)).await;
    let mut engine = engine(config(&server.uri()));
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
}

/// A server that streams in the shape wiremock cannot: headers and one chunk
/// now, the rest on the script's schedule. The script gets the connection.
async fn raw_server<F, Fut>(script: F) -> String
where
    F: FnOnce(TcpStream) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        // The request headers and body are small enough to arrive in one read.
        let mut buffer = [0; 4096];
        let _ = stream.read(&mut buffer).await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/octet-stream\r\ntransfer-encoding: chunked\r\n\r\n")
            .await
            .unwrap();
        script(stream).await;
    });
    base
}

async fn write_chunk(stream: &mut TcpStream, data: &[u8]) -> std::io::Result<()> {
    stream
        .write_all(format!("{:x}\r\n", data.len()).as_bytes())
        .await?;
    stream.write_all(data).await?;
    stream.write_all(b"\r\n").await
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_mid_stream_returns_at_once_and_drops_the_connection() {
    let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
    let base = raw_server(|mut stream| async move {
        write_chunk(&mut stream, &pcm(0, 100)).await.unwrap();
        // The reply never ends; only the client closing the socket ends the read.
        let mut buffer = [0; 64];
        while stream.read(&mut buffer).await.is_ok_and(|n| n > 0) {}
        let _ = closed_tx.send(());
    })
    .await;
    let mut engine = engine(config(&base));
    speak(&mut *engine, &["Hi."]).unwrap();
    let mut out = [0.0; 256];
    let mut first = 0;
    for _ in 0..POLL_LIMIT {
        if let TtsPoll::Audio { samples } = engine.poll(&mut out).unwrap() {
            first = samples;
            break;
        }
        tokio::time::sleep(POLL_PAUSE).await;
    }
    assert_eq!(first, 100);
    let started = Instant::now();
    engine.cancel();
    assert!(started.elapsed() < Duration::from_millis(50));
    assert_eq!(engine.poll(&mut out).unwrap(), TtsPoll::Done);
    tokio::time::timeout(Duration::from_secs(2), closed_rx)
        .await
        .expect("the request was not aborted")
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_reply_replaces_one_in_flight() {
    let base = raw_server(|mut stream| async move {
        write_chunk(&mut stream, &pcm(0, 100)).await.unwrap();
        tokio::time::sleep(Duration::from_secs(30)).await;
    })
    .await;
    let mut engine = engine(config(&base));
    speak(&mut *engine, &["Hi."]).unwrap();
    // `begin` implies `cancel`: the second reply starts clean, with nothing of the first.
    engine.begin(&request(None, 1.0)).unwrap();
    engine.finish().unwrap();
    assert!(collect(&mut *engine, 256).await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stalled_stream_delivers_its_audio_then_fails() {
    let base = raw_server(|mut stream| async move {
        write_chunk(&mut stream, &pcm(7, 100)).await.unwrap();
        tokio::time::sleep(Duration::from_secs(30)).await;
    })
    .await;
    let mut stalled = config(&base);
    stalled.request_timeout = Duration::from_millis(300);
    let mut engine = engine(stalled);
    speak(&mut *engine, &["Hi."]).unwrap();
    let mut out = [0.0; 256];
    let mut heard = 0;
    let error = loop {
        match engine.poll(&mut out) {
            Ok(TtsPoll::Audio { samples }) => heard += samples,
            Ok(TtsPoll::Pending) => tokio::time::sleep(POLL_PAUSE).await,
            Ok(TtsPoll::Done) => panic!("a cut-off reply must not look complete"),
            Err(error) => break error,
        }
    };
    assert_eq!(heard, 100);
    assert!(matches!(error, SpeechError::Failed(_)));
    assert!(error.to_string().contains("timed out"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unpolled_engine_slows_the_server_instead_of_buffering() {
    let written = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&written);
    let base = raw_server(|mut stream| async move {
        let chunk = vec![0u8; CHUNK_BYTES];
        // Never ends: with no reader this blocks once socket buffers and the audio queue fill.
        while write_chunk(&mut stream, &chunk).await.is_ok() {
            counter.fetch_add(chunk.len(), Ordering::Relaxed);
        }
    })
    .await;
    let mut engine = engine(config(&base));
    speak(&mut *engine, &["Hi."]).unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    // Unbounded buffering would have moved the 14 MiB default cap by now.
    let sent = written.load(Ordering::Relaxed);
    assert!(
        sent < 8 * 1024 * 1024,
        "{sent} bytes were read with nobody polling"
    );
    engine.cancel();
}
