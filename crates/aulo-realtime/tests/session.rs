//! The client against a local WebSocket server that replays a recorded event
//! sequence (the fixtures, shaped like the examples in OpenAI's references)
//! and records what the client sends: the session flow of both protocols, error
//! mapping, the caps, and that the key never shows up where it should not.

// Helpers outside `#[test]` fns are not covered by clippy's allow-in-tests.
// `result_large_err`: the handshake callback's `ErrorResponse` is fixed by tokio-tungstenite.
#![allow(clippy::unwrap_used, clippy::panic, clippy::result_large_err)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use aulo_realtime::{
    ApiKey, Dialect, RealtimeConfig, RealtimeError, ServerEvent, Session, Tool, ToolCall,
};
use aulo_types::{AuloEvent, NoticeLevel, TranscriptKind, VoiceState};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::{Bytes, Message};
use tokio_tungstenite::{WebSocketStream, accept_hdr_async};

const KEY: &str = "sk-test-secret-key";
const REALTIME: &str = include_str!("fixtures/realtime-session.jsonl");
const LIVE: &str = include_str!("fixtures/live-session.jsonl");

type Ws = WebSocketStream<TcpStream>;

struct Observed {
    uri: String,
    auth: Option<String>,
    messages: Vec<Value>,
}

fn config(dialect: Dialect, base: &str) -> RealtimeConfig {
    let model = match dialect {
        Dialect::Realtime => "gpt-realtime-2.1",
        Dialect::Live => "gpt-live-1",
    };
    let mut config = RealtimeConfig::new(dialect, model, ApiKey::new(KEY));
    config.base_url = format!("{base}/v1");
    config.timeout = Duration::from_secs(5);
    config.voice = Some("marin".into());
    config
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
            let auth = request
                .headers()
                .get("authorization")
                .map(|v| v.to_str().unwrap().to_owned());
            *record.lock().unwrap() = (request.uri().to_string(), auth);
            if let Some(status) = reject {
                let mut error = ErrorResponse::new(Some(format!("bad key {KEY}")));
                *error.status_mut() = StatusCode::from_u16(status).unwrap();
                return Err(error);
            }
            Ok(response)
        })
        .await;
        let (uri, auth) = seen.lock().unwrap().clone();
        let messages = match ws {
            Ok(ws) => script(ws).await,
            Err(_) => Vec::new(),
        };
        Observed {
            uri,
            auth,
            messages,
        }
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

async fn drain(ws: &mut Ws) -> Vec<Value> {
    let mut messages = Vec::new();
    while let Some(message) = next_json(ws).await {
        messages.push(message);
    }
    // Sends the reply to the client's close, which the client waits for.
    let _ = ws.flush().await;
    messages
}

async fn play(ws: &mut Ws, lines: &[&str]) {
    for line in lines {
        ws.send(Message::text(*line)).await.unwrap();
    }
}

/// The server greets and the client answers with its configuration, returned.
async fn open_realtime(ws: &mut Ws, greeting: &str) -> Vec<Value> {
    play(ws, &[greeting]).await;
    vec![next_json(ws).await.unwrap()]
}

/// Everything up to the end of the session.
async fn events_of(session: &mut Session) -> Vec<ServerEvent> {
    let mut events = Vec::new();
    while let Some(Ok(event)) = session.recv().await {
        events.push(event);
    }
    events
}

async fn next(session: &mut Session) -> ServerEvent {
    session.recv().await.unwrap().unwrap()
}

async fn next_error(session: &mut Session) -> RealtimeError {
    loop {
        if let Err(error) = session.recv().await.unwrap() {
            return error;
        }
    }
}

fn voice(state: VoiceState) -> ServerEvent {
    ServerEvent::Aulo(AuloEvent::VoiceState { state })
}

#[tokio::test(flavor = "multi_thread")]
async fn realtime_recorded_session() {
    let (base, server) = serve(|mut ws| async move {
        let lines: Vec<&str> = REALTIME.lines().collect();
        let mut seen = open_realtime(&mut ws, lines[0]).await;
        seen.push(next_json(&mut ws).await.unwrap());
        play(&mut ws, &lines[1..]).await;
        seen.extend(drain(&mut ws).await);
        seen
    })
    .await;
    let mut config = config(Dialect::Realtime, &base);
    config.instructions = Some("Be brief.".into());
    config.transcription_model = Some("gpt-transcribe".into());
    config.tools = vec![Tool {
        name: "consult_agent".into(),
        description: "Ask the main agent".into(),
        parameters: json!({"type": "object", "properties": {"request": {"type": "string"}}}),
    }];
    let mut session = Session::connect(&config).await.unwrap();
    assert_eq!(next(&mut session).await, ServerEvent::Ready);
    assert!(matches!(
        session.send_audio(&[1]).await,
        Err(RealtimeError::Invalid { .. })
    ));
    session.send_audio(&[1, 0, 2, 0]).await.unwrap();

    let mut events = Vec::new();
    while events.last() != Some(&voice(VoiceState::Listening)) || events.len() < 5 {
        events.push(next(&mut session).await);
    }
    let ServerEvent::Aulo(AuloEvent::Transcript { turn_id, .. }) = events[5].clone() else {
        panic!("expected a transcript, got {:?}", events[5]);
    };
    let transcript = |kind, text: &str, language: Option<&str>| {
        ServerEvent::Aulo(AuloEvent::Transcript {
            turn_id,
            kind,
            text: text.into(),
            language: language.map(str::to_owned),
        })
    };
    assert_eq!(
        events,
        vec![
            ServerEvent::Aulo(AuloEvent::SpeechStarted { at_ms: 1000 }),
            voice(VoiceState::Listening),
            ServerEvent::Aulo(AuloEvent::SpeechEnded { at_ms: 2000 }),
            voice(VoiceState::Thinking),
            transcript(TranscriptKind::Partial, "Hey", None),
            transcript(TranscriptKind::Partial, "Hey, can you hear me?", None),
            transcript(TranscriptKind::Final, "Hey, can you hear me?", Some("en")),
            voice(VoiceState::Speaking),
            ServerEvent::Audio(vec![1, 0, 2, 0]),
            ServerEvent::Audio(vec![3, 0]),
            ServerEvent::AssistantText {
                text: "Loud and ".into(),
                last: false
            },
            ServerEvent::AssistantText {
                text: "Loud and clear!".into(),
                last: true
            },
            ServerEvent::ToolCall(ToolCall {
                call_id: "call_001".into(),
                name: "consult_agent".into(),
                arguments: r#"{"request":"open the mail"}"#.into(),
            }),
            voice(VoiceState::Listening),
        ]
    );

    session
        .tool_output("call_001", r#"{"ok":true}"#)
        .await
        .unwrap();
    session.cancel_response().await.unwrap();
    session.close().await;
    assert!(events_of(&mut session).await.is_empty());

    let seen = server.await.unwrap();
    assert_eq!(seen.uri, "/v1/realtime?model=gpt-realtime-2.1");
    assert_eq!(seen.auth.as_deref(), Some("Bearer sk-test-secret-key"));
    assert_eq!(
        seen.messages,
        vec![
            json!({"type": "session.update", "session": {
                "type": "realtime",
                "output_modalities": ["audio"],
                "instructions": "Be brief.",
                "audio": {
                    "input": {"turn_detection": {"type": "server_vad"},
                              "transcription": {"model": "gpt-transcribe"}},
                    "output": {"voice": "marin"},
                },
                "tools": [{"type": "function", "name": "consult_agent",
                           "description": "Ask the main agent",
                           "parameters": {"type": "object", "properties": {"request": {"type": "string"}}}}],
                "tool_choice": "auto",
            }}),
            json!({"type": "input_audio_buffer.append", "audio": "AQACAA=="}),
            json!({"type": "conversation.item.create", "item": {
                "type": "function_call_output", "call_id": "call_001", "output": "{\"ok\":true}"}}),
            json!({"type": "response.create"}),
            json!({"type": "response.cancel"}),
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn live_recorded_session() {
    let (base, server) = serve(|mut ws| async move {
        let lines: Vec<&str> = LIVE.lines().collect();
        let (closed, replay) = lines.split_last().unwrap();
        // GPT-Live reads its configuration from the first message.
        let mut seen = vec![next_json(&mut ws).await.unwrap()];
        play(&mut ws, &replay[..1]).await;
        seen.push(next_json(&mut ws).await.unwrap());
        play(&mut ws, &replay[1..]).await;
        let close = next_json(&mut ws).await.unwrap();
        seen.push(close);
        play(&mut ws, &[closed]).await;
        seen.extend(drain(&mut ws).await);
        seen
    })
    .await;
    let mut config = config(Dialect::Live, &base);
    config.instructions = Some("Be brief.".into());
    let mut session = Session::connect(&config).await.unwrap();
    assert_eq!(next(&mut session).await, ServerEvent::Ready);
    session.send_audio(&[1, 0, 2, 0]).await.unwrap();
    assert_eq!(
        session.cancel_response().await,
        Err(RealtimeError::Unsupported)
    );
    assert_eq!(
        session.tool_output("call", "{}").await,
        Err(RealtimeError::Unsupported)
    );

    let mut events = Vec::new();
    while events.last() != Some(&ServerEvent::Audio(vec![1, 0, 2, 0])) {
        events.push(next(&mut session).await);
    }
    let ServerEvent::Aulo(AuloEvent::Transcript { turn_id, .. }) = events[0].clone() else {
        panic!("expected a transcript, got {:?}", events[0]);
    };
    let transcript = |kind, text: &str| {
        ServerEvent::Aulo(AuloEvent::Transcript {
            turn_id,
            kind,
            text: text.into(),
            language: None,
        })
    };
    assert_eq!(
        events,
        vec![
            transcript(TranscriptKind::Partial, "What is"),
            transcript(TranscriptKind::Partial, "What is the time"),
            // The assistant starting to answer is what ends the user's turn.
            transcript(TranscriptKind::Final, "What is the time"),
            ServerEvent::AssistantText {
                text: "It is ".into(),
                last: false
            },
            ServerEvent::Audio(vec![1, 0, 2, 0]),
        ]
    );
    session.close().await;
    assert_eq!(
        events_of(&mut session).await,
        vec![ServerEvent::Closed { seconds: Some(12) }]
    );

    let seen = server.await.unwrap();
    assert_eq!(seen.uri, "/v1/live/sessions");
    assert_eq!(seen.auth.as_deref(), Some("Bearer sk-test-secret-key"));
    assert_eq!(
        seen.messages,
        vec![
            json!({"type": "session.start", "session": {
                "model": "gpt-live-1",
                "instructions": "Be brief.",
                "audio": {"format": {"type": "audio/pcm", "rate": 24000},
                          "output": {"voice": "marin"}},
            }}),
            json!({"type": "session.input_audio.append", "audio": "AQACAA=="}),
            json!({"type": "session.close"}),
        ]
    );
}

/// Connects to a server that, after greeting, plays `frames` and keeps the socket open.
async fn connect_and_play(frames: Vec<Message>) -> Session {
    let greeting = REALTIME.lines().next().unwrap();
    let (base, _server) = serve(move |mut ws| async move {
        play(&mut ws, &[greeting]).await;
        let _ = next_json(&mut ws).await;
        for frame in frames {
            ws.send(frame).await.unwrap();
        }
        drain(&mut ws).await
    })
    .await;
    let mut session = Session::connect(&config(Dialect::Realtime, &base))
        .await
        .unwrap();
    assert_eq!(next(&mut session).await, ServerEvent::Ready);
    session
}

fn text(value: &Value) -> Message {
    Message::text(value.to_string())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_message_over_the_cap_ends_the_session_without_buffering_it() {
    let huge = json!({"type": "response.output_audio.delta", "delta": "A".repeat(2 << 20)});
    let mut session = connect_and_play(vec![text(&huge)]).await;
    assert_eq!(
        next_error(&mut session).await,
        RealtimeError::Protocol("message too large")
    );
    assert!(session.recv().await.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn malformed_traffic_ends_the_session() {
    for (frame, reason) in [
        (
            Message::Binary(Bytes::from_static(b"\x00")),
            "unexpected binary message",
        ),
        (Message::text("{not json"), "unreadable message"),
        (text(&json!({"delta": "x"})), "message without a type"),
        (
            text(&json!({"type": "response.output_audio.delta", "delta": "***"})),
            "undecodable audio",
        ),
    ] {
        let mut session = connect_and_play(vec![frame]).await;
        assert_eq!(
            next_error(&mut session).await,
            RealtimeError::Protocol(reason)
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_events_are_ignored_and_errors_never_echo_the_server() {
    let error = json!({"type": "error", "error": {
        "type": "invalid_request_error", "code": "invalid_api_key",
        "message": format!("Incorrect API key provided: {KEY}")}});
    let hostile = json!({"type": "error", "error": {"code": "<script>", "message": KEY}});
    let mut session = connect_and_play(vec![
        text(&json!({"type": "something.new", "payload": [1, 2, 3]})),
        text(&error),
        text(&hostile),
    ])
    .await;
    let notice = |message: &str| {
        ServerEvent::Aulo(AuloEvent::Notice {
            level: NoticeLevel::Error,
            message: message.into(),
            source: Some("openai-realtime".into()),
        })
    };
    assert_eq!(
        next(&mut session).await,
        notice("the realtime server reported an error (invalid_api_key)")
    );
    assert_eq!(
        next(&mut session).await,
        notice("the realtime server reported an error")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_oversized_tool_call_is_refused_with_a_warning() {
    let call = json!({"type": "response.function_call_arguments.done", "call_id": "c",
        "name": "consult_agent", "arguments": "x".repeat(70_000)});
    let mut session = connect_and_play(vec![text(&call)]).await;
    assert_eq!(
        next(&mut session).await,
        ServerEvent::Aulo(AuloEvent::Notice {
            level: NoticeLevel::Warn,
            message: "the realtime server sent a malformed tool call".into(),
            source: Some("openai-realtime".into()),
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_session_start_is_an_error() {
    let (base, _server) = serve(|mut ws| async move {
        let error = json!({"type": "error", "error": {"code": "invalid_model", "message": KEY}});
        play(&mut ws, &[&error.to_string()]).await;
        drain(&mut ws).await
    })
    .await;
    let error = Session::connect(&config(Dialect::Realtime, &base))
        .await
        .unwrap_err();
    assert_eq!(error, RealtimeError::Protocol("the session was refused"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rejected_key_is_reported_by_status_only() {
    for (status, expected) in [
        (401, RealtimeError::Rejected(401)),
        (429, RealtimeError::RateLimited),
        (500, RealtimeError::Refused(500)),
    ] {
        let (base, _server) = serve_with(Some(status), |_| async { Vec::new() }).await;
        let error = Session::connect(&config(Dialect::Realtime, &base))
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error} {error:?}").contains(KEY));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_silent_server_times_out() {
    let (base, _server) = serve(|mut ws| async move { drain(&mut ws).await }).await;
    let mut config = config(Dialect::Realtime, &base);
    config.timeout = Duration::from_millis(200);
    assert_eq!(
        Session::connect(&config).await.unwrap_err(),
        RealtimeError::TimedOut
    );
}

#[tokio::test]
async fn config_is_checked_before_anything_is_sent() {
    let mut plain = config(Dialect::Realtime, "ws://example.com");
    plain.base_url = "ws://example.com/v1".into();
    let mut live_tools = config(Dialect::Live, "ws://127.0.0.1:1");
    live_tools.tools = vec![Tool {
        name: "t".into(),
        description: String::new(),
        parameters: json!({}),
    }];
    let mut no_key = config(Dialect::Realtime, "ws://127.0.0.1:1");
    no_key.api_key = ApiKey::new("");
    let mut bad_key = config(Dialect::Realtime, "ws://127.0.0.1:1");
    bad_key.api_key = ApiKey::new("line\nbreak");
    for config in [plain, live_tools, no_key, bad_key] {
        let error = Session::connect(&config).await.unwrap_err();
        assert!(matches!(error, RealtimeError::Invalid { .. }), "{error:?}");
    }
}

#[test]
fn the_key_is_redacted_in_debug_output() {
    let config = config(Dialect::Realtime, "ws://127.0.0.1:1");
    assert!(!format!("{config:?}").contains(KEY));
}
