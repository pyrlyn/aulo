//! The engine against a local HTTP server: both reply shapes, the request
//! the server sees, error mapping to fallback categories, and the caps.

// Helpers outside `#[test]` fns are not covered by clippy's allow-in-tests.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::time::{Duration, Instant};

use aulo_speech::{
    AudioFormat, AudioFrame, EngineId, EngineSpec, SpeechError, SpeechRate, SttEngine, SttPoll,
    Transcript, TranscriptKind, TurnId,
};
use aulo_speech_cloud::{
    ApiKey, CloudSttConfig, CloudSttFactory, MAX_TRANSCRIPT_BYTES, ResponseShape,
};
use serde_json::json;
use tokio::runtime::Handle;
use wiremock::matchers::{header, method, path};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

const KEY: &str = "sk-test-secret-key";
const MODEL: &str = "gpt-test-transcribe";
const FRAME: usize = 320;
const FRAMES_PER_SECOND: usize = 50;
const POLL_PAUSE: Duration = Duration::from_millis(10);
const POLL_LIMIT: usize = 500;
const SSE: &str = "text/event-stream";

type Seen = Vec<(TranscriptKind, String)>;

/// The multipart body holds binary audio, so wiremock's string matchers cannot
/// be used; this one searches the raw bytes.
struct BodyHas(&'static [&'static str]);

impl Match for BodyHas {
    fn matches(&self, request: &Request) -> bool {
        self.0.iter().all(|needle| {
            request
                .body
                .windows(needle.len())
                .any(|window| window == needle.as_bytes())
        })
    }
}

fn config(server: &MockServer, shape: ResponseShape) -> CloudSttConfig {
    let mut config = CloudSttConfig::new("Test", format!("{}/v1", server.uri()), MODEL);
    config.api_key = Some(ApiKey::new(KEY));
    config.shape = shape;
    config
}

fn spec(model: Option<&str>) -> EngineSpec {
    EngineSpec {
        engine: EngineId::new("openai-stt").unwrap(),
        model: model.map(str::to_owned),
        voice: None,
        rate: SpeechRate::NORMAL,
    }
}

fn engine(config: CloudSttConfig) -> Box<dyn SttEngine> {
    CloudSttFactory::new(config, Handle::current())
        .unwrap()
        .build(&spec(None))
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
    push_frames(engine, FRAMES_PER_SECOND)?;
    engine.finish()?;
    collect(engine, turn).await
}

async fn serve(response: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(response)
        .mount(&server)
        .await;
    server
}

fn sse(events: &[serde_json::Value]) -> ResponseTemplate {
    let body: String = events.iter().map(|e| format!("data: {e}\n\n")).collect();
    ResponseTemplate::new(200).set_body_raw(body, SSE)
}

fn only_error(seen: Result<Seen, SpeechError>) -> SpeechError {
    seen.unwrap_err()
}

#[tokio::test(flavor = "multi_thread")]
async fn json_shape_sends_the_documented_request_and_returns_one_final() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .and(header("authorization", format!("Bearer {KEY}").as_str()))
        .and(BodyHas(&[
            "RIFF",
            "WAVE",
            "name=\"file\"",
            MODEL,
            "name=\"response_format\"",
            "name=\"language\"\r\n\r\nen\r\n",
        ]))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"text": " open the browser "})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let mut engine = engine(config(&server, ResponseShape::Json));
    assert!(!engine.info().capabilities.streaming);
    assert!(
        engine.info().capabilities.offline,
        "loopback server works offline"
    );
    assert!(engine.info().capabilities.needs_network);

    let seen = utterance(engine.as_mut()).await.unwrap();

    assert_eq!(
        seen,
        [(TranscriptKind::Final, "open the browser".to_owned())]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn stream_shape_delivers_growing_partials_then_the_final() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(BodyHas(&["name=\"stream\"\r\n\r\ntrue\r\n"]))
        .respond_with(sse(&[
            json!({"type": "transcript.text.delta", "delta": "open"}),
            json!({"type": "transcript.text.delta", "delta": " the"}),
            json!({"type": "transcript.text.segment", "text": "ignored"}),
            json!({"type": "transcript.text.delta", "delta": " browser"}),
            json!({"type": "transcript.text.done", "text": "open the browser"}),
        ]))
        .expect(1)
        .mount(&server)
        .await;
    let mut engine = engine(config(&server, ResponseShape::Stream));
    assert!(engine.info().capabilities.streaming);

    let seen = utterance(engine.as_mut()).await.unwrap();

    let texts: Vec<_> = seen.iter().map(|(_, text)| text.as_str()).collect();
    assert_eq!(
        texts,
        ["open", "open the", "open the browser", "open the browser"]
    );
    assert!(
        seen[..3]
            .iter()
            .all(|(kind, _)| *kind == TranscriptKind::Partial)
    );
    assert_eq!(seen[3].0, TranscriptKind::Final);
}

#[tokio::test(flavor = "multi_thread")]
async fn stream_shape_accepts_a_plain_json_reply_like_whisper_1_sends() {
    let server = serve(ResponseTemplate::new(200).set_body_json(json!({"text": "hello"}))).await;
    let mut engine = engine(config(&server, ResponseShape::Stream));

    let seen = utterance(engine.as_mut()).await.unwrap();

    assert_eq!(seen, [(TranscriptKind::Final, "hello".to_owned())]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stream_that_stops_early_fails_instead_of_returning_half_a_sentence() {
    let server = serve(sse(&[
        json!({"type": "transcript.text.delta", "delta": "delete"}),
    ]))
    .await;
    let mut engine = engine(config(&server, ResponseShape::Stream));

    let error = only_error(utterance(engine.as_mut()).await);

    assert!(matches!(error, SpeechError::Failed(_)), "{error:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn spec_model_overrides_the_default_and_the_engine_takes_the_spec_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(BodyHas(&["whisper-1"]))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"text": "ok"})))
        .expect(1)
        .mount(&server)
        .await;
    let factory = CloudSttFactory::new(config(&server, ResponseShape::Json), Handle::current())
        .unwrap()
        .into_factory();

    let mut engine = factory(&spec(Some("whisper-1"))).unwrap();
    utterance(engine.as_mut()).await.unwrap();

    assert_eq!(engine.info().id.as_str(), "openai-stt");
    assert_eq!(engine.info().name, "Test");
}

#[tokio::test(flavor = "multi_thread")]
async fn rejected_key_falls_back_without_echoing_what_the_server_quoted() {
    let body = json!({"error": {"message": format!("Incorrect API key provided: {KEY}")}});
    let server = serve(ResponseTemplate::new(401).set_body_json(body)).await;
    let mut engine = engine(config(&server, ResponseShape::Json));

    let error = only_error(utterance(engine.as_mut()).await);

    assert!(matches!(error, SpeechError::Unavailable(_)), "{error:?}");
    assert!(error.should_fall_back());
    assert!(error.to_string().contains("API key"));
    assert!(!format!("{error:?}").contains(KEY));
}

#[tokio::test(flavor = "multi_thread")]
async fn http_failures_map_to_fallback_categories() {
    for (status, unavailable) in [(429, true), (500, false), (503, false), (404, false)] {
        let server = serve(ResponseTemplate::new(status)).await;
        let mut engine = engine(config(&server, ResponseShape::Json));

        let error = only_error(utterance(engine.as_mut()).await);

        assert!(error.should_fall_back(), "{status}: {error:?}");
        assert_eq!(
            matches!(error, SpeechError::Unavailable(_)),
            unavailable,
            "{status}: {error:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn server_hints_are_not_echoed() {
    let message = "unknown model\nINFO forged";
    let server =
        serve(ResponseTemplate::new(400).set_body_json(json!({"error": {"message": message}})))
            .await;
    let mut engine = engine(config(&server, ResponseShape::Json));

    let error = only_error(utterance(engine.as_mut()).await);

    assert!(matches!(error, SpeechError::Failed(_)));
    assert!(error.to_string().contains("HTTP 400"));
    assert!(!error.to_string().contains("forged"));
}

#[tokio::test(flavor = "multi_thread")]
async fn unreachable_server_is_unavailable() {
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", closed.local_addr().unwrap());
    drop(closed);
    let mut engine = engine(CloudSttConfig::new("Test", url, MODEL));

    let error = only_error(utterance(engine.as_mut()).await);

    assert!(matches!(error, SpeechError::Unavailable(_)), "{error:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_slow_server_times_out_as_a_failure() {
    let server = serve(
        ResponseTemplate::new(200)
            .set_body_json(json!({"text": "late"}))
            .set_delay(Duration::from_secs(5)),
    )
    .await;
    let mut config = config(&server, ResponseShape::Json);
    config.request_timeout = Duration::from_millis(200);
    let mut engine = engine(config);

    let error = only_error(utterance(engine.as_mut()).await);

    assert!(matches!(error, SpeechError::Failed(_)), "{error:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn malformed_replies_fail() {
    for response in [
        ResponseTemplate::new(200).set_body_string("<html>proxy login</html>"),
        ResponseTemplate::new(200).set_body_json(json!({"words": []})),
    ] {
        let server = serve(response).await;
        let mut engine = engine(config(&server, ResponseShape::Json));

        let error = only_error(utterance(engine.as_mut()).await);

        assert!(matches!(error, SpeechError::Failed(_)), "{error:?}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn long_replies_are_capped_on_a_char_boundary() {
    let long = "я".repeat(MAX_TRANSCRIPT_BYTES);
    let server = serve(ResponseTemplate::new(200).set_body_json(json!({"text": long}))).await;
    let mut engine = engine(config(&server, ResponseShape::Json));

    let seen = utterance(engine.as_mut()).await.unwrap();

    let text = &seen[0].1;
    assert!(text.len() <= MAX_TRANSCRIPT_BYTES && text.len() > MAX_TRANSCRIPT_BYTES - 2);
    assert!(text.chars().all(|c| c == 'я'));
}

#[tokio::test(flavor = "multi_thread")]
async fn oversized_bodies_and_streams_are_refused() {
    let huge = "a".repeat(MAX_TRANSCRIPT_BYTES * 8);
    let server = serve(ResponseTemplate::new(200).set_body_json(json!({"text": huge}))).await;
    let error = only_error(utterance(engine(config(&server, ResponseShape::Json)).as_mut()).await);
    assert!(matches!(error, SpeechError::Failed(_)), "{error:?}");

    let delta = json!({"type": "transcript.text.delta", "delta": "x".repeat(1_000)});
    let flood = vec![delta; 400];
    let server = serve(sse(&flood)).await;
    let error =
        only_error(utterance(engine(config(&server, ResponseShape::Stream)).as_mut()).await);
    assert!(matches!(error, SpeechError::Failed(_)), "{error:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn streamed_text_is_capped() {
    let delta = json!({"type": "transcript.text.delta", "delta": "y".repeat(5_000)});
    let done =
        json!({"type": "transcript.text.done", "text": "z".repeat(MAX_TRANSCRIPT_BYTES + 4_000)});
    let server = serve(sse(&[
        delta.clone(),
        delta.clone(),
        delta.clone(),
        delta,
        done,
    ]))
    .await;
    let mut engine = engine(config(&server, ResponseShape::Stream));

    let seen = utterance(engine.as_mut()).await.unwrap();

    assert!(
        seen.iter()
            .all(|(_, text)| text.len() <= MAX_TRANSCRIPT_BYTES)
    );
    assert_eq!(seen.last().unwrap().0, TranscriptKind::Final);
}

#[tokio::test(flavor = "multi_thread")]
async fn silence_finishes_with_an_empty_final_and_no_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let mut engine = engine(config(&server, ResponseShape::Json));
    let turn = TurnId::new();

    engine.begin(turn, None).unwrap();
    engine.finish().unwrap();
    let seen = collect(engine.as_mut(), turn).await.unwrap();

    assert_eq!(seen, [(TranscriptKind::Final, String::new())]);
}

#[tokio::test(flavor = "multi_thread")]
async fn push_is_bounded_by_the_longest_utterance_and_never_blocks() {
    let server = MockServer::start().await;
    let mut config = config(&server, ResponseShape::Json);
    config.max_utterance = Duration::from_secs(1);
    let mut engine = engine(config);
    engine.begin(TurnId::new(), None).unwrap();

    push_frames(engine.as_mut(), FRAMES_PER_SECOND).unwrap();
    let error = push_frames(engine.as_mut(), 1).unwrap_err();

    assert_eq!(error, SpeechError::Overflow { dropped: FRAME });
    assert!(!error.should_fall_back());
}

#[tokio::test(flavor = "multi_thread")]
async fn call_order_and_audio_format_are_checked() {
    let server = MockServer::start().await;
    let mut engine = engine(config(&server, ResponseShape::Json));
    assert!(matches!(
        push_frames(engine.as_mut(), 1),
        Err(SpeechError::OutOfOrder(_))
    ));
    assert!(matches!(engine.finish(), Err(SpeechError::OutOfOrder(_))));

    engine.begin(TurnId::new(), None).unwrap();
    let stereo = AudioFormat::new(48_000, 2).unwrap();
    let frame = AudioFrame::new(&[0.0; 4], stereo, 0).unwrap();
    assert!(matches!(
        engine.push(frame),
        Err(SpeechError::Invalid { .. })
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_is_instant_and_the_engine_is_reusable() {
    let server = serve(
        ResponseTemplate::new(200)
            .set_body_json(json!({"text": "late"}))
            .set_delay(Duration::from_secs(5)),
    )
    .await;
    let mut engine = engine(config(&server, ResponseShape::Json));
    engine.begin(TurnId::new(), None).unwrap();
    push_frames(engine.as_mut(), FRAMES_PER_SECOND).unwrap();
    engine.finish().unwrap();

    let started = Instant::now();
    engine.cancel();

    assert!(started.elapsed() < Duration::from_millis(50));
    let mut out = Transcript::with_capacity(TurnId::new(), 8);
    assert_eq!(engine.poll(&mut out).unwrap(), SttPoll::Pending);
    engine.begin(TurnId::new(), None).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn results_are_stamped_with_their_own_turn() {
    let server = serve(ResponseTemplate::new(200).set_body_json(json!({"text": "hi"}))).await;
    let mut engine = engine(config(&server, ResponseShape::Json));
    let turn = TurnId::new();
    engine.begin(turn, None).unwrap();
    push_frames(engine.as_mut(), 1).unwrap();
    engine.finish().unwrap();
    let mut out = Transcript::with_capacity(TurnId::new(), 8);
    for _ in 0..POLL_LIMIT {
        if engine.poll(&mut out).unwrap() == SttPoll::Updated {
            break;
        }
        tokio::time::sleep(POLL_PAUSE).await;
    }
    assert_eq!(out.turn_id, turn);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_key_never_shows_in_debug_output() {
    let server = MockServer::start().await;
    let config = config(&server, ResponseShape::Json);
    let factory = CloudSttFactory::new(config.clone(), Handle::current()).unwrap();
    let engine = factory.build(&spec(None)).unwrap();
    let dump = format!("{config:?} {factory:?} {:?}", engine.info());
    assert!(!dump.contains(KEY), "{dump}");
}

#[tokio::test(flavor = "multi_thread")]
async fn unsafe_or_invalid_configs_are_refused() {
    let handle = Handle::current();
    let mut plain_http = CloudSttConfig::new("Test", "http://stt.example.com/v1", MODEL);
    plain_http.api_key = Some(ApiKey::new(KEY));
    let mut no_audio = CloudSttConfig::new("Test", "https://stt.example.com/v1", MODEL);
    no_audio.max_utterance = Duration::ZERO;
    let mut too_long = no_audio.clone();
    too_long.max_utterance = Duration::from_secs(3_600);
    let configs = [
        plain_http,
        no_audio,
        too_long,
        CloudSttConfig::new("Test", "not a url", MODEL),
        CloudSttConfig::new("Test", "ftp://stt.example.com", MODEL),
    ];
    for config in configs {
        let error = CloudSttFactory::new(config, handle.clone()).unwrap_err();
        assert!(matches!(error, SpeechError::Invalid { .. }), "{error:?}");
    }
    // Plain http without a key is fine: no secret crosses the wire.
    assert!(
        CloudSttFactory::new(
            CloudSttConfig::new("Test", "http://stt.example.com/v1", MODEL),
            handle
        )
        .is_ok()
    );
}
