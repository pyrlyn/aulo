//! End-to-end pipeline tests (T8.1): fake engines from the aulo-speech
//! testkit, scripted frames and a scripted session, no real audio device and
//! no network. The full turn is frames → VAD → STT → end of turn →
//! submission → streamed reply → normalizer → TTS → playback.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use aulo_speech::testkit::{
    FAKE_VOICE, FRAME_SAMPLES, FakeStt, FakeTts, FakeVad, SPOKEN, TTS_RATE_HZ, TTS_SAMPLES_PER_BYTE,
};
use aulo_speech::{AudioFormat, AudioFrame, EngineId, EngineSpec, SpeechError, SpeechRate};
use aulo_types::{AuloEvent, ChatId, TranscriptKind, VoiceState};
use aulo_voice::{
    AudioSink, FrameSource, ListenMode, Output, PipelineConfig, PipelineDeps, SessionError,
    SessionEvent, SttRegistry, Submission, TtsRegistry, VoiceSession, spawn_pipeline,
};
use tokio::sync::mpsc;

/// 20 ms of loud capture: the VAD calls it speech.
fn loud() -> Vec<f32> {
    vec![0.5; FRAME_SAMPLES]
}

/// 20 ms of silence.
fn quiet() -> Vec<f32> {
    vec![0.0; FRAME_SAMPLES]
}

/// Test ids are literals in this file; `builtin` keeps the helper total so
/// the helpers outside `#[test]` bodies need no unwrap.
fn engine_id(id: &str) -> EngineId {
    EngineId::builtin(id)
}

fn stt_registry(capacity_samples: usize) -> Arc<SttRegistry> {
    let mut registry = SttRegistry::new();
    let registered = registry.register(engine_id("fake-stt"), move |spec| {
        Ok(Box::new(FakeStt::with_capacity(
            spec.engine.clone(),
            capacity_samples,
        )))
    });
    assert!(registered.is_ok(), "the fake stt engine must register");
    Arc::new(registry)
}

fn tts_registry() -> Arc<TtsRegistry> {
    let mut registry = TtsRegistry::new();
    let registered = registry.register(engine_id("fake-tts"), |spec| {
        Ok(Box::new(FakeTts::new(spec.engine.clone())))
    });
    assert!(registered.is_ok(), "the fake tts engine must register");
    Arc::new(registry)
}

fn stt_candidates() -> Vec<EngineSpec> {
    vec![EngineSpec {
        engine: engine_id("fake-stt"),
        model: None,
        voice: None,
        rate: SpeechRate::NORMAL,
    }]
}

fn tts_candidates() -> Vec<EngineSpec> {
    vec![EngineSpec {
        engine: engine_id("fake-tts"),
        model: None,
        voice: Some(FAKE_VOICE.to_owned()),
        rate: SpeechRate::NORMAL,
    }]
}

/// What a recording sink saw.
#[derive(Debug, Default)]
struct SinkState {
    samples: usize,
    volume: f32,
    stopped: bool,
    finished: bool,
}

/// A sink that keeps everything written and accepts it all.
#[derive(Debug, Clone)]
struct RecordingSink {
    state: Arc<Mutex<SinkState>>,
    dropped: Arc<AtomicBool>,
}

impl RecordingSink {
    fn new() -> (Self, Arc<Mutex<SinkState>>, Arc<AtomicBool>) {
        let state = Arc::new(Mutex::new(SinkState {
            volume: 1.0,
            ..SinkState::default()
        }));
        let dropped = Arc::new(AtomicBool::new(false));
        (
            Self {
                state: Arc::clone(&state),
                dropped: Arc::clone(&dropped),
            },
            state,
            dropped,
        )
    }
}

impl AudioSink for RecordingSink {
    fn write(&mut self, frame: AudioFrame<'_>) -> usize {
        let mut state = lock(&self.state);
        state.samples += frame.len();
        frame.len()
    }

    fn finish(&mut self) {
        lock(&self.state).finished = true;
    }

    fn stop(&mut self) {
        lock(&self.state).stopped = true;
    }

    fn set_volume(&mut self, volume: f32) -> Result<(), SpeechError> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(SpeechError::invalid("volume", "must be within 0.0..=1.0"));
        }
        lock(&self.state).volume = volume;
        Ok(())
    }
}

impl Drop for RecordingSink {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Relaxed);
    }
}

/// Frames handed out in order. With a `gate`, nothing is handed out until
/// the gate opens, which is how a push-to-talk test arms the microphone.
#[derive(Debug)]
struct ScriptedSource {
    frames: Mutex<VecDeque<Vec<f32>>>,
    gate: Option<Arc<AtomicBool>>,
    current: Vec<f32>,
    position: u64,
    dropped: Arc<AtomicBool>,
}

impl ScriptedSource {
    fn new(frames: Vec<Vec<f32>>) -> (Self, Arc<AtomicBool>) {
        let dropped = Arc::new(AtomicBool::new(false));
        (
            Self {
                frames: Mutex::new(frames.into()),
                gate: None,
                current: Vec::new(),
                position: 0,
                dropped: Arc::clone(&dropped),
            },
            dropped,
        )
    }

    fn gated(frames: Vec<Vec<f32>>, gate: Arc<AtomicBool>) -> (Self, Arc<AtomicBool>) {
        let (mut source, dropped) = Self::new(frames);
        source.gate = Some(gate);
        (source, dropped)
    }
}

impl FrameSource for ScriptedSource {
    fn next_frame(&mut self) -> Option<AudioFrame<'_>> {
        if let Some(gate) = &self.gate
            && !gate.load(Ordering::Relaxed)
        {
            return None;
        }
        let frame = lock(&self.frames).pop_front()?;
        self.current = frame;
        let position = self.position;
        self.position += FRAME_SAMPLES as u64;
        AudioFrame::new(&self.current, AudioFormat::PIPELINE, position).ok()
    }
}

impl Drop for ScriptedSource {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Relaxed);
    }
}

/// A session that answers every turn with the same scripted reply, and keeps
/// the submissions so a test can assert what the agent was asked.
#[derive(Debug)]
struct ScriptedSession {
    reply: String,
    /// When set, every submit fails with this detail.
    failure: Option<String>,
    submissions: Arc<Mutex<Vec<Submission>>>,
}

impl ScriptedSession {
    fn new(reply: &str) -> Self {
        Self {
            reply: reply.to_owned(),
            failure: None,
            submissions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn failing(detail: &str) -> Self {
        Self {
            failure: Some(detail.to_owned()),
            ..Self::new("")
        }
    }

    /// A handle on the submissions this session was given.
    fn submissions(&self) -> Arc<Mutex<Vec<Submission>>> {
        Arc::clone(&self.submissions)
    }
}

impl VoiceSession for ScriptedSession {
    fn submit(&self, submission: Submission) -> Result<mpsc::Receiver<SessionEvent>, SessionError> {
        lock(&self.submissions).push(submission);
        if let Some(detail) = &self.failure {
            return Err(SessionError::unavailable(detail));
        }
        let (events, receiver) = mpsc::channel(8);
        let _ = events.try_send(SessionEvent::Text {
            delta: self.reply.clone(),
        });
        let _ = events.try_send(SessionEvent::Finished);
        Ok(receiver)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

const REPLY: &str = "Sure. Opening the browser now.";

/// The reply the fake TTS turns `REPLY` into: two sentences, 100 silent
/// samples per byte of text.
fn expected_samples() -> usize {
    let sure = "Sure.".len() * TTS_SAMPLES_PER_BYTE;
    let rest = "Opening the browser now.".len() * TTS_SAMPLES_PER_BYTE;
    sure + rest
}

struct Harness {
    handle: aulo_voice::VoiceHandle,
    submissions: Arc<Mutex<Vec<Submission>>>,
    sink: Arc<Mutex<SinkState>>,
    source_dropped: Arc<AtomicBool>,
    sink_dropped: Arc<AtomicBool>,
    collected: Vec<Output>,
}

impl Harness {
    /// Collects outputs until the turn is complete: the last TTS chunk has
    /// been reported and the pipeline has settled back into `settled`.
    async fn run_until_settled(&mut self, settled: VoiceState) {
        let finished = tokio::time::timeout(Duration::from_secs(10), async {
            while let Some(output) = self.handle.next_output().await {
                self.collected.push(output);
                if turn_complete(&self.collected, settled) {
                    break;
                }
            }
        })
        .await;
        assert!(
            finished.is_ok(),
            "the pipeline did not settle into {settled:?}; saw {} outputs",
            self.collected.len()
        );
    }

    fn states(&self) -> Vec<VoiceState> {
        self.collected
            .iter()
            .filter_map(|output| match output {
                Output::Event(AuloEvent::VoiceState { state }) => Some(*state),
                _ => None,
            })
            .collect()
    }

    fn transcripts(&self) -> Vec<(TranscriptKind, String)> {
        self.collected
            .iter()
            .filter_map(|output| match output {
                Output::Event(AuloEvent::Transcript { kind, text, .. }) => {
                    Some((*kind, text.clone()))
                }
                _ => None,
            })
            .collect()
    }

    fn audio_chunks(&self) -> Vec<u32> {
        self.collected
            .iter()
            .filter_map(|output| match output {
                Output::Audio(chunk) => Some(chunk.seq),
                _ => None,
            })
            .collect()
    }
}

fn turn_complete(outputs: &[Output], settled: VoiceState) -> bool {
    let Some(last) = outputs.iter().rposition(|output| {
        matches!(
            output,
            Output::Event(AuloEvent::TtsChunk(chunk)) if chunk.last
        )
    }) else {
        return false;
    };
    outputs[last..].iter().any(|output| {
        matches!(
            output,
            Output::Event(AuloEvent::VoiceState { state }) if *state == settled
        )
    })
}

/// Waits for `what` to hold, the way a client would poll the pipeline: the
/// tasks run concurrently, so a state change lands a tick after the command.
async fn await_until(what: &str, mut check: impl FnMut() -> bool) {
    let settled = tokio::time::timeout(Duration::from_secs(5), async {
        while !check() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(settled.is_ok(), "{what} never happened");
}

fn harness(config: PipelineConfig, session: ScriptedSession, frames: Vec<Vec<f32>>) -> Harness {
    let submissions = session.submissions();
    harness_with(config, session, submissions, frames, None)
}

fn harness_with(
    config: PipelineConfig,
    session: ScriptedSession,
    submissions: Arc<Mutex<Vec<Submission>>>,
    frames: Vec<Vec<f32>>,
    gate: Option<Arc<AtomicBool>>,
) -> Harness {
    let (source, source_dropped) = match gate {
        Some(gate) => ScriptedSource::gated(frames, gate),
        None => ScriptedSource::new(frames),
    };
    let (sink, sink_state, sink_dropped) = RecordingSink::new();
    let handle = spawn_pipeline(
        config,
        PipelineDeps {
            chat_id: ChatId::new(),
            session: Arc::new(session),
            stt_registry: stt_registry(FRAME_SAMPLES * 40),
            tts_registry: tts_registry(),
            stt: stt_candidates(),
            tts: tts_candidates(),
            vad: Box::new(FakeVad::new(engine_id("fake-vad"), 0.1, 3)),
            source: Box::new(source),
            sink: Box::new(sink),
            stt_language: None,
        },
    );
    Harness {
        handle,
        submissions,
        sink: sink_state,
        source_dropped,
        sink_dropped,
        collected: Vec::new(),
    }
}

#[tokio::test]
async fn fake_engines_drive_a_full_turn() {
    // 25 frames of speech (500 ms), then 45 of silence (900 ms): the silence
    // passes the 700 ms end-of-turn timeout well after the VAD's hangover.
    let mut frames = Vec::new();
    frames.extend(std::iter::repeat_with(loud).take(25));
    frames.extend(std::iter::repeat_with(quiet).take(45));
    let mut harness = harness(
        PipelineConfig {
            listen_mode: ListenMode::AlwaysListening,
            ..PipelineConfig::default()
        },
        ScriptedSession::new(REPLY),
        frames,
    );
    harness.run_until_settled(VoiceState::Listening).await;

    // One utterance, one submission, the full scripted transcript.
    {
        let submissions = lock(&harness.submissions);
        assert_eq!(submissions.len(), 1, "exactly one turn was submitted");
        assert_eq!(submissions[0].text, SPOKEN);
        assert_eq!(submissions[0].language.as_deref(), Some("en"));
    }

    // The state machine walked the whole cycle.
    assert_eq!(
        harness.states(),
        [
            VoiceState::Listening,
            VoiceState::Thinking,
            VoiceState::Speaking,
            VoiceState::Listening,
        ]
    );

    // Speech boundaries and a growing partial, then the final transcript.
    let started = harness
        .collected
        .iter()
        .filter(|output| matches!(output, Output::Event(AuloEvent::SpeechStarted { .. })))
        .count();
    assert_eq!(started, 1, "one speech segment");
    let transcripts = harness.transcripts();
    assert!(
        transcripts
            .iter()
            .any(|(kind, text)| *kind == TranscriptKind::Partial && text == "op")
    );
    assert!(
        transcripts
            .iter()
            .any(|(kind, text)| *kind == TranscriptKind::Final && text == SPOKEN)
    );
    assert!(
        transcripts
            .iter()
            .any(|(kind, text)| *kind == TranscriptKind::Partial && text == SPOKEN),
        "the last partial already holds the whole sentence"
    );

    // The reply was synthesized in sentence order and played out whole.
    let chunks = harness.audio_chunks();
    assert_eq!(chunks, [0, 1], "one chunk per sentence");
    let last = harness.collected.iter().rposition(|output| {
        matches!(
            output,
            Output::Event(AuloEvent::TtsChunk(chunk)) if chunk.last
        )
    });
    assert!(last.is_some(), "the reply ends with a last chunk");
    assert_eq!(lock(&harness.sink).samples, expected_samples());
    let first_chunk = harness
        .collected
        .iter()
        .find_map(|output| match output {
            Output::Event(AuloEvent::TtsChunk(chunk)) if chunk.seq == 0 => Some(*chunk),
            _ => None,
        })
        .expect("first chunk");
    assert_eq!(first_chunk.sample_rate_hz, TTS_RATE_HZ);
    assert_eq!(first_chunk.channels, 1);
    assert_eq!(
        first_chunk.duration_ms, 20,
        "the first sentence is 500 samples"
    );
    assert!(!first_chunk.last);

    // The reply was not stopped, and the audio streams close on shutdown.
    assert!(!lock(&harness.sink).stopped);
    harness.handle.shutdown().await;
    assert!(harness.source_dropped.load(Ordering::Relaxed));
    assert!(harness.sink_dropped.load(Ordering::Relaxed));
}

#[tokio::test]
async fn push_to_talk_start_and_stop_produce_one_utterance() {
    // The gate keeps the frames until the microphone is armed. The frames
    // hold speech and no silence at all, so only the stop can end the turn.
    let gate = Arc::new(AtomicBool::new(false));
    let frames: Vec<Vec<f32>> = (0..25).map(|_| loud()).collect();
    let session = ScriptedSession::new(REPLY);
    let submissions = session.submissions();
    let mut harness = harness_with(
        PipelineConfig::default(),
        session,
        submissions,
        frames,
        Some(Arc::clone(&gate)),
    );
    // Nothing is heard before the start: the state stays Idle.
    assert_eq!(harness.handle.state(), VoiceState::Idle);
    assert!(!harness.handle.is_listening());

    harness
        .handle
        .start_listening()
        .await
        .expect("start listening");
    await_until("the microphone to be live", || {
        harness.handle.is_listening()
    })
    .await;
    gate.store(true, Ordering::Relaxed);

    // Wait for partials, which proves the frames are flowing and the engine
    // is transcribing them.
    let hearing = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(output) = harness.handle.next_output().await {
            harness.collected.push(output);
            if harness.transcripts().len() >= 3 {
                break;
            }
        }
    })
    .await;
    assert!(hearing.is_ok(), "no partials arrived");
    // No submission yet: the utterance is still open.
    assert!(lock(&harness.submissions).is_empty());

    harness.handle.stop_listening().await.expect("stop");
    harness.run_until_settled(VoiceState::Idle).await;

    {
        let submissions = lock(&harness.submissions);
        assert_eq!(submissions.len(), 1, "start and stop produce one utterance");
        assert_eq!(submissions[0].text, SPOKEN);
    }
    assert_eq!(
        harness.states(),
        [
            VoiceState::Idle,
            VoiceState::Listening,
            VoiceState::Thinking,
            VoiceState::Speaking,
            VoiceState::Idle,
        ]
    );
    assert!(!harness.handle.is_listening());
    assert_eq!(lock(&harness.sink).samples, expected_samples());
    harness.handle.shutdown().await;
}

#[tokio::test]
async fn a_long_transcript_is_capped_before_the_agent_sees_it() {
    let mut frames = Vec::new();
    frames.extend(std::iter::repeat_with(loud).take(25));
    frames.extend(std::iter::repeat_with(quiet).take(45));
    let mut harness = harness(
        PipelineConfig {
            listen_mode: ListenMode::AlwaysListening,
            max_transcript_chars: 10,
            ..PipelineConfig::default()
        },
        ScriptedSession::new(REPLY),
        frames,
    );
    harness.run_until_settled(VoiceState::Listening).await;

    {
        let submissions = lock(&harness.submissions);
        assert_eq!(submissions.len(), 1);
        assert_eq!(submissions[0].text, "open the b");
    }
    // The client sees the same capped text, not the engine's full output.
    let final_transcript = harness
        .transcripts()
        .into_iter()
        .find(|(kind, _)| *kind == TranscriptKind::Final)
        .expect("final transcript");
    assert_eq!(final_transcript.1, "open the b");
    harness.handle.shutdown().await;
}

#[tokio::test]
async fn a_session_that_refuses_the_turn_keeps_the_pipeline_alive() {
    let mut frames = Vec::new();
    frames.extend(std::iter::repeat_with(loud).take(25));
    frames.extend(std::iter::repeat_with(quiet).take(45));
    let mut harness = harness(
        PipelineConfig {
            listen_mode: ListenMode::AlwaysListening,
            ..PipelineConfig::default()
        },
        ScriptedSession::failing("chat is gone"),
        frames,
    );
    // The turn is refused; the pipeline notes it and goes back to listening.
    let settled = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(output) = harness.handle.next_output().await {
            harness.collected.push(output);
            let notice = harness.collected.iter().any(|output| {
                matches!(
                    output,
                    Output::Event(AuloEvent::Notice { message, .. })
                        if message.contains("the agent could not take the turn")
                )
            });
            if notice && harness.handle.state() == VoiceState::Listening {
                break;
            }
        }
    })
    .await;
    assert!(settled.is_ok(), "the refusal never reached the client");
    // The turn was attempted and refused: the pipeline noted it and went on.
    assert_eq!(lock(&harness.submissions).len(), 1);
    assert_eq!(harness.audio_chunks(), Vec::<u32>::new());
    harness.handle.shutdown().await;
}

#[tokio::test]
async fn the_volume_reaches_the_sink_and_bad_values_are_refused() {
    let gate = Arc::new(AtomicBool::new(false));
    let session = ScriptedSession::new(REPLY);
    let submissions = session.submissions();
    let harness = harness_with(
        PipelineConfig::default(),
        session,
        submissions,
        Vec::new(),
        Some(gate),
    );
    harness.handle.set_volume(0.5).await.expect("volume");
    await_until("the sink to take the volume", || {
        lock(&harness.sink).volume == 0.5
    })
    .await;
    for volume in [-0.1, 1.5, f32::NAN] {
        assert!(
            harness.handle.set_volume(volume).await.is_err(),
            "{volume} was accepted"
        );
    }
    assert_eq!(lock(&harness.sink).volume, 0.5);
    harness.handle.shutdown().await;
}
