//! TLS on the TCP listener: pinned clients, certificate loading, and the
//! non-loopback gate.
// Helpers outside #[test] fns are not covered by allow-unwrap-in-tests.
#![allow(clippy::unwrap_used)]

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use aulo_server::auth::ApiToken;
use aulo_server::tls::{CertFingerprint, TlsIdentity, pinned_client_config};
use aulo_server::{ApiServer, CancellationToken, Limits, Listen, ServerError, TcpListen, bind};
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tokio_rustls::TlsConnector;
use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Status};
use tonic_health::pb::HealthCheckRequest;
use tonic_health::pb::health_client::HealthClient;
use tower::service_fn;

struct Served {
    addr: SocketAddr,
    token: ApiToken,
    shutdown: CancellationToken,
    task: JoinHandle<Result<(), ServerError>>,
}

impl Served {
    async fn stop(self) {
        self.shutdown.cancel();
        self.task.await.unwrap().unwrap();
    }
}

async fn serve_tls(identity: TlsIdentity) -> Served {
    let bound = bind(&Listen::Tcp(TcpListen {
        addr: "127.0.0.1:0".parse().unwrap(),
        tls: Some(identity),
    }))
    .await
    .unwrap();
    let addr = bound.tcp_addr().unwrap();
    let token = ApiToken::generate().unwrap();
    let shutdown = CancellationToken::new();
    let server = ApiServer::new(Limits::default())
        .unwrap()
        .with_token(&token);
    let task = tokio::spawn(server.serve(vec![bound], shutdown.clone()));
    Served {
        addr,
        token,
        shutdown,
        task,
    }
}

/// What `aulo connect --pin` will do: TLS with the pinned verifier under a
/// plain tonic channel.
async fn connect_pinned(
    addr: SocketAddr,
    pin: CertFingerprint,
) -> Result<Channel, tonic::transport::Error> {
    let connector = TlsConnector::from(Arc::new(pinned_client_config(pin).unwrap()));
    Endpoint::from_shared(format!("http://{addr}"))
        .unwrap()
        .connect_with_connector(service_fn(move |_| {
            let connector = connector.clone();
            async move {
                let tcp = TcpStream::connect(addr).await?;
                // The verifier ignores the name; rustls still needs one for SNI.
                let name = ServerName::try_from("localhost").unwrap();
                let tls = connector.connect(name, tcp).await?;
                Ok::<_, std::io::Error>(TokioIo::new(tls))
            }
        }))
        .await
}

async fn check(channel: Channel, token: &ApiToken) -> Result<(), Status> {
    let bearer: MetadataValue<_> = format!("Bearer {}", token.reveal()).parse().unwrap();
    let mut health = HealthClient::with_interceptor(channel, move |mut request: Request<()>| {
        request
            .metadata_mut()
            .insert("authorization", bearer.clone());
        Ok(request)
    });
    health.check(HealthCheckRequest::default()).await.map(drop)
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

#[tokio::test]
async fn the_right_pin_connects() {
    let home = tempfile::tempdir().unwrap();
    let identity = TlsIdentity::self_signed(&home.path().join("tls")).unwrap();
    let pin = identity.fingerprint();
    let server = serve_tls(identity).await;
    let channel = connect_pinned(server.addr, pin).await.unwrap();
    check(channel, &server.token).await.unwrap();
    server.stop().await;
}

#[tokio::test]
async fn a_wrong_pin_is_refused() {
    let home = tempfile::tempdir().unwrap();
    let identity = TlsIdentity::self_signed(&home.path().join("tls")).unwrap();
    let wrong = CertFingerprint::of(b"some other certificate");
    let server = serve_tls(identity).await;
    let err = connect_pinned(server.addr, wrong).await.unwrap_err();
    // The refusal comes from the verifier, not from a timeout or a reset.
    let chain = format!("{:?}", std::error::Error::source(&err));
    assert!(chain.contains("ApplicationVerificationFailure"), "{chain}");
    server.stop().await;
}

#[tokio::test]
async fn a_plaintext_client_is_not_served_by_a_tls_listener() {
    let home = tempfile::tempdir().unwrap();
    let identity = TlsIdentity::self_signed(&home.path().join("tls")).unwrap();
    let server = serve_tls(identity).await;
    let plain = Endpoint::from_shared(format!("http://{}", server.addr))
        .unwrap()
        .connect_lazy();
    check(plain, &server.token).await.unwrap_err();
    server.stop().await;
}

#[tokio::test]
async fn tls_still_requires_the_token() {
    let home = tempfile::tempdir().unwrap();
    let identity = TlsIdentity::self_signed(&home.path().join("tls")).unwrap();
    let pin = identity.fingerprint();
    let server = serve_tls(identity).await;
    let channel = connect_pinned(server.addr, pin).await.unwrap();
    let status = check(channel, &ApiToken::generate().unwrap())
        .await
        .unwrap_err();
    assert_eq!(status.code(), tonic::Code::Unauthenticated, "{status:?}");
    server.stop().await;
}

#[tokio::test]
async fn non_loopback_binds_with_tls_but_is_not_served_without_a_token() {
    let home = tempfile::tempdir().unwrap();
    let identity = TlsIdentity::self_signed(&home.path().join("tls")).unwrap();
    let bound = bind(&Listen::Tcp(TcpListen {
        addr: "0.0.0.0:0".parse().unwrap(),
        tls: Some(identity),
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

#[test]
fn the_self_signed_identity_is_kept_across_starts() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("tls");
    let first = TlsIdentity::self_signed(&dir).unwrap();
    let second = TlsIdentity::self_signed(&dir).unwrap();
    assert_eq!(first.fingerprint(), second.fingerprint());
    // The stored files load as a user-provided pair too.
    let loaded = TlsIdentity::from_pem_files(&dir.join("cert.pem"), &dir.join("key.pem")).unwrap();
    assert_eq!(loaded.fingerprint(), first.fingerprint());
}

#[cfg(unix)]
#[test]
fn the_self_signed_files_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("tls");
    TlsIdentity::self_signed(&dir).unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&dir.join("cert.pem")), 0o600);
    assert_eq!(mode(&dir.join("key.pem")), 0o600);

    let key = dir.join("key.pem");
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o640)).unwrap();
    let err = TlsIdentity::self_signed(&dir).unwrap_err();
    assert!(matches!(err, ServerError::InsecureFile(_)), "{err}");
}

#[test]
fn a_key_from_another_certificate_is_an_error() {
    let home = tempfile::tempdir().unwrap();
    TlsIdentity::self_signed(&home.path().join("a")).unwrap();
    TlsIdentity::self_signed(&home.path().join("b")).unwrap();
    let err = TlsIdentity::from_pem_files(
        &home.path().join("a/cert.pem"),
        &home.path().join("b/key.pem"),
    )
    .unwrap_err();
    assert!(matches!(err, ServerError::TlsIdentity(_)), "{err}");
}

#[test]
fn files_without_a_certificate_or_key_are_errors() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("tls");
    TlsIdentity::self_signed(&dir).unwrap();
    let cert = dir.join("cert.pem");
    let key = dir.join("key.pem");
    let junk = write(home.path(), "junk.pem", b"not a pem file\n");

    // Each file swapped for the other, or for junk.
    let err = TlsIdentity::from_pem_files(&key, &key).unwrap_err();
    assert!(matches!(err, ServerError::NoCertificate(_)), "{err}");
    let err = TlsIdentity::from_pem_files(&junk, &key).unwrap_err();
    assert!(matches!(err, ServerError::NoCertificate(_)), "{err}");
    let err = TlsIdentity::from_pem_files(&cert, &cert).unwrap_err();
    assert!(matches!(err, ServerError::Pem { .. }), "{err}");
    let err = TlsIdentity::from_pem_files(&cert, &home.path().join("missing.pem")).unwrap_err();
    assert!(matches!(err, ServerError::Io { .. }), "{err}");
}

#[test]
fn a_certificate_without_its_key_is_not_regenerated_over() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("tls");
    TlsIdentity::self_signed(&dir).unwrap();
    std::fs::remove_file(dir.join("key.pem")).unwrap();
    // Regenerating would silently change the pin every client holds.
    let err = TlsIdentity::self_signed(&dir).unwrap_err();
    assert!(matches!(err, ServerError::Io { .. }), "{err}");
    assert!(dir.join("cert.pem").exists());
}
