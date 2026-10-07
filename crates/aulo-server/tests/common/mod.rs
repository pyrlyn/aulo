//! Helpers shared by the integration tests: a server on a real Unix socket in a
//! temporary home, and a client channel that dials it.
#![cfg(unix)]
// Helpers outside #[test] fns are not covered by allow-unwrap-in-tests.
#![allow(clippy::unwrap_used)]
// Each test binary uses a different subset.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use aulo_server::{
    ApiServer, CancellationToken, Limits, Listen, ServerError, bind, local_socket_path,
};
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tokio::task::JoinHandle;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

pub struct Running {
    pub _home: tempfile::TempDir,
    pub socket: PathBuf,
    pub shutdown: CancellationToken,
    pub task: JoinHandle<Result<(), ServerError>>,
}

pub async fn start(limits: Limits) -> Running {
    start_with(limits, |server| server).await
}

/// Starts a server after `mount` has added the services under test.
pub async fn start_with(limits: Limits, mount: impl FnOnce(ApiServer) -> ApiServer) -> Running {
    let home = tempfile::tempdir().unwrap();
    let socket = local_socket_path(home.path());
    let bound = bind(&Listen::Unix(socket.clone())).await.unwrap();
    let shutdown = CancellationToken::new();
    let server = mount(ApiServer::new(limits).unwrap());
    let task = tokio::spawn(server.serve(vec![bound], shutdown.clone()));
    Running {
        _home: home,
        socket,
        shutdown,
        task,
    }
}

// The URI is required by the builder but ignored: the connector dials the socket.
pub async fn connect(socket: &Path) -> Channel {
    let socket = socket.to_owned();
    Endpoint::from_static("http://[::]:50051")
        .connect_with_connector(service_fn(move |_| {
            let socket = socket.clone();
            async move { Ok::<_, std::io::Error>(TokioIo::new(UnixStream::connect(socket).await?)) }
        }))
        .await
        .unwrap()
}
