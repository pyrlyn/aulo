//! Client helpers for `SessionService.Converse`.
//!
//! The session stays open so a test can attach, send a turn, then interrupt
//! or switch model before the reply finishes. [`drive`] is the one-shot
//! form: send these submissions, then read until an event matches.

use std::time::Duration;

use aulo_proto::aulo::v1::converse_request::Submission;
use aulo_proto::aulo::v1::session_service_client::SessionServiceClient;
use aulo_proto::aulo::v1::{
    Attach, ConverseRequest, Event, Interrupt, ModelRef, SwitchModel, UserTurn,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;

use crate::error::TestkitError;

/// How long [`ConverseSession::next_event`] waits for one message.
const IDLE: Duration = Duration::from_secs(2);

/// One open `Converse` stream.
pub struct ConverseSession {
    outgoing: mpsc::Sender<ConverseRequest>,
    incoming: tonic::Streaming<aulo_proto::aulo::v1::ConverseResponse>,
}

impl std::fmt::Debug for ConverseSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConverseSession").finish_non_exhaustive()
    }
}

impl ConverseSession {
    /// Opens the bidi stream. The first submission a server accepts is
    /// [`attach`]; sending anything else closes the stream.
    pub async fn open(channel: Channel) -> Result<Self, TestkitError> {
        let (outgoing, rx) = mpsc::channel(16);
        let response = SessionServiceClient::new(channel)
            .converse(ReceiverStream::new(rx))
            .await
            .map_err(rpc)?;
        Ok(Self {
            outgoing,
            incoming: response.into_inner(),
        })
    }

    /// Sends one submission. Fails when the server has already hung up.
    pub async fn send(&self, request: ConverseRequest) -> Result<(), TestkitError> {
        self.outgoing
            .send(request)
            .await
            .map_err(|_| TestkitError::Closed)
    }

    /// The next event, or `Ok(None)` when the server closed the stream
    /// cleanly.
    pub async fn next_event(&mut self) -> Result<Option<Event>, TestkitError> {
        let message = tokio::time::timeout(IDLE, self.incoming.message())
            .await
            .map_err(|_| TestkitError::Timeout)?
            .map_err(rpc)?;
        Ok(message.and_then(|response| response.event))
    }

    /// Reads until `pred` matches one event, inclusive.
    pub async fn until(
        &mut self,
        pred: impl Fn(&Event) -> bool,
    ) -> Result<Vec<Event>, TestkitError> {
        let mut events = Vec::new();
        loop {
            match self.next_event().await? {
                Some(event) => {
                    let done = pred(&event);
                    events.push(event);
                    if done {
                        return Ok(events);
                    }
                }
                None => return Err(TestkitError::Closed),
            }
        }
    }
}

/// Opens a stream, sends `requests` in order, and reads until `pred`.
pub async fn drive(
    channel: Channel,
    requests: Vec<ConverseRequest>,
    pred: impl Fn(&Event) -> bool,
) -> Result<Vec<Event>, TestkitError> {
    let mut session = ConverseSession::open(channel).await?;
    for request in requests {
        session.send(request).await?;
    }
    session.until(pred).await
}

/// Binds the stream to a chat. Must be the first submission.
#[must_use]
pub fn attach(chat_id: impl Into<String>) -> ConverseRequest {
    ConverseRequest {
        submission: Some(Submission::Attach(Attach {
            chat_id: chat_id.into(),
        })),
    }
}

/// A user message. The server answers with `TurnStarted`.
#[must_use]
pub fn user_turn(text: impl Into<String>) -> ConverseRequest {
    ConverseRequest {
        submission: Some(Submission::UserTurn(UserTurn { text: text.into() })),
    }
}

/// Changes the model stored for this chat from the next turn on.
#[must_use]
pub fn switch_model(provider: impl Into<String>, model: impl Into<String>) -> ConverseRequest {
    ConverseRequest {
        submission: Some(Submission::SwitchModel(SwitchModel {
            model: Some(ModelRef {
                provider: provider.into(),
                model: model.into(),
            }),
        })),
    }
}

/// Stops the running turn. `all` is the kill switch and needs `control`.
#[must_use]
pub fn interrupt(all: bool) -> ConverseRequest {
    ConverseRequest {
        submission: Some(Submission::Interrupt(Interrupt { all })),
    }
}

fn rpc(status: tonic::Status) -> TestkitError {
    TestkitError::Rpc {
        code: status.code().to_string(),
        message: status.message().to_owned(),
    }
}
