//! Assembles the `aulod` gRPC server: listeners, transport limits, health,
//! reflection and graceful shutdown.
//!
//! Service implementations come from callers; this crate only decides how they
//! are exposed. Chat history and standing grants are the exception: [`mount_database`]
//! opens `aulo.db` and mounts those two services for `aulod`. Every listener
//! authenticates its clients ([`auth`]); the TCP listener adds TLS ([`tls`]) and
//! refuses a non-loopback address without it.

pub mod auth;
mod chat;
mod config_api;
mod error;
mod grant;
mod limits;
mod listener;
#[cfg(windows)]
mod pipe;
mod server;
pub mod tls;

pub use chat::ChatApi;
pub use config_api::ConfigApi;
pub use error::ServerError;
pub use grant::{GrantApi, mount_database};
pub use limits::{Limits, MessageLimits};
pub use listener::{Bound, Listen, TcpListen, bind, local_socket_path};
pub use server::ApiServer;
pub use tokio_util::sync::CancellationToken;
