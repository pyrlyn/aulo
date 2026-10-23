//! The approval gate: the loop's `Approvals` seam implemented over a
//! [`Policy`], so every tool call — built-in, MCP or plugin — is judged by
//! the same deterministic rules before it runs.
//!
//! Grants come from two places. Standing grants (an "always" answer,
//! persisted with scope and expiry) are read through [`Grants`], which the
//! daemon implements over `aulo-store`; session grants (an
//! "always-for-this-chat" answer) live in the gate for the session's life.
//! The gate itself does no I/O.

use std::sync::{Arc, Mutex, PoisonError};

use agent_loop::traits::{Approvals, Verdict};
use async_trait::async_trait;
use llm_wire::Risk as WireRisk;
use perm_rules::{Call, DecidedBy, Outcome, Risk, Segments, Why};

use crate::policy::Policy;
use agent_loop::types::ToolCall;

/// Where standing grants come from. The daemon implements this over
/// `aulo-store`; a test implements it over a vector.
pub trait Grants: Send + Sync {
    /// The active `(tool, subject)` grants for calls to `tool`, expiry
    /// already filtered.
    fn active(&self, tool: &str) -> Vec<(String, String)>;
}

/// A gate over one [`Policy`] and one grant source. One per session, so
/// session grants end with it.
pub struct PolicyGate {
    policy: Policy,
    grants: Arc<dyn Grants>,
    session: Arc<Mutex<Vec<(String, String)>>>,
}

impl std::fmt::Debug for PolicyGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicyGate")
            .field(
                "session_grants",
                &self
                    .session
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .len(),
            )
            .finish_non_exhaustive()
    }
}

impl Clone for PolicyGate {
    fn clone(&self) -> Self {
        Self {
            policy: self.policy.clone(),
            grants: Arc::clone(&self.grants),
            session: Arc::clone(&self.session),
        }
    }
}

impl PolicyGate {
    /// A gate that reads standing grants from `grants`.
    #[must_use]
    pub fn new(policy: Policy, grants: Arc<dyn Grants>) -> Self {
        Self {
            policy,
            grants,
            session: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// The policy this gate judges with.
    #[must_use]
    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    /// The active grants for a call to `tool`: standing ones plus this
    /// session's.
    #[must_use]
    pub fn active_grants(&self, tool: &str) -> Vec<(String, String)> {
        let mut grants = self.grants.active(tool);
        grants.extend(
            self.session
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .cloned(),
        );
        grants
    }

    /// Decides a call directly, for a surface that is not the loop (the
    /// MCP server, the CLI).
    #[must_use]
    pub fn decide_call(&self, call: &Call) -> Outcome {
        self.policy.decide(call, &self.active_grants(&call.tool))
    }

    /// Records the session half of an allow-for-this-chat answer. The
    /// standing half (an "always" answer) is written by the approval flow
    /// (T5.3), which knows which answer it was.
    pub fn record_session_grant(&self, call: &Call) {
        let grants = perm_rules::grants_for(call);
        self.session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend(grants);
    }
}

/// The engine's view of a loop call: the executor already rated it, so the
/// subject and the risk carry over unchanged.
fn engine_call(call: &ToolCall) -> Call {
    Call {
        tool: call.name.clone(),
        subject: call.subject.clone(),
        risk: map_risk(call.risk),
        segments: call.segments.as_ref().map(|s| Segments {
            commands: s.commands.clone(),
            opaque: s.opaque,
        }),
    }
}

fn map_risk(risk: WireRisk) -> Risk {
    match risk {
        WireRisk::ReadOnly => Risk::ReadOnly,
        WireRisk::Write => Risk::Write,
        WireRisk::Exec => Risk::Exec,
        WireRisk::Destructive => Risk::Destructive,
    }
}

fn map_risk_back(risk: Risk) -> WireRisk {
    match risk {
        Risk::ReadOnly => WireRisk::ReadOnly,
        Risk::Write => WireRisk::Write,
        Risk::Exec => WireRisk::Exec,
        Risk::Destructive => WireRisk::Destructive,
    }
}

fn map_why(why: Why) -> agent_loop::types::Why {
    match why {
        Why::RuleAsk { rule } => agent_loop::types::Why::RuleAsk { rule },
        Why::Risk { risk } => agent_loop::types::Why::Risk {
            risk: map_risk_back(risk),
        },
        Why::Policy { policy } => agent_loop::types::Why::Policy {
            policy: approval_name(policy).to_owned(),
        },
    }
}

fn approval_name(policy: perm_rules::ApprovalPolicy) -> &'static str {
    match policy {
        perm_rules::ApprovalPolicy::Untrusted => "untrusted",
        perm_rules::ApprovalPolicy::OnRequest => "on-request",
        perm_rules::ApprovalPolicy::OnFailure => "on-failure",
        perm_rules::ApprovalPolicy::Never => "never",
    }
}

#[async_trait]
impl Approvals for PolicyGate {
    async fn decide(&self, call: &ToolCall) -> Verdict {
        match self.decide_call(&engine_call(call)) {
            Outcome::Allow { .. } => Verdict::Allow,
            Outcome::Deny { reason, by } => Verdict::Deny {
                reason,
                by: map_decided_by(by),
            },
            Outcome::Ask(why) => Verdict::Ask(map_why(why)),
        }
    }

    async fn grant(&self, call: &ToolCall) {
        self.record_session_grant(&engine_call(call));
    }
}

fn map_decided_by(by: DecidedBy) -> agent_loop::types::DecidedBy {
    match by {
        DecidedBy::Rule => agent_loop::types::DecidedBy::Rule,
        DecidedBy::Session => agent_loop::types::DecidedBy::Session,
        DecidedBy::Policy => agent_loop::types::DecidedBy::Policy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{Policy, Settings};
    use crate::subjects::{app_call, input_call};
    use agent_loop::types::{ToolCall as LoopCall, ToolResult, Why as LoopWhy};
    use std::path::PathBuf;
    use std::sync::Mutex as StdMutex;

    /// Standing grants from a vector.
    #[derive(Default)]
    struct Store {
        grants: StdMutex<Vec<(String, String)>>,
    }

    impl Grants for Store {
        fn active(&self, _tool: &str) -> Vec<(String, String)> {
            self.grants.lock().expect("grants").clone()
        }
    }

    fn gate() -> PolicyGate {
        let policy = Policy::new(
            &perm_rules::RuleSet::default(),
            Settings::default(),
            None,
            &PathBuf::from("."),
        )
        .expect("compiles");
        PolicyGate::new(policy, Arc::new(Store::default()))
    }

    fn loop_call(name: &str, subject: &str, risk: WireRisk) -> LoopCall {
        LoopCall {
            id: llm_wire::CallId::new(),
            name: name.into(),
            input: serde_json::json!({}),
            risk,
            subject: subject.into(),
            segments: None,
        }
    }

    #[tokio::test]
    async fn a_risk_ask_becomes_a_loop_ask() {
        let gate = gate();
        let verdict = gate
            .decide(&loop_call("app_launch", "com.apple.Safari", WireRisk::Exec))
            .await;
        assert!(matches!(verdict, Verdict::Ask(_)));
    }

    #[tokio::test]
    async fn a_session_grant_covers_the_next_call() {
        let gate = gate();
        let call = loop_call("app_launch", "com.apple.Safari", WireRisk::Exec);
        gate.grant(&call).await;
        let verdict = gate.decide(&call).await;
        assert!(matches!(verdict, Verdict::Allow));
    }

    #[tokio::test]
    async fn a_deny_rule_is_a_loop_deny() {
        let policy = Policy::new(
            &perm_rules::RuleSet {
                deny: vec!["Input(type)".into()],
                ..perm_rules::RuleSet::default()
            },
            Settings::default(),
            None,
            &PathBuf::from("."),
        )
        .expect("compiles");
        let gate = PolicyGate::new(policy, Arc::new(Store::default()));
        let verdict = gate
            .decide(&loop_call("input_type", "type", WireRisk::Exec))
            .await;
        let Verdict::Deny { reason, by } = verdict else {
            panic!("a denial");
        };
        assert!(reason.contains("Input(type)"), "{reason}");
        assert_eq!(by, agent_loop::types::DecidedBy::Rule);
    }

    #[tokio::test]
    async fn an_ask_why_keeps_the_rule_that_matched() {
        let policy = Policy::new(
            &perm_rules::RuleSet {
                ask: vec!["App(com.apple.Safari)".into()],
                ..perm_rules::RuleSet::default()
            },
            Settings::default(),
            None,
            &PathBuf::from("."),
        )
        .expect("compiles");
        let gate = PolicyGate::new(policy, Arc::new(Store::default()));
        let verdict = gate
            .decide(&loop_call("app_launch", "com.apple.Safari", WireRisk::Exec))
            .await;
        assert!(matches!(
            verdict,
            Verdict::Ask(LoopWhy::RuleAsk { rule }) if rule == "App(com.apple.Safari)"
        ));
    }

    #[test]
    fn the_gate_decides_a_non_loop_call_too() {
        let gate = gate();
        assert!(matches!(
            gate.decide_call(&app_call("com.apple.Safari")),
            Outcome::Ask(_)
        ));
        assert!(matches!(
            gate.decide_call(&input_call("click")),
            Outcome::Ask(_)
        ));
        // A tool result is never built by the gate: this only checks the
        // type is in scope for surfaces that map one.
        let _ = ToolResult::failed("unused");
    }
}
