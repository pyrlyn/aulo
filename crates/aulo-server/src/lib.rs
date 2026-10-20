//! Assembles the `aulod` gRPC server: listeners, transport limits, health,
//! reflection and graceful shutdown.
//!
//! Service implementations come from callers; this crate only decides how they
//! are exposed. Every listener authenticates its clients ([`auth`]); the TCP
//! listener adds TLS ([`tls`]) and refuses a non-loopback address without it.

pub mod auth;
mod chat;
pub mod client;
mod config_api;
mod error;
mod limits;
mod listener;
#[cfg(windows)]
mod pipe;
mod scripted;
mod server;
mod text;
pub mod tls;

pub use chat::ChatApi;
pub use client::{
    ChatEvent, ChatLine, ChatList, ClientError, Conversation, DaemonClient, DaemonStatus,
    TurnOutcome,
};
pub use config_api::ConfigApi;
pub use error::ServerError;
pub use limits::{Limits, MessageLimits};
#[cfg(windows)]
pub use listener::local_pipe_name;
pub use listener::{Bound, Listen, TcpListen, bind, local_socket_path};
pub use scripted::{DEFAULT_SCENARIO, ScenarioError, ScriptedProvider, ScriptedSession};
pub use server::ApiServer;
pub use text::{MAX_TURN_TEXT_BYTES, MAX_USER_TEXT_BYTES, plain_text};
pub use tokio_util::sync::CancellationToken;
