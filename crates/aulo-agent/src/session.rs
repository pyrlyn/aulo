//! One chat session: the daemon's handle on a conversation.
//!
//! A [`Session`] wraps one `agent_loop::Agent` for one chat. Submissions
//! go in ([`Session::submit`]), events come out on the channel the caller
//! owns, and an interrupt from any client is synchronous so a kill switch
//! acts at once. The loop owns ordering, concurrency, approvals and
//! cancellation; this module maps its events onto [`SessionEvent`] and
//! re-keys its ids onto the `aulo-types` id space.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use aulo_types::{CallId, ChatId};
use llm_wire::{Message, Provider};
use tokio::sync::mpsc;

use agent_loop::traits::{Approvals, Context, Tools};
use agent_loop::types::{Decision, LoopError};

use crate::events::{SessionEvent, SessionState};
use crate::sink::SessionSink;

/// The largest `UserTurn.text` accepted. Longer text is rejected, not
/// truncated, so a session never runs a prompt the user did not write
/// (the same cap the proto documents).
pub const MAX_USER_TEXT_BYTES: usize = 64 * 1024;

/// How many mapped events a session buffers before its reader. The daemon
/// drains the channel onto every attached client; a full channel fails the
/// turn closed rather than growing without bound.
pub const EVENT_CHANNEL: usize = 256;

/// Limits for one session, passed to the loop unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionConfig {
    /// Provider calls allowed per user turn; the turn ends `MaxSteps`
    /// when they are used up.
    pub max_steps: u32,
    /// Most `Parallel` tool calls running at once; `0` means 1.
    pub parallel_tools: u32,
    /// How long an approval may wait before it counts as a deny.
    pub approval_timeout: Option<Duration>,
    /// Most bytes of a tool's visible output the model sees.
    pub tool_output_cap: usize,
}

impl Default for SessionConfig {
    /// The loop's own defaults: a generous step budget, no approval
    /// timeout (the daemon sets the configured one) and an output cap
    /// well above a tool's own truncation.
    fn default() -> Self {
        Self {
            max_steps: 200,
            parallel_tools: 4,
            approval_timeout: None,
            tool_output_cap: 64 * 1024,
        }
    }
}

/// What a session runs on. Policy is the `approvals` gate the daemon
/// installs; nothing here decides a permission.
pub struct SessionParts {
    /// The model behind the loop.
    pub provider: Arc<dyn Provider>,
    /// The tool executor: rates calls and runs the allowed ones.
    pub tools: Arc<dyn Tools>,
    /// The approval gate. Every tool call passes it before running.
    pub approvals: Arc<dyn Approvals>,
    /// Request assembly and the hooks around a provider call.
    pub context: Arc<dyn Context>,
}

impl std::fmt::Debug for SessionParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionParts").finish_non_exhaustive()
    }
}

/// A request into a session. Names follow the loop's `Submission`.
#[derive(Debug, Clone, PartialEq)]
pub enum Submission {
    /// Start a turn with user text; returns once the turn finishes.
    UserTurn {
        /// The submitted text.
        text: String,
        /// Attached files or images.
        attachments: Vec<agent_loop::Attachment>,
    },
    /// Answer a pending approval prompt.
    Approve {
        /// The call being decided.
        call_id: CallId,
        /// The answer.
        choice: ApprovalChoice,
    },
    /// Cancel the running turn: the provider stream, the tools and any
    /// approval wait share one cancellation token.
    Interrupt,
}

/// How a human answered an approval prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalChoice {
    /// Run this call once.
    Once,
    /// Run this call and let the gate grant similar calls for this chat.
    ForChat,
    /// Run this call and grant it from now on. The standing grant itself
    /// is persisted by the approval flow (T5.3); the loop sees a session
    /// grant.
    Always,
    /// Refuse the call.
    Never,
}

impl From<ApprovalChoice> for Decision {
    fn from(choice: ApprovalChoice) -> Self {
        match choice {
            ApprovalChoice::Once => Self::Allow,
            ApprovalChoice::ForChat | ApprovalChoice::Always => Self::AllowForSession,
            ApprovalChoice::Never => Self::Deny {
                reason: "denied by the user".into(),
            },
        }
    }
}

/// Why a submission failed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SessionError {
    /// Empty, or longer than [`MAX_USER_TEXT_BYTES`].
    #[error("user text must be non-empty and at most 64 KiB")]
    BadText,
    /// The answer named no pending call in this session.
    #[error("no approval pending for this call")]
    NoPendingApproval,
    /// The loop failed.
    #[error(transparent)]
    Loop(#[from] LoopError),
}

/// One conversation's turn loop. Cheap to clone; clones share the
/// conversation, so one task can run a turn while another answers an
/// approval or interrupts.
#[derive(Clone)]
pub struct Session {
    chat_id: ChatId,
    agent: agent_loop::Agent,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("chat_id", &self.chat_id)
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl Session {
    /// A session with empty history. Events go to `outbox`; the caller
    /// owns the receiving end for as long as the session lives.
    pub fn new(
        chat_id: ChatId,
        parts: SessionParts,
        config: SessionConfig,
        outbox: mpsc::Sender<SessionEvent>,
    ) -> Self {
        Self::with_history(chat_id, parts, config, Vec::new(), outbox)
    }

    /// A session that continues `history` (a chat resumed after a daemon
    /// restart, T4.8).
    pub fn with_history(
        chat_id: ChatId,
        parts: SessionParts,
        config: SessionConfig,
        history: Vec<Message>,
        outbox: mpsc::Sender<SessionEvent>,
    ) -> Self {
        let sink = Arc::new(SessionSink::new(chat_id, outbox));
        Self {
            chat_id,
            agent: agent_loop::Agent::with_history(
                agent_loop::Parts {
                    provider: parts.provider,
                    tools: parts.tools,
                    approvals: parts.approvals,
                    context: parts.context,
                    sink,
                },
                agent_loop::Config {
                    max_steps: config.max_steps,
                    parallel_tools: config.parallel_tools,
                    approval_timeout: config.approval_timeout,
                    tool_output_cap: config.tool_output_cap,
                },
                history,
            ),
        }
    }

    /// The chat this session serves.
    #[must_use]
    pub fn chat_id(&self) -> ChatId {
        self.chat_id
    }

    /// Where the running turn is.
    #[must_use]
    pub fn state(&self) -> SessionState {
        self.agent.state().into()
    }

    /// The conversation so far, in provider message form.
    pub async fn history(&self) -> Vec<Message> {
        self.agent.history().await
    }

    /// Cancels the running turn's provider stream, tools and approval
    /// wait. Synchronous so a barge-in or kill switch acts at once.
    pub fn interrupt(&self) {
        self.agent.interrupt();
    }

    /// Feeds one submission into the state machine. A `UserTurn` returns
    /// once its turn finishes; the others return at once.
    ///
    /// # Errors
    ///
    /// [`SessionError::BadText`] for an empty or oversized user turn,
    /// [`SessionError::NoPendingApproval`] when the answer named no
    /// parked call, and [`SessionError::Loop`] for a loop failure
    /// (interrupt, provider or sink).
    pub async fn submit(&self, submission: Submission) -> Result<(), SessionError> {
        match submission {
            Submission::UserTurn { text, attachments } => {
                if text.is_empty() || text.len() > MAX_USER_TEXT_BYTES {
                    return Err(SessionError::BadText);
                }
                self.agent
                    .submit(agent_loop::Submission::UserTurn { text, attachments })
                    .await?;
            }
            Submission::Approve { call_id, choice } => {
                let Ok(call) = llm_wire::CallId::from_str(&call_id.to_string()) else {
                    return Err(SessionError::NoPendingApproval);
                };
                self.agent
                    .submit(agent_loop::Submission::Approve {
                        call_id: call,
                        decision: choice.into(),
                    })
                    .await?;
            }
            Submission::Interrupt => {
                self.agent.submit(agent_loop::Submission::Interrupt).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_maps_onto_the_loop_decision_it_means() {
        let once: Decision = ApprovalChoice::Once.into();
        assert_eq!(once, Decision::Allow);
        let for_chat: Decision = ApprovalChoice::ForChat.into();
        assert_eq!(for_chat, Decision::AllowForSession);
        let always: Decision = ApprovalChoice::Always.into();
        assert_eq!(always, Decision::AllowForSession);
        let never: Decision = ApprovalChoice::Never.into();
        assert_eq!(
            never,
            Decision::Deny {
                reason: "denied by the user".into()
            }
        );
    }

    #[tokio::test]
    async fn an_empty_or_oversized_turn_is_refused_before_the_loop_sees_it() {
        let (outbox, _rx) = mpsc::channel(EVENT_CHANNEL);
        let session = Session::new(
            ChatId::new(),
            SessionParts {
                provider: Arc::new(aulo_testkit::Scripted::default()),
                tools: Arc::new(NoTools),
                approvals: Arc::new(AllowAll),
                context: Arc::new(crate::TurnContext::new("scripted")),
            },
            SessionConfig::default(),
            outbox,
        );
        assert_eq!(
            session
                .submit(Submission::UserTurn {
                    text: String::new(),
                    attachments: vec![],
                })
                .await,
            Err(SessionError::BadText)
        );
        assert_eq!(
            session
                .submit(Submission::UserTurn {
                    text: "x".repeat(MAX_USER_TEXT_BYTES + 1),
                    attachments: vec![],
                })
                .await,
            Err(SessionError::BadText)
        );
    }

    struct NoTools;

    #[async_trait::async_trait]
    impl Tools for NoTools {
        fn rate(
            &self,
            _name: &str,
            _input: &serde_json::Value,
        ) -> Option<agent_loop::traits::Rating> {
            None
        }
        fn concurrency(&self, _call: &agent_loop::types::ToolCall) -> llm_wire::Concurrency {
            llm_wire::Concurrency::Exclusive
        }
        async fn run(
            &self,
            _call: &agent_loop::types::ToolCall,
            _cx: agent_loop::traits::ToolCx,
        ) -> agent_loop::types::ToolResult {
            agent_loop::types::ToolResult::failed("no tools")
        }
    }

    struct AllowAll;

    #[async_trait::async_trait]
    impl Approvals for AllowAll {
        async fn decide(&self, _call: &agent_loop::types::ToolCall) -> agent_loop::traits::Verdict {
            agent_loop::traits::Verdict::Allow
        }
    }
}
