//! The one error type of aulo-server, so `aulod` can tell a busy socket or a
//! refused remote bind or a broken certificate apart from a plain I/O failure.

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
        "refusing to serve the API without TLS on non-loopback address {0}; \
         configure TLS or bind to 127.0.0.1 or ::1"
    )]
    UnauthenticatedRemote(SocketAddr),
    #[error("{0} is accessible to other users; restrict it to the owner (chmod 600)")]
    InsecureFile(PathBuf),
    #[error("{path}: {source}")]
    Pem {
        path: PathBuf,
        #[source]
        source: rustls::pki_types::pem::Error,
    },
    #[error("{0} holds no certificate")]
    NoCertificate(PathBuf),
    #[error("the TLS certificate and key are not a usable pair: {0}")]
    TlsIdentity(#[source] rustls::Error),
    #[error("cannot build the TLS client config: {0}")]
    TlsClient(#[source] rustls::Error),
    #[error("cannot generate a self-signed certificate: {0}")]
    SelfSigned(#[from] rcgen::Error),
    #[error("a certificate fingerprint is 32 bytes of hex, optionally colon-separated")]
    InvalidFingerprint,
    #[error("refusing to serve TCP without an API token; install one with ApiServer::with_token")]
    TcpWithoutToken,
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
