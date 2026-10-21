//! Each local server kind, against a fixture HTTP server. Nothing here talks
//! to a real model server.

// Helpers sit outside `#[test]`, where clippy does not exempt unwrap.
#![allow(clippy::unwrap_used)]

use aulo_providers::{
    DetectedServer, LoadState, MAX_BODY_BYTES, MAX_MODELS, ProbeError, ServerKind, probe_client,
    probe_origin, probe_origins,
};
use serde_json::{Value, json};
use wiremock::matchers::{header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn mount(server: &MockServer, route: &str, body: Value) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

async fn probe(server: &MockServer) -> Result<DetectedServer, ProbeError> {
    probe_origin(&probe_client().unwrap(), &server.uri()).await
}

fn openai(rows: Vec<Value>) -> Value {
    json!({"object": "list", "data": rows})
}

fn model(id: &str, owned_by: &str) -> Value {
    json!({"id": id, "object": "model", "owned_by": owned_by})
}

#[tokio::test]
async fn runa_is_the_server_whose_models_are_owned_by_runa() {
    let server = MockServer::start().await;
    // A key must never be attached: local probes are unauthenticated, and a
    // header would put a secret on the wire.
    Mock::given(method("GET"))
        .and(header_exists("authorization"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    mount(
        &server,
        "/v1/models",
        openai(vec![
            model("aulo-qwen", "runa"),
            model("aulo-qwen", "runa"),
            model("bad\nid", "runa"),
        ]),
    )
    .await;

    let found = probe(&server).await.unwrap();
    assert_eq!(found.kind, ServerKind::Runa);
    assert_eq!(found.kind.provider_kind().as_str(), "runa");
    assert_eq!(found.base_url, format!("{}/v1", server.uri()));
    assert_eq!(found.models.len(), 1);
    assert_eq!(found.models[0].id, "aulo-qwen");
    assert_eq!(found.models[0].state, None);
    assert!(!format!("{found:?}").contains("bad"));
}

#[tokio::test]
async fn an_openai_base_ending_in_v1_probes_the_origin() {
    let server = MockServer::start().await;
    mount(&server, "/v1/models", openai(vec![model("only", "runa")])).await;
    let client = probe_client().unwrap();
    let found = probe_origin(&client, &format!("{}/v1", server.uri()))
        .await
        .unwrap();
    assert_eq!(found.base_url, format!("{}/v1", server.uri()));
    assert_eq!(found.models[0].id, "only");
}

#[tokio::test]
async fn ollama_is_the_version_endpoint_plus_the_model_list() {
    let server = MockServer::start().await;
    mount(&server, "/api/version", json!({"version": "0.11.4"})).await;
    // Current Ollama copies the model namespace into owned_by, which is empty
    // for a bare name, so the owner string is not the fingerprint.
    mount(
        &server,
        "/v1/models",
        openai(vec![
            model("llama3.2:latest", ""),
            model("library/nomic", "library"),
        ]),
    )
    .await;

    let found = probe(&server).await.unwrap();
    assert_eq!(found.kind, ServerKind::Ollama);
    assert_eq!(
        found
            .models
            .iter()
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>(),
        vec!["llama3.2:latest", "library/nomic"]
    );
}

#[tokio::test]
async fn an_older_ollama_that_sets_owned_by_ollama_still_matches() {
    let server = MockServer::start().await;
    mount(
        &server,
        "/v1/models",
        openai(vec![model("llama3:latest", "ollama")]),
    )
    .await;
    assert_eq!(probe(&server).await.unwrap().kind, ServerKind::Ollama);
}

#[tokio::test]
async fn a_version_document_alone_is_not_ollama() {
    let server = MockServer::start().await;
    mount(&server, "/api/version", json!({"version": "0.11.4"})).await;
    assert_eq!(probe(&server).await.unwrap_err(), ProbeError::Unrecognized);
}

#[tokio::test]
async fn lm_studio_uses_the_native_model_list() {
    let server = MockServer::start().await;
    mount(
        &server,
        "/v1/models",
        openai(vec![model("openai-shaped", "organization")]),
    )
    .await;
    mount(
        &server,
        "/api/v0/models",
        json!({"object": "list", "data": [
            {
                "id": "meta-llama-3.1-8b-instruct",
                "object": "model",
                "type": "llm",
                "publisher": "lmstudio-community",
                "quantization": "Q4_K_M",
                "state": "not-loaded",
                "max_context_length": 131072
            },
            {
                "id": "already-up",
                "object": "model",
                "type": "llm",
                "state": "loaded",
                "quantization": "Q4_0",
                "max_context_length": 4096
            }
        ]}),
    )
    .await;

    let found = probe(&server).await.unwrap();
    assert_eq!(found.kind, ServerKind::LmStudio);
    assert_eq!(found.models[0].id, "meta-llama-3.1-8b-instruct");
    assert_eq!(found.models[0].state, Some(LoadState::NotLoaded));
    assert_eq!(found.models[1].state, Some(LoadState::Loaded));
    assert!(!found.models.iter().any(|m| m.id == "openai-shaped"));
}

#[tokio::test]
async fn lm_studio_v1_keys_are_a_native_list_too() {
    let server = MockServer::start().await;
    mount(
        &server,
        "/api/v1/models",
        json!({"models": [{
            "type": "llm",
            "key": "qwen/qwen3-4b",
            "display_name": "Qwen3",
            "quantization": {"name": "Q4_K_M", "bits_per_weight": 4.5},
            "loaded_instances": [{"id": "instance-1"}],
            "max_context_length": 4096
        }]}),
    )
    .await;

    let found = probe(&server).await.unwrap();
    assert_eq!(found.kind, ServerKind::LmStudio);
    assert_eq!(found.models[0].id, "qwen/qwen3-4b");
    assert_eq!(found.models[0].state, Some(LoadState::Loaded));
}

#[tokio::test]
async fn llama_cpp_is_owned_by_llamacpp() {
    let server = MockServer::start().await;
    mount(
        &server,
        "/v1/models",
        openai(vec![json!({
            "id": "Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf",
            "object": "model",
            "created": 1_735_142_223_u64,
            "owned_by": "llamacpp",
            "meta": {"n_vocab": 128_256, "n_ctx_train": 131_072}
        })]),
    )
    .await;

    let found = probe(&server).await.unwrap();
    assert_eq!(found.kind, ServerKind::LlamaCpp);
    assert_eq!(found.models[0].id, "Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf");
}

#[tokio::test]
async fn a_generic_openai_list_is_not_a_local_preset() {
    let server = MockServer::start().await;
    mount(
        &server,
        "/v1/models",
        openai(vec![model("gpt-4o", "openai")]),
    )
    .await;
    assert_eq!(probe(&server).await.unwrap_err(), ProbeError::Unrecognized);
}

#[tokio::test]
async fn a_redirect_is_not_followed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", format!("{}/secret", server.uri())),
        )
        .mount(&server)
        .await;
    mount(&server, "/secret", openai(vec![model("hidden", "runa")])).await;

    let err = probe(&server).await.unwrap_err();
    assert_eq!(err, ProbeError::Unrecognized);
    assert!(!err.to_string().contains("hidden"));
    let paths: Vec<_> = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|request| request.url.path().to_owned())
        .collect();
    assert!(paths.contains(&"/v1/models".to_owned()));
    assert!(!paths.iter().any(|path| path == "/secret"));
}

#[tokio::test]
async fn an_oversized_list_is_refused_without_the_body() {
    let server = MockServer::start().await;
    let mut body = vec![b' '; MAX_BODY_BYTES + 1];
    body.extend_from_slice(b"s3cr3t-in-the-body");
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(&server)
        .await;

    let err = probe(&server).await.unwrap_err();
    assert_eq!(err, ProbeError::TooLarge);
    assert!(!err.to_string().contains("s3cr3t"));
}

#[tokio::test]
async fn the_model_list_is_capped() {
    let server = MockServer::start().await;
    let rows = (0..MAX_MODELS + 40)
        .map(|n| model(&format!("model-{n}"), "runa"))
        .collect();
    mount(&server, "/v1/models", openai(rows)).await;

    let found = probe(&server).await.unwrap();
    assert_eq!(found.models.len(), MAX_MODELS);
    assert_eq!(found.models[0].id, "model-0");
    assert_eq!(
        found.models[MAX_MODELS - 1].id,
        format!("model-{}", MAX_MODELS - 1)
    );
}

#[tokio::test]
async fn a_down_origin_is_skipped_and_a_live_one_is_kept() {
    let server = MockServer::start().await;
    mount(&server, "/v1/models", openai(vec![model("up", "runa")])).await;
    let client = probe_client().unwrap();
    let found = probe_origins(&client, &[&server.uri(), "http://127.0.0.1:1"]).await;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].models[0].id, "up");
}

#[tokio::test]
async fn a_credential_in_the_origin_is_refused_and_not_echoed() {
    let err = probe_origin(&probe_client().unwrap(), "http://user:s3cr3t@127.0.0.1:9")
        .await
        .unwrap_err();
    assert_eq!(err, ProbeError::BadOrigin);
    assert!(!err.to_string().contains("s3cr3t"));
    assert!(!format!("{err:?}").contains("s3cr3t"));
}
