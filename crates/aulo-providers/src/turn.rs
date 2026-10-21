//! The provider request for one chat turn, from the model the chat stores.
//!
//! The stored string is `provider/model` (the same spelling `ChatService`
//! writes). A switch replaces that string; the next [`prepare_turn`] sends
//! the new model id to the endpoint named before the first slash. The
//! Converse stream that carries `SwitchModel` is T4.8; this is the request
//! that stream will build.

use llm_wire::{Content, Effort, Job, Message, ModelId, Request, Role, Thinking, Tier};

/// The largest `UserTurn.text` the proto accepts. Longer text is rejected,
/// not truncated, so the request cannot carry a prompt the user did not write.
const MAX_USER_TEXT_BYTES: usize = 64 * 1024;
const MAX_PROVIDER_BYTES: usize = 64;
const MAX_MODEL_BYTES: usize = 128;
/// Output cap until the agent loop (T4.6) applies the catalog's max output.
const TURN_MAX_TOKENS: u32 = 4_096;

/// Why a turn could not be turned into a provider request.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TurnError {
    /// The stored model is not `provider/model`, or a part is empty, too
    /// long or contains a control character.
    #[error("chat model must be provider/model, with no control characters")]
    BadModel,
    /// The provider half names no configured endpoint.
    #[error("no configured endpoint named {0}")]
    UnknownEndpoint(String),
    /// Empty, or longer than the proto's 64 KiB cap.
    #[error("user text must be non-empty and at most 64 KiB")]
    BadText,
}

/// What the next provider call for this chat is.
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    /// Endpoint name under `providers.endpoints`.
    pub endpoint: String,
    /// The request that endpoint's provider is given.
    pub request: Request,
}

/// The store's single model string for a provider and a model id.
pub fn stored_model(provider: &str, model: &str) -> Result<String, TurnError> {
    if !part_ok(provider, MAX_PROVIDER_BYTES)
        || provider.contains('/')
        || !part_ok(model, MAX_MODEL_BYTES)
    {
        return Err(TurnError::BadModel);
    }
    Ok(format!("{provider}/{model}"))
}

/// Builds the request the next turn sends. `endpoints` are the configured
/// endpoint names; `stored_model` is the chat's current model string.
pub fn prepare_turn(
    endpoints: &[&str],
    stored_model: &str,
    user_text: &str,
) -> Result<Turn, TurnError> {
    if user_text.is_empty() || user_text.len() > MAX_USER_TEXT_BYTES {
        return Err(TurnError::BadText);
    }
    let (provider, model) = split_stored(stored_model)?;
    if !endpoints.contains(&provider) {
        return Err(TurnError::UnknownEndpoint(provider.to_owned()));
    }
    Ok(Turn {
        endpoint: provider.to_owned(),
        request: Request {
            // aulo's main tier is the wire's `code` tier. llm-router keeps
            // aulo's own tier enum; the agent loop is what maps them. This
            // request only has to name the model the call sends.
            tier: Tier::Code,
            job: Job::Main,
            model: ModelId(model.to_owned()),
            system: Vec::new(),
            tools: Vec::new(),
            messages: vec![Message {
                role: Role::User,
                content: vec![Content::Text {
                    text: user_text.to_owned(),
                }],
            }],
            effort: Effort::Medium,
            max_tokens: TURN_MAX_TOKENS,
            thinking: Thinking::Off,
            cache_breakpoints: Vec::new(),
            stop_sequences: Vec::new(),
        },
    })
}

fn split_stored(stored: &str) -> Result<(&str, &str), TurnError> {
    let Some((provider, model)) = stored.split_once('/') else {
        return Err(TurnError::BadModel);
    };
    // `split_once` already took the first slash, so the provider half has none.
    if !part_ok(provider, MAX_PROVIDER_BYTES) || !part_ok(model, MAX_MODEL_BYTES) {
        return Err(TurnError::BadModel);
    }
    Ok((provider, model))
}

fn part_ok(raw: &str, max: usize) -> bool {
    !raw.is_empty() && raw.len() <= max && !raw.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENDPOINTS: &[&str] = &["openai", "anthropic"];

    #[test]
    fn a_switch_changes_the_endpoint_and_the_model_on_the_next_request() {
        let first = stored_model("openai", "gpt-5.5").unwrap();
        let turn = prepare_turn(ENDPOINTS, &first, "hello").unwrap();
        assert_eq!(turn.endpoint, "openai");
        assert_eq!(turn.request.model.0, "gpt-5.5");
        assert!(matches!(
            &turn.request.messages[0].content[0],
            Content::Text { text } if text == "hello"
        ));

        let switched = stored_model("anthropic", "claude-sonnet-5").unwrap();
        let turn = prepare_turn(ENDPOINTS, &switched, "and now").unwrap();
        assert_eq!(turn.endpoint, "anthropic");
        assert_eq!(turn.request.model.0, "claude-sonnet-5");
    }

    #[test]
    fn a_model_id_may_contain_slashes() {
        let stored = stored_model("openrouter", "anthropic/claude-sonnet-5").unwrap();
        let turn = prepare_turn(&["openrouter"], &stored, "hello").unwrap();
        assert_eq!(turn.request.model.0, "anthropic/claude-sonnet-5");
    }

    #[test]
    fn a_bad_model_or_an_unknown_endpoint_is_refused() {
        assert_eq!(stored_model("open/ai", "gpt-5.5"), Err(TurnError::BadModel));
        assert_eq!(stored_model("openai", ""), Err(TurnError::BadModel));
        assert_eq!(
            prepare_turn(ENDPOINTS, "gpt-5.5", "hello"),
            Err(TurnError::BadModel)
        );
        assert_eq!(
            prepare_turn(ENDPOINTS, "missing/gpt-5.5", "hello"),
            Err(TurnError::UnknownEndpoint("missing".into()))
        );
        assert_eq!(
            prepare_turn(ENDPOINTS, "openai/gpt-5.5", ""),
            Err(TurnError::BadText)
        );
    }
}
