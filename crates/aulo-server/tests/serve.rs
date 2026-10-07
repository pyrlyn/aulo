//! End-to-end checks of the assembled server over a real Unix socket: health,
//! reflection, the message-size cap, shutdown, and the socket safety rules.
#![cfg(unix)]
// Helpers outside #[test] fns are not covered by allow-unwrap-in-tests.
#![allow(clippy::unwrap_used)]

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use aulo_server::{
    ApiServer, CancellationToken, Limits, Listen, ServerError, TcpListen, bind, local_socket_path,
};
use common::{connect, start};
use tonic::Code;
use tonic::transport::Endpoint;
use tonic_health::pb::HealthCheckRequest;
use tonic_health::pb::health_check_response::ServingStatus;
use tonic_health::pb::health_client::HealthClient;
use tonic_reflection::pb::v1::ServerReflectionRequest;
use tonic_reflection::pb::v1::server_reflection_client::ServerReflectionClient;
use tonic_reflection::pb::v1::server_reflection_request::MessageRequest;
use tonic_reflection::pb::v1::server_reflection_response::MessageResponse;

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[tokio::test]
async fn health_check_answers_over_the_unix_socket() {
    let server = start(Limits::default()).await;
    let mut health = HealthClient::new(connect(&server.socket).await);
    let response = health
        .check(HealthCheckRequest {
            service: String::new(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(response.status(), ServingStatus::Serving);
    assert_eq!(mode(&server.socket), 0o600);
    assert_eq!(mode(server.socket.parent().unwrap()), 0o700);
}

#[tokio::test]
async fn reflection_lists_the_aulo_services() {
    let server = start(Limits::default()).await;
    let mut reflection = ServerReflectionClient::new(connect(&server.socket).await);
    let request = ServerReflectionRequest {
        host: String::new(),
        message_request: Some(MessageRequest::ListServices(String::new())),
    };
    let mut responses = reflection
        .server_reflection_info(tokio_stream::iter([request]))
        .await
        .unwrap()
        .into_inner();
    let Some(MessageResponse::ListServicesResponse(list)) =
        responses.message().await.unwrap().unwrap().message_response
    else {
        panic!("expected a service list");
    };
    let names: Vec<_> = list.service.into_iter().map(|s| s.name).collect();
    for expected in [
        "aulo.v1.ChatService",
        "aulo.v1.SessionService",
        "grpc.health.v1.Health",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "{expected} missing from {names:?}"
        );
    }
}

#[tokio::test]
async fn oversize_request_is_rejected_before_the_handler() {
    let limits = Limits {
        max_decoding_message_size: 1024,
        ..Limits::default()
    };
    let server = start(limits).await;
    let mut health = HealthClient::new(connect(&server.socket).await);
    let status = health
        .check(HealthCheckRequest {
            service: "x".repeat(4096),
        })
        .await
        .unwrap_err();
    assert_eq!(status.code(), Code::OutOfRange, "{status:?}");
}

#[tokio::test]
async fn shutdown_completes_and_removes_the_socket() {
    let server = start(Limits::default()).await;
    // An idle client must not hold shutdown open.
    let _client = HealthClient::new(connect(&server.socket).await);
    server.shutdown.cancel();
    tokio::time::timeout(Duration::from_secs(5), server.task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!server.socket.exists());
}

#[tokio::test]
async fn shutdown_abandons_streams_after_the_grace() {
    let limits = Limits {
        shutdown_grace: Duration::from_millis(200),
        ..Limits::default()
    };
    let server = start(limits).await;
    let mut health = HealthClient::new(connect(&server.socket).await);
    // Watch never ends on its own, so only the grace can end shutdown.
    let _watch = health
        .watch(HealthCheckRequest {
            service: String::new(),
        })
        .await
        .unwrap();
    let started = std::time::Instant::now();
    server.shutdown.cancel();
    tokio::time::timeout(Duration::from_secs(5), server.task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(started.elapsed() >= Duration::from_millis(200));
}

#[tokio::test]
async fn a_live_socket_is_not_clobbered() {
    let server = start(Limits::default()).await;
    let err = bind(&Listen::Unix(server.socket.clone()))
        .await
        .unwrap_err();
    assert!(matches!(err, ServerError::SocketInUse(_)), "{err}");
    let mut health = HealthClient::new(connect(&server.socket).await);
    health.check(HealthCheckRequest::default()).await.unwrap();
}

#[tokio::test]
async fn a_stale_socket_is_replaced() {
    let home = tempfile::tempdir().unwrap();
    let socket = local_socket_path(home.path());
    drop(bind(&Listen::Unix(socket.clone())).await.unwrap());
    // Leave a socket file nobody listens on, as a crashed server would.
    drop(std::os::unix::net::UnixListener::bind(&socket).unwrap());
    assert!(socket.exists());
    bind(&Listen::Unix(socket)).await.unwrap();
}

#[tokio::test]
async fn a_regular_file_is_not_replaced() {
    let home = tempfile::tempdir().unwrap();
    let socket = local_socket_path(home.path());
    std::fs::create_dir(socket.parent().unwrap()).unwrap();
    std::fs::set_permissions(
        socket.parent().unwrap(),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    std::fs::write(&socket, "keep me").unwrap();
    let err = bind(&Listen::Unix(socket.clone())).await.unwrap_err();
    assert!(matches!(err, ServerError::NotASocket(_)), "{err}");
    assert_eq!(std::fs::read_to_string(&socket).unwrap(), "keep me");
}

#[tokio::test]
async fn a_directory_other_users_can_enter_is_refused() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("run");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let err = bind(&Listen::Unix(dir.join("aulod.sock")))
        .await
        .unwrap_err();
    assert!(matches!(err, ServerError::InsecureDirectory(_)), "{err}");
}

#[tokio::test]
async fn a_relative_socket_path_is_refused() {
    let err = bind(&Listen::Unix(PathBuf::from("aulod.sock")))
        .await
        .unwrap_err();
    assert!(matches!(err, ServerError::InvalidSocketPath(_)), "{err}");
}

#[tokio::test]
async fn tcp_refuses_non_loopback_without_auth() {
    for addr in ["0.0.0.0:0", "[::]:0"] {
        let listen = Listen::Tcp(TcpListen {
            addr: addr.parse().unwrap(),
            remote_auth_configured: false,
        });
        let err = bind(&listen).await.unwrap_err();
        assert!(
            matches!(err, ServerError::UnauthenticatedRemote(_)),
            "{err}"
        );
    }
}

#[tokio::test]
async fn tcp_on_loopback_serves_health() {
    let bound = bind(&Listen::Tcp(TcpListen {
        addr: "127.0.0.1:0".parse().unwrap(),
        remote_auth_configured: false,
    }))
    .await
    .unwrap();
    let addr = bound.tcp_addr().unwrap();
    let shutdown = CancellationToken::new();
    let task = tokio::spawn(
        ApiServer::new(Limits::default())
            .unwrap()
            .serve(vec![bound], shutdown.clone()),
    );
    let channel = Endpoint::from_shared(format!("http://{addr}"))
        .unwrap()
        .connect()
        .await
        .unwrap();
    let response = HealthClient::new(channel)
        .check(HealthCheckRequest::default())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(response.status(), ServingStatus::Serving);
    shutdown.cancel();
    task.await.unwrap().unwrap();
}
