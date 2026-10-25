//! The turn driver: final transcripts in, spoken replies out.
//!
//! One task owns the agent session and the speakable-text normalizer. It
//! submits the transcript, streams the reply's sentences to the speaker, and
//! waits for the reply to drain before it takes the next submission, so
//! turns stay ordered. A submission that arrives while a turn runs waits in
//! the bounded channel instead of being dropped (T8.15 refines this into
//! steering messages for the running turn).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use aulo_types::{AuloEvent, NoticeLevel, VoiceState};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::pipeline::session::{SessionEvent, Submission, VoiceSession};
use crate::pipeline::speaker::SpeakerMsg;
use crate::pipeline::{Output, StateCell};
use crate::{Language, SpeakOptions, SpeakableStream};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run(
    session: Arc<dyn VoiceSession>,
    language: Language,
    mut submissions: mpsc::Receiver<Submission>,
    speaker: mpsc::Sender<SpeakerMsg>,
    mut reply_done: mpsc::Receiver<()>,
    outputs: mpsc::Sender<Output>,
    state: Arc<StateCell>,
    listening_open: Arc<AtomicBool>,
    cancel: CancellationToken,
) {
    loop {
        let submission = tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            submission = submissions.recv() => match submission {
                Some(submission) => submission,
                None => break,
            },
        };
        let turn_id = submission.turn_id;
        state.set(VoiceState::Thinking, &outputs);
        let mut events = match session.submit(submission) {
            Ok(events) => events,
            Err(error) => {
                let _ = outputs.try_send(Output::Event(AuloEvent::Notice {
                    level: NoticeLevel::Warn,
                    message: format!("the agent could not take the turn: {error}"),
                    source: None,
                }));
                state.end(listening_open.load(Ordering::Relaxed), &outputs);
                continue;
            }
        };
        let mut stream = SpeakableStream::new(SpeakOptions::new(language));
        let mut cancelled = false;
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Text { delta } => {
                    // The normalizer caps the reply: model text is untrusted
                    // and must not turn into minutes of speech.
                    for sentence in stream.push(&delta) {
                        if speaker
                            .send(SpeakerMsg::Speak { turn_id, sentence })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                SessionEvent::ToolStarted { .. } => {
                    // Progress fillers (T8.8) hook in here.
                }
                SessionEvent::Notice { level, message } => {
                    let _ = outputs.try_send(Output::Event(AuloEvent::Notice {
                        level,
                        message,
                        source: None,
                    }));
                }
                SessionEvent::Finished => break,
                SessionEvent::Cancelled => {
                    cancelled = true;
                    break;
                }
            }
        }
        if cancelled {
            let _ = speaker.send(SpeakerMsg::Cancel { turn_id }).await;
        } else {
            for sentence in stream.finish() {
                if speaker
                    .send(SpeakerMsg::Speak { turn_id, sentence })
                    .await
                    .is_err()
                {
                    break;
                }
            }
            let _ = speaker.send(SpeakerMsg::EndReply { turn_id }).await;
        }
        // The reply has to drain before the next turn starts, or two replies
        // would overlap on one speaker.
        tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            _ = reply_done.recv() => {}
        }
    }
}
