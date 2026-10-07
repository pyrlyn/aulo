//! Authentication over real listeners: TCP needs the token, the local socket
//! admits its owner.
// Helpers outside #[test] fns are not covered by allow-unwrap-in-tests.
#![allow(clippy::unwrap_used)]

use aulo_server::auth::{ApiToken, MemoryTokenStore, TokenStore};
use aulo_server::{ApiServer, CancellationToken, Limits, Listen, ServerError, TcpListen, bind};
use tokio::task::JoinHandle;
use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, Endpoint};
use tonic::{Code, Request, Status};
use tonic_health::pb::HealthCheckRequest;
use tonic_health::pb::health_client::HealthClient;

struct Tcp {
    channel: Channel,
    shutdown: CancellationToken,
    task: JoinHandle<Result<(), ServerError>>,
}

impl Tcp {
    async fn stop(self) {
        self.shutdown.cancel();
        self.task.await.unwrap().unwrap();
    }
}

async fn serve_tcp(token: &ApiToken) -> Tcp {
    let bound = bind(&Listen::Tcp(TcpListen {
        addr: "127.0.0.1:0".parse().unwrap(),
        tls: None,
    }))
    .await
    .unwrap();
    let addr = bound.tcp_addr().unwrap();
    let shutdown = CancellationToken::new();
    let server = ApiServer::new(Limits::default()).unwrap().with_token(token);
    let task = tokio::spawn(server.serve(vec![bound], shutdown.clone()));
    let channel = Endpoint::from_shared(format!("http://{addr}"))
        .unwrap()
        .connect()
        .await
        .unwrap();
    Tcp {
        channel,
        shutdown,
        task,
    }
}

async fn check(channel: Channel, authorization: Option<String>) -> Result<(), Status> {
    let mut health = HealthClient::with_interceptor(channel, move |mut request: Request<()>| {
        if let Some(value) = &authorization {
            let value = MetadataValue::try_from(value.as_str()).unwrap();
            request.metadata_mut().insert("authorization", value);
        }
        Ok(request)
    });
    health.check(HealthCheckRequest::default()).await.map(drop)
}

fn token() -> ApiToken {
    MemoryTokenStore::default().load_or_create().unwrap()
}

#[tokio::test]
async fn tcp_without_a_token_is_rejected() {
    let server = serve_tcp(&token()).await;
    let status = check(server.channel.clone(), None).await.unwrap_err();
    assert_eq!(status.code(), Code::Unauthenticated, "{status:?}");
    server.stop().await;
}

#[tokio::test]
async fn tcp_with_a_wrong_token_is_rejected() {
    let server = serve_tcp(&token()).await;
    let wrong = format!("Bearer {}", token().reveal());
    let status = check(server.channel.clone(), Some(wrong))
        .await
        .unwrap_err();
    assert_eq!(status.code(), Code::Unauthenticated, "{status:?}");
    // The refusal says nothing about the expected token.
    assert_eq!(status.message(), "authentication required");
    server.stop().await;
}

#[tokio::test]
async fn tcp_with_the_right_token_is_accepted() {
    let token = token();
    let server = serve_tcp(&token).await;
    let right = format!("Bearer {}", token.reveal());
    check(server.channel.clone(), Some(right)).await.unwrap();
    server.stop().await;
}

#[tokio::test]
async fn tcp_without_an_installed_token_is_not_served() {
    let bound = bind(&Listen::Tcp(TcpListen {
        addr: "127.0.0.1:0".parse().unwrap(),
        tls: None,
    }))
    .await
    .unwrap();
    let err = ApiServer::new(Limits::default())
        .unwrap()
        .serve(vec![bound], CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(err, ServerError::TcpWithoutToken), "{err}");
}

#[cfg(unix)]
#[tokio::test]
async fn unix_socket_from_the_owner_is_accepted_without_a_token() {
    use hyper_util::rt::TokioIo;
    use tokio::net::UnixStream;
    use tower::service_fn;

    let home = tempfile::tempdir().unwrap();
    let socket = aulo_server::local_socket_path(home.path());
    let bound = bind(&Listen::Unix(socket.clone())).await.unwrap();
    let shutdown = CancellationToken::new();
    // A token installed for TCP must not be demanded on the local socket.
    let server = ApiServer::new(Limits::default())
        .unwrap()
        .with_token(&token());
    let task = tokio::spawn(server.serve(vec![bound], shutdown.clone()));
    // The URI is required by the builder but ignored: the connector dials the socket.
    let channel = Endpoint::from_static("http://[::]:50051")
        .connect_with_connector(service_fn(move |_| {
            let socket = socket.clone();
            async move { Ok::<_, std::io::Error>(TokioIo::new(UnixStream::connect(socket).await?)) }
        }))
        .await
        .unwrap();
    check(channel, None).await.unwrap();
    shutdown.cancel();
    task.await.unwrap().unwrap();
}
