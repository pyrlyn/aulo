//! Assembles the `aulod` gRPC server: listeners, transport limits, health,
//! reflection and graceful shutdown.
//!
//! Service implementations come from callers; this crate only decides how they
//! are exposed. Every listener authenticates its clients ([`auth`]); TLS
//! (T3.8) plugs in at the TCP listener, which refuses non-loopback addresses
//! until it exists.

pub mod auth;
mod chat;
mod error;
mod limits;
mod listener;
#[cfg(windows)]
mod pipe;
mod server;

pub use chat::ChatApi;
pub use error::ServerError;
pub use limits::{Limits, MessageLimits};
pub use listener::{Bound, Listen, TcpListen, bind, local_socket_path};
pub use server::ApiServer;
pub use tokio_util::sync::CancellationToken;
