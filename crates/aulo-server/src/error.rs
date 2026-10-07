//! The one error type of aulo-server, so `aulod` can tell a busy socket or a
//! refused remote bind apart from a plain I/O failure.

use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("socket path {0} must be absolute and have a parent directory")]
    InvalidSocketPath(PathBuf),
    #[error("{0} is not a directory")]
    NotADirectory(PathBuf),
    #[error("{0} is accessible to other users; restrict it to the owner (chmod 700)")]
    InsecureDirectory(PathBuf),
    #[error("{0} exists and is not a socket; refusing to replace it")]
    NotASocket(PathBuf),
    #[error("{0} is in use by a running server")]
    SocketInUse(PathBuf),
    #[error(
        "refusing to serve the API without authentication on non-loopback address {0}; \
         bind to 127.0.0.1 or ::1"
    )]
    UnauthenticatedRemote(SocketAddr),
    #[error("{action} {target}: {source}")]
    Io {
        action: &'static str,
        target: String,
        #[source]
        source: io::Error,
    },
    #[error("reflection descriptors are invalid: {0}")]
    Reflection(#[from] tonic_reflection::server::Error),
    #[error("transport: {0}")]
    Transport(#[from] tonic::transport::Error),
    #[error("listener task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

impl ServerError {
    pub(crate) fn io(
        action: &'static str,
        target: impl ToString,
    ) -> impl FnOnce(io::Error) -> Self {
        let target = target.to_string();
        move |source| Self::Io {
            action,
            target,
            source,
        }
    }
}
