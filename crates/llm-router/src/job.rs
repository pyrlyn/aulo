//! The tiers aulo routes on, and the jobs that land on them.
//!
//! These are aulo's tiers (spec §10), not `llm_wire::Tier`. That enum is the
//! coding-agent scale (`cheap`, `code`, `think`); changing it would break cox.
//! A cox caller maps its own tier onto one of these before it asks the router.

use serde::{Deserialize, Serialize};

/// Who a model call is for. Each job has a default [`Tier`]; a router may
/// override the default per job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Job {
    /// A spoken answer, or a turn of the realtime front.
    Voice,
    /// The main agent turn, including tool calls.
    Main,
    /// A sentinel review of a consequential action.
    Sentinel,
    /// A chat title after the first turn.
    Title,
    /// A compaction summary of older turns.
    Summary,
}

impl Job {
    /// The tier this job uses when the router has no override.
    #[must_use]
    pub const fn default_tier(self) -> Tier {
        match self {
            Self::Voice => Tier::Voice,
            Self::Main => Tier::Main,
            Self::Sentinel => Tier::Sentinel,
            Self::Title | Self::Summary => Tier::Cheap,
        }
    }

    /// The config and log name, matching the serde form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Voice => "voice",
            Self::Main => "main",
            Self::Sentinel => "sentinel",
            Self::Title => "title",
            Self::Summary => "summary",
        }
    }
}

/// A routing tier (spec §10). One provider is built for each tier that is in use.
///
/// Ordered cheapest first, so a later "never escalate" check can compare two
/// tiers with `<=`: `Cheap < Voice < Main < Sentinel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Titles and summaries.
    Cheap,
    /// Fast, short answers, or the realtime front.
    Voice,
    /// The main agent, with tools.
    Main,
    /// The reviewer. It can only make a decision stricter.
    Sentinel,
}

impl Tier {
    /// The config and log name, matching the serde form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cheap => "cheap",
            Self::Voice => "voice",
            Self::Main => "main",
            Self::Sentinel => "sentinel",
        }
    }
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::fmt::Display for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
