//! Assembles the `aulod` gRPC server: listeners, transport limits, health,
//! reflection and graceful shutdown.
//!
//! Service implementations come from callers; this crate only decides how they
//! are exposed. Authentication (T3.4) and TLS (T3.8) plug in at the TCP
//! listener, which refuses non-loopback addresses until they exist.

mod error;
mod limits;
mod listener;
#[cfg(windows)]
mod pipe;
mod server;

pub use error::ServerError;
pub use limits::{Limits, MessageLimits};
pub use listener::{Bound, Listen, TcpListen, bind, local_socket_path};
pub use server::ApiServer;
pub use tokio_util::sync::CancellationToken;
