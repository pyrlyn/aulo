//! Client secret minting against a local HTTP server: the request OpenAI
//! documents, the reply it documents, and a hostile or failing server.

#![allow(clippy::unwrap_used)]

use std::time::Duration;

use aulo_realtime::{ApiKey, ClientSecretMinter, ClientSecretRequest, RealtimeError};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const KEY: &str = "sk-test-secret-key";

fn request(ttl: u64) -> ClientSecretRequest {
    ClientSecretRequest {
        model: "gpt-realtime-2.1".into(),
        voice: Some("marin".into()),
        instructions: Some("Be brief.".into()),
        ttl: Duration::from_secs(ttl),
    }
}

fn minter(server: &MockServer) -> ClientSecretMinter {
    ClientSecretMinter::new(
        ApiKey::new(KEY),
        &format!("{}/v1", server.uri()),
        Duration::from_secs(5),
    )
    .unwrap()
}

#[tokio::test]
async fn mints_a_secret_with_the_documented_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/realtime/client_secrets"))
        .and(header("authorization", "Bearer sk-test-secret-key"))
        .and(body_json(json!({
            "expires_after": {"anchor": "created_at", "seconds": 120},
            "session": {"type": "realtime", "model": "gpt-realtime-2.1",
                        "audio": {"output": {"voice": "marin"}},
                        "instructions": "Be brief."},
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": "ek_1234", "expires_at": 1_790_000_000u64,
            "session": {"type": "realtime", "id": "sess_1", "object": "realtime.session"},
        })))
        .expect(1)
        .mount(&server)
        .await;
    let secret = minter(&server).mint(&request(120)).await.unwrap();
    assert_eq!(secret.expose(), "ek_1234");
    assert_eq!(secret.expires_at, 1_790_000_000);
    // The token is meant for the remote client only: it stays out of logs.
    assert!(!format!("{secret:?}").contains("ek_1234"));
}

#[tokio::test]
async fn failures_are_reported_by_status_without_the_body() {
    for (status, expected) in [
        (401, RealtimeError::Rejected(401)),
        (429, RealtimeError::RateLimited),
        (503, RealtimeError::Refused(503)),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(status).set_body_string(format!("bad key {KEY}")))
            .mount(&server)
            .await;
        let error = minter(&server).mint(&request(60)).await.unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error} {error:?}").contains(KEY));
    }
}

#[tokio::test]
async fn a_hostile_reply_is_refused() {
    let big = "x".repeat(100 * 1024);
    for body in [
        "not json".to_owned(),
        json!({"expires_at": 1}).to_string(),
        json!({"value": "ek 1\n", "expires_at": 1}).to_string(),
        json!({"value": "ek_1"}).to_string(),
        json!({"value": "ek_1", "expires_at": 1, "pad": big}).to_string(),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
        let error = minter(&server).mint(&request(60)).await.unwrap_err();
        assert!(matches!(error, RealtimeError::Protocol(_)), "{error:?}");
    }
}

#[tokio::test]
async fn a_redirect_is_not_followed_with_the_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(307).insert_header("location", "http://127.0.0.1:1/x"))
        .mount(&server)
        .await;
    assert_eq!(
        minter(&server).mint(&request(60)).await.unwrap_err(),
        RealtimeError::Refused(307)
    );
}

#[tokio::test]
async fn a_slow_server_times_out() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .mount(&server)
        .await;
    let minter = ClientSecretMinter::new(
        ApiKey::new(KEY),
        &format!("{}/v1", server.uri()),
        Duration::from_millis(200),
    )
    .unwrap();
    assert_eq!(
        minter.mint(&request(60)).await.unwrap_err(),
        RealtimeError::TimedOut
    );
}

#[tokio::test]
async fn bad_requests_and_endpoints_are_refused_before_sending() {
    let server = MockServer::start().await;
    let minter = minter(&server);
    for ttl in [9, 7201] {
        assert!(matches!(
            minter.mint(&request(ttl)).await,
            Err(RealtimeError::Invalid { .. })
        ));
    }
    // A key must not travel over plain HTTP to another host.
    assert!(matches!(
        ClientSecretMinter::new(
            ApiKey::new(KEY),
            "http://example.com/v1",
            Duration::from_secs(5)
        ),
        Err(RealtimeError::Invalid { .. })
    ));
    assert!(server.received_requests().await.unwrap().is_empty());
}
