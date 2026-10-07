//! The downloader against a local HTTP server; nothing here touches the
//! network.

// The fixture helpers sit outside `#[test]` fns, where clippy does not
// exempt unwrap; a failing fixture should fail the test loudly anyway.
#![allow(clippy::unwrap_used)]

use std::path::Path;

use aulo_models::{Catalog, ModelError, ModelManager};
use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const MODEL_BIN: &[u8] = b"pretend these are onnx weights, long enough to be sent in pieces";
const TOKENS: &[u8] = b"<blk> 0\na 1\n";

/// Serves a body and honours `Range: bytes=N-`, like Hugging Face does.
struct Ranged(Vec<u8>);

impl Respond for Ranged {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let start = request
            .headers
            .get("range")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| {
                v.strip_prefix("bytes=")?
                    .strip_suffix('-')?
                    .parse::<usize>()
                    .ok()
            })
            .filter(|start| *start < self.0.len());
        match start {
            Some(start) => ResponseTemplate::new(206)
                .insert_header(
                    "content-range",
                    format!("bytes {start}-{}/{}", self.0.len() - 1, self.0.len()),
                )
                .set_body_bytes(self.0[start..].to_vec()),
            None => ResponseTemplate::new(200).set_body_bytes(self.0.clone()),
        }
    }
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A one-model catalog whose files are `model.bin` and `sub/tokens.txt`,
/// pinned to the real bytes above.
fn catalog(server: &MockServer) -> Catalog {
    let file = |path: &str, bytes: &[u8]| json!({"path": path, "size": bytes.len(), "sha256": sha256(bytes)});
    let json = json!({"models": [{
        "id": "tiny-stt",
        "kind": "stt",
        "name": "Tiny",
        "license": "MIT",
        "base_url": format!("{}/m/", server.uri()),
        "files": [file("model.bin", MODEL_BIN), file("sub/tokens.txt", TOKENS)],
    }]});
    Catalog::parse(&json.to_string()).unwrap()
}

async fn serve(server: &MockServer, file: &str, body: &[u8]) {
    Mock::given(method("GET"))
        .and(path(format!("/m/{file}")))
        .respond_with(Ranged(body.to_vec()))
        .mount(server)
        .await;
}

fn manager(home: &TempDir, server: &MockServer) -> ModelManager {
    ModelManager::new(home.path(), catalog(server)).unwrap()
}

fn leftovers(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_owned()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "part") {
                found.push(p.display().to_string());
            }
        }
    }
    found
}

#[tokio::test]
async fn pull_installs_verified_files_and_reports_progress() {
    let server = MockServer::start().await;
    serve(&server, "model.bin", MODEL_BIN).await;
    serve(&server, "sub/tokens.txt", TOKENS).await;
    let home = TempDir::new().unwrap();
    let models = manager(&home, &server);
    assert!(!models.list()[0].installed());

    let mut seen = Vec::new();
    let dir = models
        .pull("tiny-stt", |p| seen.push((p.bytes, p.total_bytes)))
        .await
        .unwrap();

    assert_eq!(dir, home.path().join("models/tiny-stt"));
    assert_eq!(std::fs::read(dir.join("model.bin")).unwrap(), MODEL_BIN);
    assert_eq!(std::fs::read(dir.join("sub/tokens.txt")).unwrap(), TOKENS);
    assert!(leftovers(home.path()).is_empty());
    let total = (MODEL_BIN.len() + TOKENS.len()) as u64;
    assert_eq!(seen.last(), Some(&(total, total)));
    assert!(
        seen.windows(2).all(|w| w[0].0 <= w[1].0),
        "progress went backwards: {seen:?}"
    );
    let status = &models.list()[0];
    assert!(status.installed());
    assert_eq!(models.path("tiny-stt"), Some(dir));
}

#[tokio::test]
async fn a_corrupted_download_is_rejected() {
    let server = MockServer::start().await;
    let mut corrupted = MODEL_BIN.to_vec();
    corrupted[3] ^= 0xff; // same length, so only the hash can catch it
    serve(&server, "model.bin", &corrupted).await;
    let home = TempDir::new().unwrap();
    let models = manager(&home, &server);

    let err = models.pull("tiny-stt", |_| {}).await.unwrap_err();

    assert!(
        matches!(err, ModelError::HashMismatch { ref file, .. } if file == "model.bin"),
        "{err}"
    );
    assert!(!home.path().join("models/tiny-stt/model.bin").exists());
    assert!(leftovers(home.path()).is_empty());
    assert!(models.path("tiny-stt").is_none());
}

#[tokio::test]
async fn a_body_over_the_pinned_size_is_cut_off_and_deleted() {
    let server = MockServer::start().await;
    let mut long = MODEL_BIN.to_vec();
    long.extend_from_slice(&[0; 4096]);
    serve(&server, "model.bin", &long).await;
    let home = TempDir::new().unwrap();

    let err = manager(&home, &server)
        .pull("tiny-stt", |_| {})
        .await
        .unwrap_err();

    assert!(
        matches!(err, ModelError::TooLarge { limit, .. } if limit == MODEL_BIN.len() as u64),
        "{err}"
    );
    assert!(leftovers(home.path()).is_empty());
}

#[tokio::test]
async fn an_http_error_installs_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let home = TempDir::new().unwrap();

    let err = manager(&home, &server)
        .pull("tiny-stt", |_| {})
        .await
        .unwrap_err();

    assert!(matches!(err, ModelError::Http { status: 404, .. }), "{err}");
}

#[tokio::test]
async fn an_interrupted_download_resumes_with_a_range_request() {
    let server = MockServer::start().await;
    serve(&server, "model.bin", MODEL_BIN).await;
    serve(&server, "sub/tokens.txt", TOKENS).await;
    let home = TempDir::new().unwrap();
    let dir = home.path().join("models/tiny-stt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("model.bin.part"), &MODEL_BIN[..10]).unwrap();

    manager(&home, &server)
        .pull("tiny-stt", |_| {})
        .await
        .unwrap();

    assert_eq!(std::fs::read(dir.join("model.bin")).unwrap(), MODEL_BIN);
    let requests = server.received_requests().await.unwrap();
    let ranged = requests
        .iter()
        .find(|r| r.url.path() == "/m/model.bin")
        .unwrap();
    assert_eq!(ranged.headers.get("range").unwrap(), "bytes=10-");
    assert!(leftovers(home.path()).is_empty());
}

#[tokio::test]
async fn a_server_that_ignores_the_range_restarts_the_file() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/m/model.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(MODEL_BIN))
        .mount(&server)
        .await;
    serve(&server, "sub/tokens.txt", TOKENS).await;
    let home = TempDir::new().unwrap();
    let dir = home.path().join("models/tiny-stt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("model.bin.part"), &MODEL_BIN[..10]).unwrap();

    manager(&home, &server)
        .pull("tiny-stt", |_| {})
        .await
        .unwrap();

    assert_eq!(std::fs::read(dir.join("model.bin")).unwrap(), MODEL_BIN);
}

#[tokio::test]
async fn a_corrupt_partial_fails_the_hash_and_is_deleted() {
    let server = MockServer::start().await;
    serve(&server, "model.bin", MODEL_BIN).await;
    let home = TempDir::new().unwrap();
    let dir = home.path().join("models/tiny-stt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("model.bin.part"), b"garbage!!!").unwrap();

    let err = manager(&home, &server)
        .pull("tiny-stt", |_| {})
        .await
        .unwrap_err();

    assert!(matches!(err, ModelError::HashMismatch { .. }), "{err}");
    assert!(leftovers(home.path()).is_empty());
}

#[tokio::test]
async fn installed_files_are_not_downloaded_again() {
    let server = MockServer::start().await;
    let home = TempDir::new().unwrap();
    let dir = home.path().join("models/tiny-stt");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("model.bin"), MODEL_BIN).unwrap();
    std::fs::write(dir.join("sub/tokens.txt"), TOKENS).unwrap();
    let models = manager(&home, &server);
    assert!(models.list()[0].installed());

    models.pull("tiny-stt", |_| {}).await.unwrap();

    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn remove_deletes_the_model_and_its_partials() {
    let server = MockServer::start().await;
    let home = TempDir::new().unwrap();
    let dir = home.path().join("models/tiny-stt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("model.bin.part"), b"half").unwrap();
    let models = manager(&home, &server);
    assert_eq!(models.list()[0].files_present, 0);

    assert!(models.remove("tiny-stt").unwrap());
    assert!(!dir.exists());
    assert!(
        !models.remove("tiny-stt").unwrap(),
        "nothing left to remove"
    );
}

#[tokio::test]
async fn unknown_and_unsafe_ids_are_refused() {
    let server = MockServer::start().await;
    let home = TempDir::new().unwrap();
    let models = manager(&home, &server);

    assert!(matches!(
        models.pull("nope", |_| {}).await,
        Err(ModelError::UnknownModel(_))
    ));
    for id in ["..", "../x", "a/b", "", "/etc"] {
        assert!(
            matches!(models.remove(id), Err(ModelError::UnsafeName(_))),
            "{id}"
        );
    }
}
