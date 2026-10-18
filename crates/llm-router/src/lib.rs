//! One provider per tier (spec §10, T1.7).
//!
//! A [`Router`] holds the provider built for each [`Tier`] and sends a [`Job`]
//! to that tier's provider. The voice front, the main agent and the sentinel
//! can therefore call different models. The caller builds the providers; this
//! crate does not open a socket or read a key.
//!
//! A job with no provider for its tier fails closed: [`Router::route`] returns
//! [`RouteError::NoProvider`] and nothing is called.

mod job;
mod router;

pub use job::{Job, Tier};
pub use router::{RouteError, Router, RouterBuilder};
