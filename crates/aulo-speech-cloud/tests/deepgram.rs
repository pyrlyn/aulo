//! The Deepgram engine against a local WebSocket server that replays messages
//! in the shape the live API reference documents: the handshake the server
//! sees, the frames the client sends, partial and final results, error
//! mapping to fallback categories, the caps, cancel and an empty utterance.

// Helpers outside `#[test]` fns are not covered by clippy's allow-in-tests.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::result_large_err
)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aulo_speech::{
    AudioFormat, AudioFrame, EngineId, EngineSpec, SpeechError, SpeechRate, SttEngine, SttPoll,
    Transcript, TranscriptKind, TurnId,
};
use aulo_speech_cloud::{
    ApiKey, DEEPGRAM_BASE_URL, DeepgramSttConfig, DeepgramSttFactory, MAX_TRANSCRIPT_BYTES,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{Message, Utf8Bytes};
use tokio_tungstenite::{WebSocketStream, accept_hdr_async};

const KEY: &str = "dg-test-secret-key";
const MODEL: &str = "nova-test";
const FRAME: usize = 320;
const FRAME_BYTES: usize = 2 * FRAME;
const FRAMES: usize = 50;
const POLL_PAUSE: Duration = Duration::from_millis(10);
const POLL_LIMIT: usize = 500;
const SECRET_TEXT: &str = "secret server text";

type Ws = WebSocketStream<TcpStream>;
type Seen = Vec<(TranscriptKind, String)>;

/// What the server saw during the handshake and afterwards.
#[derive(Default, Clone)]
struct Observed {
    uri: String,
    auth: Option<String>,
    binary: Vec<Vec<u8>>,
    texts: Vec<Value>,
}

enum Kind {
    Audio,
    Text(Value),
}

fn config(base: &str) -> DeepgramSttConfig {
    let mut config = DeepgramSttConfig::new("Test", format!("{base}/v1"), MODEL);
    config.api_key = Some(ApiKey::new(KEY));
    config.finish_timeout = Duration::from_secs(5);
    config
}

fn spec() -> EngineSpec {
    EngineSpec {
        engine: EngineId::new("deepgram-stt").unwrap(),
        model: None,
        voice: None,
        rate: SpeechRate::NORMAL,
    }
}

fn engine(config: DeepgramSttConfig) -> Box<dyn SttEngine> {
    DeepgramSttFactory::new(config, Handle::current())
        .unwrap()
        .build(&spec())
        .unwrap()
}

fn push_frames(engine: &mut dyn SttEngine, count: usize) -> Result<(), SpeechError> {
    let samples = [0.25_f32; FRAME];
    for index in 0..count {
        let position = (index * FRAME) as u64;
        engine.push(AudioFrame::new(&samples, AudioFormat::PIPELINE, position).unwrap())?;
    }
    Ok(())
}

async fn collect(engine: &mut dyn SttEngine, turn: TurnId) -> Result<Seen, SpeechError> {
    let mut out = Transcript::with_capacity(turn, 64);
    let mut seen = Vec::new();
    for _ in 0..POLL_LIMIT {
        match engine.poll(&mut out)? {
            SttPoll::Updated => seen.push((out.kind, out.text.clone())),
            SttPoll::Done => return Ok(seen),
            SttPoll::Pending => tokio::time::sleep(POLL_PAUSE).await,
        }
    }
    panic!("no final transcript in time");
}

async fn utterance(engine: &mut dyn SttEngine) -> Result<Seen, SpeechError> {
    let turn = TurnId::new();
    engine.begin(turn, Some("en-US"))?;
    push_frames(engine, FRAMES)?;
    engine.finish()?;
    collect(engine, turn).await
}

/// A `Results` message in the documented shape.
fn results(transcript: &str, is_final: bool, speech_final: bool) -> Message {
    text(&json!({
        "type": "Results",
        "channel_index": [0, 1],
        "duration": 1.5,
        "start": 0.0,
        "is_final": is_final,
        "speech_final": speech_final,
        "from_finalize": false,
        "channel": {"alternatives": [{
            "transcript": transcript,
            "confidence": 0.98,
            "words": [{"word": "hello", "start": 0.0, "end": 0.5, "confidence": 0.99,
                       "punctuated_word": "Hello"}],
        }]},
        "metadata": {
            "request_id": "550e8400-e29b-41d4-a716-446655440000",
            "model_info": {"name": "nova-3", "version": "2024-01-18.29447", "arch": "nova-3"},
            "model_uuid": "1842b5b2-ef26-4f13-bec5-48eaef70e9c0",
        },
    }))
}

fn metadata() -> Message {
    text(&json!({
        "type": "Metadata",
        "transaction_key": "deprecated",
        "request_id": "8c8ebea9-dbec-45fa-a035-e4632cb05b5f",
        "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "created": "2024-08-29T22:37:55.202Z",
        "duration": 1.0,
        "channels": 1,
    }))
}

fn text(value: &Value) -> Message {
    Message::Text(Utf8Bytes::from(value.to_string()))
}

fn close(code: CloseCode) -> Message {
    Message::Close(Some(CloseFrame {
        code,
        reason: Utf8Bytes::from_static(SECRET_TEXT),
    }))
}

/// The next client message, recorded; `None` once the client is gone.
async fn next(ws: &mut Ws, seen: &mut Observed) -> Option<Kind> {
    loop {
        match ws.next().await? {
            Ok(Message::Binary(bytes)) => {
                seen.binary.push(bytes.to_vec());
                return Some(Kind::Audio);
            }
            Ok(Message::Text(text)) => {
                let value: Value = serde_json::from_str(&text).unwrap();
                seen.texts.push(value.clone());
                return Some(Kind::Text(value));
            }
            Ok(Message::Close(_)) | Err(_) => return None,
            Ok(_) => {}
        }
    }
}

fn is_close_stream(value: &Value) -> bool {
    value["type"] == "CloseStream"
}

/// Reads until the client sends `CloseStream`, calling `on_audio` with the
/// number of audio frames seen so far for each one.
async fn until_close_stream(
    ws: &mut Ws,
    seen: &mut Observed,
    mut on_audio: impl FnMut(usize) -> Vec<Message>,
) {
    while let Some(kind) = next(ws, seen).await {
        match kind {
            Kind::Audio => {
                for frame in on_audio(seen.binary.len()) {
                    ws.send(frame).await.unwrap();
                }
            }
            Kind::Text(value) if is_close_stream(&value) => return,
            Kind::Text(_) => {}
        }
    }
}

/// Accepts one client and runs `script` on it. `reject` answers the handshake
/// with that status and a body that quotes the key, the way a 401 does.
async fn serve_with<F, Fut>(reject: Option<u16>, script: F) -> (String, JoinHandle<Observed>)
where
    F: FnOnce(Ws, Observed) -> Fut + Send + 'static,
    Fut: Future<Output = Observed> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("ws://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let handshake = Arc::new(Mutex::new(Observed::default()));
        let record = Arc::clone(&handshake);
        let ws = accept_hdr_async(stream, move |request: &Request, response: Response| {
            let mut seen = record.lock().unwrap();
            seen.uri = request.uri().to_string();
            seen.auth = request
                .headers()
                .get("authorization")
                .map(|v| v.to_str().unwrap().to_owned());
            if let Some(status) = reject {
                let mut error = ErrorResponse::new(Some(format!("bad key {KEY}")));
                *error.status_mut() = StatusCode::from_u16(status).unwrap();
                return Err(error);
            }
            Ok(response)
        })
        .await;
        let seen = handshake.lock().unwrap().clone();
        match ws {
            Ok(ws) => script(ws, seen).await,
            Err(_) => seen,
        }
    });
    (base, task)
}

async fn serve<F, Fut>(script: F) -> (String, JoinHandle<Observed>)
where
    F: FnOnce(Ws, Observed) -> Fut + Send + 'static,
    Fut: Future<Output = Observed> + Send + 'static,
{
    serve_with(None, script).await
}

/// Plays `frames` once the first audio arrives, then keeps reading.
async fn after_first_audio(base_frames: Vec<Message>) -> (String, JoinHandle<Observed>) {
    serve(move |mut ws, mut seen| async move {
        next(&mut ws, &mut seen).await;
        for frame in base_frames {
            if ws.send(frame).await.is_err() {
                break;
            }
        }
        while next(&mut ws, &mut seen).await.is_some() {}
        seen
    })
    .await
}

/// A server that accepts TCP connections and never answers the handshake. It
/// signals each connection when it opens and when the client closes it.
struct Stalled {
    base: String,
    opened: mpsc::UnboundedReceiver<()>,
    closed: mpsc::UnboundedReceiver<()>,
}

impl Stalled {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("ws://{}", listener.local_addr().unwrap());
        let (opened_tx, opened) = mpsc::unbounded_channel();
        let (closed_tx, closed) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let _ = opened_tx.send(());
                let closed_tx = closed_tx.clone();
                tokio::spawn(async move {
                    let mut buffer = [0u8; 4096];
                    while stream.read(&mut buffer).await.unwrap_or(0) > 0 {}
                    let _ = closed_tx.send(());
                });
            }
        });
        Self {
            base,
            opened,
            closed,
        }
    }

    async fn opened(&mut self) {
        let signal = tokio::time::timeout(Duration::from_secs(5), self.opened.recv());
        signal.await.expect("the engine never connected").unwrap();
    }

    async fn closed(&mut self) {
        let signal = tokio::time::timeout(Duration::from_secs(5), self.closed.recv());
        signal.await.expect("the socket stayed open").unwrap();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_recorded_session_streams_partials_and_ends_with_one_final() {
    let (base, server) = serve(|mut ws, mut seen| async move {
        until_close_stream(&mut ws, &mut seen, |frames| match frames {
            1 => vec![results("hello", false, false)],
            // The segment is committed; the next interim covers only the audio after it.
            20 => vec![
                results("hello", false, false),
                results("Hello world.", true, false),
            ],
            30 => vec![results("", false, false), results("how are", false, false)],
            _ => Vec::new(),
        })
        .await;
        // `CloseStream` flushes what the server holds, then it says goodbye.
        for frame in [
            results("How are you?", true, true),
            metadata(),
            close(CloseCode::Normal),
        ] {
            ws.send(frame).await.unwrap();
        }
        seen
    })
    .await;
    let mut engine = engine(config(&base));
    let seen = utterance(&mut *engine).await.unwrap();

    assert_eq!(
        seen,
        vec![
            (TranscriptKind::Partial, "hello".to_owned()),
            (TranscriptKind::Partial, "hello".to_owned()),
            (TranscriptKind::Partial, "Hello world.".to_owned()),
            (TranscriptKind::Partial, "Hello world. how are".to_owned()),
            (
                TranscriptKind::Partial,
                "Hello world. How are you?".to_owned()
            ),
            (
                TranscriptKind::Final,
                "Hello world. How are you?".to_owned()
            ),
        ]
    );

    let observed = server.await.unwrap();
    assert_eq!(
        observed.auth.as_deref(),
        Some(format!("Token {KEY}").as_str())
    );
    assert!(
        !observed.uri.contains(KEY),
        "the key must not be in the URL"
    );
    assert!(observed.uri.starts_with("/v1/listen?"), "{}", observed.uri);
    for pair in [
        "model=nova-test",
        "language=en-US",
        "encoding=linear16",
        "sample_rate=16000",
        "channels=1",
        "interim_results=true",
    ] {
        assert!(
            observed.uri.contains(pair),
            "{pair} missing in {}",
            observed.uri
        );
    }
    // Every frame is raw 16-bit little-endian PCM: 0.25 is 8191, `0x1FFF`.
    assert_eq!(observed.binary.len(), FRAMES);
    assert!(observed.binary.iter().all(|f| f.len() == FRAME_BYTES));
    assert_eq!(observed.binary[0][..2], [0xFF, 0x1F]);
    assert_eq!(observed.texts, vec![json!({"type": "CloseStream"})]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_close_after_close_stream_without_metadata_still_ends_the_utterance() {
    let (base, _server) = serve(|mut ws, mut seen| async move {
        until_close_stream(&mut ws, &mut seen, |_| Vec::new()).await;
        // Only an interim result was ever sent: the flush must not lose it.
        ws.send(results("open the door", false, false))
            .await
            .unwrap();
        ws.send(close(CloseCode::Normal)).await.unwrap();
        seen
    })
    .await;
    let mut engine = engine(config(&base));
    let seen = utterance(&mut *engine).await.unwrap();
    assert_eq!(
        seen.last(),
        Some(&(TranscriptKind::Final, "open the door".to_owned()))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn keep_alive_goes_out_while_audio_is_quiet_and_stops_after_close_stream() {
    let (base, server) = serve(|mut ws, mut seen| async move {
        until_close_stream(&mut ws, &mut seen, |_| Vec::new()).await;
        for frame in [metadata(), close(CloseCode::Normal)] {
            ws.send(frame).await.unwrap();
        }
        seen
    })
    .await;
    let mut config = config(&base);
    config.keepalive_interval = Duration::from_millis(100);
    let mut engine = engine(config);
    let turn = TurnId::new();
    engine.begin(turn, Some("en")).unwrap();
    push_frames(&mut *engine, 1).unwrap();
    tokio::time::sleep(Duration::from_millis(450)).await;
    push_frames(&mut *engine, 1).unwrap();
    engine.finish().unwrap();
    collect(&mut *engine, turn).await.unwrap();

    let texts = server.await.unwrap().texts;
    let keep_alives = texts
        .iter()
        .filter(|t| **t == json!({"type": "KeepAlive"}))
        .count();
    assert!(keep_alives >= 2, "{texts:?}");
    assert_eq!(texts.last(), Some(&json!({"type": "CloseStream"})));
}

#[tokio::test(flavor = "multi_thread")]
async fn handshake_failures_map_to_fallback_categories_without_echoing_the_body() {
    for (status, falls_back_as_unavailable) in [
        (401, true),
        (403, true),
        (402, true),
        (429, true),
        (500, false),
    ] {
        let (base, _server) = serve_with(Some(status), |_, seen| async move { seen }).await;
        let mut engine = engine(config(&base));
        let error = utterance(&mut *engine).await.unwrap_err();
        let shown = error.to_string();
        assert_eq!(
            matches!(error, SpeechError::Unavailable(_)),
            falls_back_as_unavailable,
            "HTTP {status}: {shown}"
        );
        assert!(matches!(
            error,
            SpeechError::Unavailable(_) | SpeechError::Failed(_)
        ));
        assert!(
            shown.contains(&status.to_string()) || status == 402,
            "{shown}"
        );
        assert!(
            !shown.contains(KEY) && !shown.contains("bad key"),
            "{shown}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unreachable_server_is_unavailable() {
    // Port 0 can never be listened on, so the connect fails at once on every platform. A port
    // freed by dropping a listener could be taken by a parallel test before the connect.
    let mut engine = engine(config("ws://127.0.0.1:0"));
    let error = utterance(&mut *engine).await.unwrap_err();
    assert!(matches!(error, SpeechError::Unavailable(_)), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn server_faults_fail_the_utterance_and_never_echo_server_text() {
    let faults = [
        text(&json!({"type": "Error", "variant": "X", "description": SECRET_TEXT, "code": "C"})),
        close(CloseCode::Error),
        close(CloseCode::Policy),
        Message::Text(Utf8Bytes::from(SECRET_TEXT)),
        Message::Binary(SECRET_TEXT.as_bytes().to_vec().into()),
    ];
    for (index, fault) in faults.into_iter().enumerate() {
        let (base, _server) = after_first_audio(vec![fault]).await;
        let mut engine = engine(config(&base));
        let error = utterance(&mut *engine)
            .await
            .expect_err(&format!("fault {index}"));
        assert!(matches!(error, SpeechError::Failed(_)), "{error}");
        assert!(!error.to_string().contains("secret"), "{error}");
        // The utterance is over: the engine does not accept more audio.
        assert!(matches!(
            push_frames(&mut *engine, 1),
            Err(SpeechError::OutOfOrder(_))
        ));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_close_code_is_reported_as_a_number_only() {
    let (base, _server) = after_first_audio(vec![close(CloseCode::Error)]).await;
    let mut engine = engine(config(&base));
    let error = utterance(&mut *engine).await.unwrap_err();
    assert!(error.to_string().contains("1011"), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_transcript_is_capped_at_the_pipeline_limit() {
    let piece = "a".repeat(15_000);
    let frames: Vec<_> = (0..3).map(|_| results(&piece, true, false)).collect();
    let (base, _server) = serve(move |mut ws, mut seen| async move {
        until_close_stream(&mut ws, &mut seen, |_| Vec::new()).await;
        for frame in frames {
            ws.send(frame).await.unwrap();
        }
        ws.send(metadata()).await.unwrap();
        seen
    })
    .await;
    let mut engine = engine(config(&base));
    let seen = utterance(&mut *engine).await.unwrap();
    let (kind, text) = seen.last().unwrap();
    assert_eq!(*kind, TranscriptKind::Final);
    assert!(text.len() <= MAX_TRANSCRIPT_BYTES && text.len() > 15_000);
    assert!(
        seen.iter()
            .all(|(_, text)| text.len() <= MAX_TRANSCRIPT_BYTES)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_huge_message_fails_the_utterance() {
    let huge = Message::Text(Utf8Bytes::from("x".repeat(400_000)));
    let (base, _server) = after_first_audio(vec![huge]).await;
    let mut engine = engine(config(&base));
    let error = utterance(&mut *engine).await.unwrap_err();
    assert!(matches!(error, SpeechError::Failed(_)), "{error}");
    assert!(error.to_string().len() < 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn audio_past_the_utterance_limit_is_dropped_and_counted() {
    let stalled = Stalled::start().await;
    let mut config = config(&stalled.base);
    config.max_utterance = Duration::from_secs(1);
    let mut engine = engine(config);
    engine.begin(TurnId::new(), None).unwrap();
    // 1 s is 50 frames of 320; the 51st has no room.
    push_frames(&mut *engine, 50).unwrap();
    let error = push_frames(&mut *engine, 1).unwrap_err();
    assert_eq!(error, SpeechError::Overflow { dropped: FRAME });
}

#[tokio::test(flavor = "multi_thread")]
async fn a_full_audio_queue_overflows_instead_of_blocking() {
    // The handshake never completes, so nothing drains the queue.
    let stalled = Stalled::start().await;
    let mut config = config(&stalled.base);
    config.max_utterance = Duration::from_secs(60);
    let mut engine = engine(config);
    engine.begin(TurnId::new(), None).unwrap();
    let started = Instant::now();
    let samples = [0.1_f32; FRAME];
    let mut accepted = 0;
    let mut dropped = 0;
    for index in 0..200 {
        let frame = AudioFrame::new(&samples, AudioFormat::PIPELINE, (index * FRAME) as u64);
        match engine.push(frame.unwrap()) {
            Ok(()) => accepted += 1,
            Err(SpeechError::Overflow { dropped: n }) => dropped += n,
            Err(other) => panic!("{other}"),
        }
    }
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(accepted > 0 && accepted < 200, "{accepted}");
    assert_eq!(dropped, (200 - accepted) * FRAME);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_returns_at_once_and_drops_the_connection() {
    let mut stalled = Stalled::start().await;
    let mut engine = engine(config(&stalled.base));
    engine.begin(TurnId::new(), None).unwrap();
    push_frames(&mut *engine, 3).unwrap();
    stalled.opened().await;
    let started = Instant::now();
    engine.cancel();
    assert!(started.elapsed() < Duration::from_millis(50));
    stalled.closed().await;
    let mut out = Transcript::with_capacity(TurnId::new(), 8);
    assert_eq!(engine.poll(&mut out).unwrap(), SttPoll::Pending);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_begin_replaces_an_utterance_in_flight() {
    let mut stalled = Stalled::start().await;
    let mut engine = engine(config(&stalled.base));
    engine.begin(TurnId::new(), None).unwrap();
    push_frames(&mut *engine, 3).unwrap();
    stalled.opened().await;
    engine.begin(TurnId::new(), None).unwrap();
    // The first socket closes; the second stays open for the new utterance.
    stalled.closed().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_goes_silent_after_finish_times_out() {
    let (base, _server) = serve(|mut ws, mut seen| async move {
        until_close_stream(&mut ws, &mut seen, |_| Vec::new()).await;
        tokio::time::sleep(Duration::from_secs(5)).await;
        seen
    })
    .await;
    let mut config = config(&base);
    config.finish_timeout = Duration::from_millis(300);
    let mut engine = engine(config);
    let started = Instant::now();
    let error = utterance(&mut *engine).await.unwrap_err();
    assert!(matches!(error, SpeechError::Failed(_)), "{error}");
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[tokio::test(flavor = "multi_thread")]
async fn nothing_said_is_one_empty_final_and_sends_no_audio() {
    let (base, server) = serve(|mut ws, mut seen| async move {
        while next(&mut ws, &mut seen).await.is_some() {}
        seen
    })
    .await;
    let mut engine = engine(config(&base));
    let turn = TurnId::new();
    engine.begin(turn, Some("en-US")).unwrap();
    engine.finish().unwrap();
    let seen = collect(&mut *engine, turn).await.unwrap();
    assert_eq!(seen, vec![(TranscriptKind::Final, String::new())]);
    drop(engine);
    // The socket may never open, because the utterance is over before the
    // connection is made; if it did, an empty binary frame would have made
    // Deepgram close the stream, so nothing may have been sent on it.
    if let Ok(observed) = tokio::time::timeout(Duration::from_millis(300), server).await {
        let observed = observed.unwrap();
        assert!(observed.binary.is_empty() && observed.texts.is_empty());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn calls_out_of_order_and_bad_input_are_errors() {
    let stalled = Stalled::start().await;
    let mut engine = engine(config(&stalled.base));
    assert!(matches!(
        push_frames(&mut *engine, 1),
        Err(SpeechError::OutOfOrder(_))
    ));
    assert!(matches!(engine.finish(), Err(SpeechError::OutOfOrder(_))));
    let tag = "x".repeat(100);
    for language in ["", "en US", "en&model=evil", tag.as_str()] {
        let error = engine.begin(TurnId::new(), Some(language)).unwrap_err();
        assert!(matches!(error, SpeechError::Unsupported(_)), "{language:?}");
    }
    engine.begin(TurnId::new(), None).unwrap();
    let samples = [0.0_f32; FRAME];
    let wrong = AudioFormat::new(48_000, 2).unwrap();
    let frame = AudioFrame::new(&samples, wrong, 0).unwrap();
    assert!(matches!(
        engine.push(frame),
        Err(SpeechError::Invalid { .. })
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_endpoint_is_validated_before_any_connection() {
    let build = |url: &str, key: bool| {
        let mut config = DeepgramSttConfig::new("T", url, MODEL);
        config.api_key = key.then(|| ApiKey::new(KEY));
        DeepgramSttFactory::new(config, Handle::current()).map(|_| ())
    };
    assert!(build(DEEPGRAM_BASE_URL, true).is_ok());
    assert!(build("ws://127.0.0.1:8080/v1", true).is_ok());
    assert!(build("ws://localhost/v1", true).is_ok());
    assert!(build("ws://[::1]:8080/v1", true).is_ok());
    // No key, so nothing readable on the path.
    assert!(build("ws://dg.example.com/v1", false).is_ok());
    for url in [
        "ws://dg.example.com/v1",
        "ws://10.0.0.5/v1",
        "https://api.deepgram.com/v1",
        "http://127.0.0.1/v1",
        "ftp://api.deepgram.com/v1",
        "not a url",
    ] {
        assert!(
            matches!(build(url, true), Err(SpeechError::Invalid { .. })),
            "{url} was accepted"
        );
    }
    let mut config = DeepgramSttConfig::new("T", DEEPGRAM_BASE_URL, MODEL);
    config.max_utterance = Duration::ZERO;
    assert!(DeepgramSttFactory::new(config.clone(), Handle::current()).is_err());
    config.max_utterance = Duration::from_secs(601);
    assert!(DeepgramSttFactory::new(config, Handle::current()).is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_key_stays_out_of_debug_output() {
    let mut config = DeepgramSttConfig::new("T", DEEPGRAM_BASE_URL, MODEL);
    config.api_key = Some(ApiKey::new(KEY));
    let factory = DeepgramSttFactory::new(config.clone(), Handle::current()).unwrap();
    assert!(!format!("{config:?}").contains(KEY));
    assert!(!format!("{factory:?}").contains(KEY));
}
