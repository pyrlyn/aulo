//! A [`Provider`] that replays a `llm-wire` scenario.
//!
//! Turns come from [`parse_scenario`]. A turn with `when_contains`,
//! `when_system_contains` or `job` is pinned and is used only when the
//! request matches; a `title` request never consumes an unpinned turn.
//! Everything else is answered in file order.

use std::sync::{Mutex, PoisonError};

use async_trait::async_trait;
use llm_wire::test_util::scripted::{TurnSpec, events_for, parse_scenario};
use llm_wire::{
    Caps, Content, Job, Provider, ProviderError, ProviderEvent, ProviderId, Request, SystemBlock,
    Usage,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Estimated usage every scripted call reports. Real wires replace this
/// with the provider's own counts.
fn usage() -> Usage {
    Usage {
        input_tokens: 10,
        output_tokens: 5,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        estimated: true,
        cost_usd: 0.0,
        latency_ms: 0,
    }
}

/// Answers each call from a scenario. Requests are kept, in arrival order,
/// so a test can assert what the next turn sent.
#[derive(Default)]
pub struct Scripted {
    turns: Mutex<Vec<TurnSpec>>,
    requests: Mutex<Vec<Request>>,
}

impl std::fmt::Debug for Scripted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let turns = lock(&self.turns).len();
        let requests = lock(&self.requests).len();
        f.debug_struct("Scripted")
            .field("turns_left", &turns)
            .field("requests", &requests)
            .finish()
    }
}

impl Scripted {
    /// Parses `toml` as a `[[turn]]` scenario.
    pub fn from_toml(toml: &str) -> Result<Self, ProviderError> {
        Ok(Self::from_turns(parse_scenario(toml)?))
    }

    /// Replays `turns` in order, honouring pins.
    #[must_use]
    pub fn from_turns(turns: Vec<TurnSpec>) -> Self {
        Self {
            turns: Mutex::new(turns),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Every request `stream` was given, in arrival order.
    #[must_use]
    pub fn requests(&self) -> Vec<Request> {
        lock(&self.requests).clone()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn transcript_of(request: &Request) -> String {
    let mut text = String::new();
    for message in &request.messages {
        for block in &message.content {
            match block {
                Content::Text { text: part } | Content::Thinking { text: part, .. } => {
                    text.push_str(part);
                }
                Content::ToolResult { content, .. } => text.push_str(content),
                Content::ToolUse { name, .. } => text.push_str(name),
                Content::Image { .. } | Content::Pointer { .. } => {}
            }
        }
    }
    text
}

fn system_of(request: &Request) -> String {
    let mut text = String::new();
    for SystemBlock { text: part, .. } in &request.system {
        text.push_str(part);
    }
    text
}

fn pinned(turn: &TurnSpec) -> bool {
    turn.job.is_some() || turn.when_contains.is_some() || turn.when_system_contains.is_some()
}

fn pins_match(turn: &TurnSpec, request: &Request, transcript: &str, system: &str) -> bool {
    if let Some(job) = &turn.job
        && job != &request.job
    {
        return false;
    }
    if let Some(needle) = &turn.when_contains
        && !transcript.contains(needle)
    {
        return false;
    }
    if let Some(needle) = &turn.when_system_contains
        && !system.contains(needle)
    {
        return false;
    }
    true
}

fn take_turn(turns: &mut Vec<TurnSpec>, request: &Request) -> Result<TurnSpec, ProviderError> {
    let transcript = transcript_of(request);
    let system = system_of(request);
    let pinned_at = turns
        .iter()
        .position(|turn| pinned(turn) && pins_match(turn, request, &transcript, &system));
    let index = if let Some(index) = pinned_at {
        index
    } else if request.job == Job::Title {
        return Err(ProviderError::Unsupported {
            feature: "no scripted turn for this title request".into(),
        });
    } else {
        turns
            .iter()
            .position(|turn| !pinned(turn))
            .ok_or_else(|| ProviderError::Unsupported {
                feature: "scripted scenario ran out".into(),
            })?
    };
    Ok(turns.remove(index))
}

#[async_trait]
impl Provider for Scripted {
    fn id(&self) -> ProviderId {
        ProviderId::Local
    }

    fn capabilities(&self) -> Caps {
        Caps {
            cache: false,
            thinking: false,
            server_tools: false,
            count_tokens: false,
            max_context: 200_000,
        }
    }

    async fn stream(
        &self,
        request: Request,
        sink: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<Usage, ProviderError> {
        let model = request.model.clone();
        let mut names: Vec<String> = request.tools.iter().map(|tool| tool.name.clone()).collect();
        let turn = {
            let mut turns = lock(&self.turns);
            let turn = take_turn(&mut turns, &request)?;
            drop(turns);
            lock(&self.requests).push(request);
            turn
        };
        let mut turn = turn;
        if turn.echo_tools {
            names.sort_unstable();
            turn.text = Some(names.join("\n"));
        }
        let failed = turn.error.clone();
        for event in events_for(&turn, model, usage()) {
            if cancel.is_cancelled() || sink.send(event).await.is_err() {
                return Err(ProviderError::Cancelled);
            }
        }
        match failed {
            Some(message) => Err(ProviderError::BadRequest { message }),
            None => Ok(usage()),
        }
    }

    async fn count_tokens(&self, _request: &Request) -> Result<u32, ProviderError> {
        Ok(0)
    }
}

/// A request with one user text block, for tests that build a turn by hand.
#[cfg(test)]
fn user_request(text: &str, job: Job) -> Request {
    use llm_wire::{Effort, Message, ModelId, Role, Thinking, Tier};
    Request {
        tier: Tier::Code,
        job,
        model: ModelId("scripted".into()),
        system: vec![],
        tools: vec![],
        messages: vec![Message {
            role: Role::User,
            content: vec![Content::Text { text: text.into() }],
        }],
        effort: Effort::High,
        max_tokens: 64,
        thinking: Thinking::Off,
        cache_breakpoints: vec![],
        stop_sequences: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_wire::ToolSpec;

    #[tokio::test]
    async fn a_switch_of_turns_is_what_the_next_call_streams() {
        let provider = Scripted::from_toml(
            r#"
[[turn]]
text = "first"
[[turn]]
text = "second"
"#,
        )
        .expect("scenario");
        let cancel = CancellationToken::new();
        let (tx, mut rx) = mpsc::channel(8);
        provider
            .stream(user_request("one", Job::Main), tx, cancel)
            .await
            .expect("stream");
        let mut text = String::new();
        while let Some(event) = rx.recv().await {
            if let ProviderEvent::TextDelta { text: delta } = event {
                text.push_str(&delta);
            }
        }
        assert_eq!(text, "first");
        assert_eq!(provider.requests().len(), 1);
        assert!(transcript_of(&provider.requests()[0]).contains("one"));
    }

    #[tokio::test]
    async fn a_pinned_turn_is_not_stolen_by_an_earlier_positional_one() {
        let provider = Scripted::from_toml(
            r#"
[[turn]]
text = "plain"

[[turn]]
text = "pinned"
when_contains = "MARKER"

[[turn]]
text = "titled"
job = "title"
"#,
        )
        .expect("scenario");

        let text = stream_text(&provider, user_request("see MARKER", Job::Main)).await;
        assert_eq!(text, "pinned");
        let text = stream_text(&provider, user_request("name this", Job::Title)).await;
        assert_eq!(text, "titled");
        let text = stream_text(&provider, user_request("ordinary", Job::Main)).await;
        assert_eq!(text, "plain");
    }

    #[tokio::test]
    async fn echo_tools_lists_the_offered_names() {
        let provider = Scripted::from_toml("[[turn]]\necho_tools = true\n").expect("scenario");
        let mut request = user_request("go", Job::Main);
        request.tools = vec![tool("zeta"), tool("alpha")];
        let text = stream_text(&provider, request).await;
        assert_eq!(text, "alpha\nzeta");
    }

    #[tokio::test]
    async fn a_title_request_does_not_consume_the_main_turn() {
        let provider = Scripted::from_toml("[[turn]]\ntext = \"main\"\n").expect("scenario");
        let error = provider
            .stream(
                user_request("title", Job::Title),
                mpsc::channel(1).0,
                CancellationToken::new(),
            )
            .await
            .expect_err("no title turn");
        assert!(matches!(error, ProviderError::Unsupported { .. }));
        let text = stream_text(&provider, user_request("go", Job::Main)).await;
        assert_eq!(text, "main");
    }

    #[test]
    fn bad_toml_is_rejected() {
        assert!(Scripted::from_toml("[[turn][").is_err());
    }

    fn tool(name: &str) -> ToolSpec {
        use llm_wire::Risk;
        ToolSpec {
            name: name.into(),
            description: String::new(),
            input_schema: serde_json::json!({}),
            deferred: false,
            risk: Risk::ReadOnly,
            concurrency: llm_wire::Concurrency::Parallel,
        }
    }

    async fn stream_text(provider: &Scripted, request: Request) -> String {
        let (tx, mut rx) = mpsc::channel(8);
        provider
            .stream(request, tx, CancellationToken::new())
            .await
            .expect("stream");
        let mut text = String::new();
        while let Some(event) = rx.recv().await {
            if let ProviderEvent::TextDelta { text: delta } = event {
                text.push_str(&delta);
            }
        }
        text
    }
}
