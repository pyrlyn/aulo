//! An in-process daemon on a temporary home.
//!
//! The socket path is the one `aulod` binds (`run/aulod.sock`), and the
//! services are the ones the daemon mounts today (`ConfigService`,
//! `ChatService`) plus anything the caller adds. The `aulo` binary is not
//! a dependency: the testkit role may not depend on a surface crate.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use aulo_config::{Config, LoadOptions, MemoryStore};
use aulo_proto::aulo::v1::chat_service_server::ChatServiceServer;
use aulo_server::{ApiServer, CancellationToken, ChatApi, ConfigApi, Limits, Listen, bind};
use aulo_store::Store;
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tokio::task::JoinHandle;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

use crate::error::TestkitError;

const CONFIG_TOML: &str = "\
[providers.endpoints.scripted]
kind = \"ollama\"
";

/// SQLite file under the temporary home. Tests that reopen the store use
/// this name.
pub const STORE_FILE: &str = "aulo.sqlite";

/// A daemon whose home, config and database disappear when it is dropped
/// after [`TestDaemon::shutdown`].
pub struct TestDaemon {
    home: tempfile::TempDir,
    socket: PathBuf,
    shutdown: CancellationToken,
    task: JoinHandle<Result<(), aulo_server::ServerError>>,
}

impl std::fmt::Debug for TestDaemon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestDaemon")
            .field("home", &self.home.path())
            .field("socket", &self.socket)
            .finish_non_exhaustive()
    }
}

impl TestDaemon {
    /// Starts with the built-in config: one `ollama` endpoint named
    /// `scripted`, and no extra services.
    pub async fn start() -> Result<Self, TestkitError> {
        Self::builder().start().await
    }

    /// Configures the home before [`TestDaemonBuilder::start`].
    #[must_use]
    pub fn builder() -> TestDaemonBuilder {
        TestDaemonBuilder {
            config_toml: CONFIG_TOML.to_owned(),
        }
    }

    /// The temporary `AULO_HOME`.
    #[must_use]
    pub fn home(&self) -> &Path {
        self.home.path()
    }

    /// `run/aulod.sock` under [`Self::home`].
    #[must_use]
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// The SQLite file [`ChatService`](aulo_proto::aulo::v1::chat_service_client::ChatServiceClient)
    /// is serving.
    #[must_use]
    pub fn store_path(&self) -> PathBuf {
        self.home.path().join(STORE_FILE)
    }

    /// A client channel dialing the local socket. The URI is unused; the
    /// connector opens the socket.
    pub async fn channel(&self) -> Result<Channel, TestkitError> {
        let socket = self.socket.clone();
        Endpoint::from_static("http://[::]:50051")
            .connect_with_connector(service_fn(move |_| {
                let socket = socket.clone();
                async move {
                    Ok::<_, std::io::Error>(TokioIo::new(UnixStream::connect(&socket).await?))
                }
            }))
            .await
            .map_err(TestkitError::Connect)
    }

    /// Cancels the server and waits for it to leave the socket.
    pub async fn shutdown(self) -> Result<(), TestkitError> {
        self.shutdown.cancel();
        match tokio::time::timeout(std::time::Duration::from_secs(3), self.task).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(error))) => Err(error.into()),
            Ok(Err(error)) => Err(TestkitError::ServerTask(error.to_string())),
            Err(_) => Err(TestkitError::Timeout),
        }
    }
}

/// What [`TestDaemon::start`] reads from the temporary home.
pub struct TestDaemonBuilder {
    config_toml: String,
}

impl std::fmt::Debug for TestDaemonBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestDaemonBuilder")
            .field("config_bytes", &self.config_toml.len())
            .finish()
    }
}

impl TestDaemonBuilder {
    /// Replaces the config file written into the home. An empty providers
    /// section is valid; a secret-named key is rejected by config loading.
    #[must_use]
    pub fn config_toml(mut self, toml: impl Into<String>) -> Self {
        self.config_toml = toml.into();
        self
    }

    /// Binds the socket, mounts config and chat, then `mount` for the
    /// services under test (a `SessionService`, once T4.8 has one).
    pub async fn start_with(
        self,
        mount: impl FnOnce(ApiServer) -> ApiServer,
    ) -> Result<TestDaemon, TestkitError> {
        let home = tempfile::tempdir().map_err(TestkitError::Home)?;
        fs::set_permissions(home.path(), fs::Permissions::from_mode(0o700))
            .map_err(TestkitError::Home)?;
        fs::write(home.path().join("config.toml"), self.config_toml).map_err(TestkitError::Home)?;
        let loaded = Config::load(&LoadOptions {
            home: Some(home.path().to_owned()),
            ..LoadOptions::default()
        })?;
        let store_path = home.path().join(STORE_FILE);
        let store_path = store_path.to_str().ok_or_else(|| {
            TestkitError::Home(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "test home is not valid UTF-8",
            ))
        })?;
        let store = Arc::new(Mutex::new(Store::open(store_path)?));
        let socket = aulo_server::local_socket_path(home.path());
        let bound = bind(&Listen::Unix(socket.clone())).await?;
        let limits = Limits {
            shutdown_grace: std::time::Duration::from_millis(200),
            ..Limits::default()
        };
        let server = ApiServer::new(limits)?
            .add_service(ConfigApi::new(loaded.config, Arc::new(MemoryStore::default())).service())
            .add_service(ChatServiceServer::new(ChatApi::new(store)));
        let server = mount(server);
        let shutdown = CancellationToken::new();
        let task = tokio::spawn(server.serve(vec![bound], shutdown.clone()));
        Ok(TestDaemon {
            home,
            socket,
            shutdown,
            task,
        })
    }

    /// [`Self::start_with`] with no extra service.
    pub async fn start(self) -> Result<TestDaemon, TestkitError> {
        self.start_with(|server| server).await
    }
}
