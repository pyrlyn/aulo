//! Failures from the test daemon and the Converse helper. A scripted
//! provider reports [`llm_wire::ProviderError`] instead.

use aulo_config::ConfigError;
use aulo_server::ServerError;
use aulo_store::StoreError;

/// Why a test daemon or a Converse drive could not finish.
#[derive(Debug, thiserror::Error)]
pub enum TestkitError {
    /// Creating the temporary home or its config file failed.
    #[error("cannot prepare the test home: {0}")]
    Home(#[source] std::io::Error),
    /// `config.toml` in the temporary home did not load.
    #[error("cannot load the test config: {0}")]
    Config(#[from] ConfigError),
    /// The temporary SQLite file could not be opened.
    #[error("cannot open the test store: {0}")]
    Store(#[from] StoreError),
    /// Binding or serving the local socket failed.
    #[error("test daemon failed: {0}")]
    Server(#[from] ServerError),
    /// The server task panicked or was cancelled.
    #[error("test daemon task failed: {0}")]
    ServerTask(String),
    /// Dialing the local socket failed.
    #[error("cannot connect to the test daemon: {0}")]
    Connect(#[source] tonic::transport::Error),
    /// The peer dropped the stream before the expected event.
    #[error("the converse stream closed")]
    Closed,
    /// No matching event arrived before the deadline.
    #[error("the converse stream produced no matching event before the deadline")]
    Timeout,
    /// The server closed the call with a status.
    #[error("converse rpc failed: {code}: {message}")]
    Rpc {
        /// tonic status code name.
        code: String,
        /// Server message, already capped by tonic.
        message: String,
    },
    /// This build has no owner-only local socket (Windows uses a named pipe
    /// the test daemon does not bind yet).
    #[error("the test daemon binds the owner-only Unix socket, which this platform does not have")]
    LocalSocket,
}
