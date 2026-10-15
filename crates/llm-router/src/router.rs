//! The table that turns a job into the provider for its tier.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use llm_wire::Provider;

use crate::{Job, Tier};

/// Why a job could not be routed. The router fails closed: the caller does
/// not get a provider, and no model is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RouteError {
    /// `tier` has no provider. Build one and register it, or point the job
    /// at a tier that already has one.
    #[error("no provider for the {tier} tier")]
    NoProvider {
        /// The tier the job resolved to.
        tier: Tier,
    },
}

/// One provider per tier, and an optional override of which tier a job uses.
#[derive(Clone)]
pub struct Router {
    providers: BTreeMap<Tier, Arc<dyn Provider>>,
    jobs: BTreeMap<Job, Tier>,
}

impl fmt::Debug for Router {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router")
            .field("tiers", &self.providers.keys().copied().collect::<Vec<_>>())
            .field("jobs", &self.jobs)
            .finish()
    }
}

impl Router {
    /// Starts an empty router. A job routes only after its tier has a provider.
    #[must_use]
    pub fn builder() -> RouterBuilder {
        RouterBuilder {
            providers: BTreeMap::new(),
            jobs: BTreeMap::new(),
        }
    }

    /// The tier `job` resolves to: the override, or [`Job::default_tier`].
    #[must_use]
    pub fn tier_of(&self, job: Job) -> Tier {
        self.jobs.get(&job).copied().unwrap_or(job.default_tier())
    }

    /// The provider registered for `tier`.
    ///
    /// # Errors
    ///
    /// [`RouteError::NoProvider`] when nothing was registered for `tier`.
    pub fn provider(&self, tier: Tier) -> Result<Arc<dyn Provider>, RouteError> {
        self.providers
            .get(&tier)
            .cloned()
            .ok_or(RouteError::NoProvider { tier })
    }

    /// The provider for `job`, after resolving its tier.
    ///
    /// # Errors
    ///
    /// [`RouteError::NoProvider`] when that tier has no provider. Nothing is called.
    pub fn route(&self, job: Job) -> Result<Arc<dyn Provider>, RouteError> {
        self.provider(self.tier_of(job))
    }
}

/// Builds a [`Router`]. Registering a tier twice keeps the later provider.
#[derive(Clone)]
pub struct RouterBuilder {
    providers: BTreeMap<Tier, Arc<dyn Provider>>,
    jobs: BTreeMap<Job, Tier>,
}

impl fmt::Debug for RouterBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouterBuilder")
            .field("tiers", &self.providers.keys().copied().collect::<Vec<_>>())
            .field("jobs", &self.jobs)
            .finish()
    }
}

impl RouterBuilder {
    /// Uses `provider` for every job that resolves to `tier`.
    #[must_use]
    pub fn provider(mut self, tier: Tier, provider: Arc<dyn Provider>) -> Self {
        self.providers.insert(tier, provider);
        self
    }

    /// Sends `job` to `tier` instead of [`Job::default_tier`].
    #[must_use]
    pub fn job(mut self, job: Job, tier: Tier) -> Self {
        self.jobs.insert(job, tier);
        self
    }

    /// Finishes the router. Missing tiers are not an error here; [`Router::route`]
    /// fails closed when a job asks for one.
    #[must_use]
    pub fn build(self) -> Router {
        Router {
            providers: self.providers,
            jobs: self.jobs,
        }
    }
}
