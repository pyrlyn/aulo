use tonic::metadata::MetadataValue;
use tonic::{Code, Request};

use super::*;

fn token() -> ApiToken {
    ApiToken::generate().unwrap()
}

fn with_authorization(values: &[&str]) -> Request<()> {
    let mut request = Request::new(());
    for value in values {
        request
            .metadata_mut()
            .append(AUTHORIZATION, MetadataValue::try_from(*value).unwrap());
    }
    request
}

fn assert_refused(result: Result<Request<()>, Status>) {
    let status = result.unwrap_err();
    assert_eq!(status.code(), Code::Unauthenticated);
    // The same message for every failure, so nothing about the cause leaks.
    assert_eq!(status.message(), "authentication required");
}

#[test]
fn the_right_token_is_admitted_and_stripped() {
    let token = token();
    let verifier = TokenVerifier::new(&token);
    let header = format!("Bearer {}", token.reveal());
    let request = authorize_bearer(with_authorization(&[&header]), &verifier).unwrap();
    assert_eq!(
        caller(&request).unwrap(),
        Caller::TokenHolder { remote: None }
    );
    assert!(request.metadata().get(AUTHORIZATION).is_none());
}

#[test]
fn the_bearer_scheme_is_case_insensitive() {
    let token = token();
    let verifier = TokenVerifier::new(&token);
    let header = format!("bEaReR {}", token.reveal());
    authorize_bearer(with_authorization(&[&header]), &verifier).unwrap();
}

#[test]
fn missing_malformed_wrong_and_duplicate_tokens_are_refused() {
    let token = token();
    let verifier = TokenVerifier::new(&token);
    let right = format!("Bearer {}", token.reveal());
    let wrong = format!("Bearer {}", self::token().reveal());
    let basic = format!("Basic {}", token.reveal());
    let bare = token.reveal().to_owned();
    let cases: [&[&str]; 7] = [
        &[],
        &["Bearer"],
        &["Bearer "],
        &[&bare],
        &[&basic],
        &[&wrong],
        &[&right, &right],
    ];
    for values in cases {
        assert_refused(authorize_bearer(with_authorization(values), &verifier));
    }
}

#[test]
fn a_request_without_a_caller_is_refused() {
    assert_eq!(
        caller(&Request::new(())).unwrap_err().code(),
        Code::Unauthenticated
    );
}

#[cfg(unix)]
#[test]
fn only_the_daemon_uid_is_the_owner() {
    let owner = daemon_uid();
    assert!(is_owner(Some(owner), owner));
    assert!(!is_owner(Some(owner.wrapping_add(1)), owner));
    assert!(!is_owner(None, owner));
}

#[test]
fn account_sids_match_only_when_every_byte_matches() {
    let user = AccountSid::from_bytes(&[1, 2, 3, 4]).unwrap();
    let copy = AccountSid::from_bytes(&[1, 2, 3, 4]).unwrap();
    let other = AccountSid::from_bytes(&[1, 2, 3, 5]).unwrap();
    assert!(same_account(&user, &copy));
    assert!(!same_account(&user, &other));
    assert!(AccountSid::from_bytes(&[]).is_none());
    assert!(AccountSid::from_bytes(&[0xAA; 69]).is_none());
    assert!(AccountSid::from_bytes(&[0x11; 68]).is_some());
}

#[test]
fn a_pipe_client_is_the_owner_only_for_the_same_unchanged_account() {
    let user = AccountSid::from_bytes(&[1, 1, 1, 1]).unwrap();
    let other = AccountSid::from_bytes(&[9, 9, 9, 9]).unwrap();
    assert_eq!(
        classify_pipe_client(Some(&user), Some(&user), true),
        PipeClient::Owner
    );
    assert_eq!(
        classify_pipe_client(Some(&other), Some(&user), true),
        PipeClient::Other
    );
    assert_eq!(
        classify_pipe_client(Some(&user), Some(&user), false),
        PipeClient::Unknown
    );
    assert_eq!(
        classify_pipe_client(None, Some(&user), true),
        PipeClient::Unknown
    );
    assert_eq!(
        classify_pipe_client(Some(&user), None, true),
        PipeClient::Unknown
    );
}

#[test]
fn a_named_pipe_request_is_admitted_only_with_owner_proof() {
    assert_refused(authorize_pipe(Request::new(())));
    let mut other = Request::new(());
    other.extensions_mut().insert(PipeConnectInfo {
        client: PipeClient::Other,
    });
    assert_refused(authorize_pipe(other));
    let mut unknown = with_authorization(&["Bearer secret"]);
    unknown.extensions_mut().insert(PipeConnectInfo {
        client: PipeClient::Unknown,
    });
    assert_refused(authorize_pipe(unknown));

    let mut owner = with_authorization(&["Bearer secret"]);
    owner.extensions_mut().insert(PipeConnectInfo {
        client: PipeClient::Owner,
    });
    let admitted = authorize_pipe(owner).unwrap();
    assert_eq!(caller(&admitted).unwrap(), Caller::LocalOwner);
    assert!(admitted.metadata().get(AUTHORIZATION).is_none());
}

#[cfg(unix)]
#[test]
fn a_local_peer_without_credentials_is_refused() {
    let mut request = Request::new(());
    request
        .extensions_mut()
        .insert(tonic::transport::server::UdsConnectInfo {
            peer_addr: None,
            peer_cred: None,
        });
    assert_refused(authorize_local(request, daemon_uid()));
    assert_refused(authorize_local(Request::new(()), daemon_uid()));
}

#[test]
fn generated_tokens_are_distinct_and_well_formed() {
    let (a, b) = (token(), token());
    assert_ne!(a.reveal(), b.reveal());
    // 256 bits are 64 hex digits.
    assert_eq!(a.reveal().len(), "aulo_".len() + 64);
    ApiToken::parse(a.reveal()).unwrap();
}

#[test]
fn parse_rejects_anything_but_the_generated_shape() {
    let good = token();
    let digits = good.reveal().trim_start_matches("aulo_");
    for bad in [
        String::new(),
        digits.to_owned(),
        format!("aulo_{}", &digits[1..]),
        format!("aulo_{digits}0"),
        format!("aulo_{}", digits.to_uppercase()),
        format!("AULO_{digits}"),
    ] {
        assert!(matches!(ApiToken::parse(&bad), Err(TokenError::Malformed)));
    }
}

#[test]
fn debug_output_never_shows_the_token() {
    let token = token();
    let store = MemoryTokenStore::with_token(token.clone());
    let verifier = TokenVerifier::new(&token);
    for shown in [
        format!("{token:?}"),
        format!("{store:?}"),
        format!("{verifier:?}"),
    ] {
        assert!(!shown.contains(token.reveal()), "{shown}");
    }
}

#[test]
fn log_redaction_masks_the_token_under_any_field_name() {
    let token = token();
    for line in [
        format!("peer connected with {}", token.reveal()),
        format!("value={}", token.reveal()),
        format!(r#"{{"fields":{{"note":"{}"}}}}"#, token.reveal()),
    ] {
        let scrubbed = aulo_telemetry::redact::scrub_line(&line);
        assert!(!scrubbed.contains(token.reveal()), "{scrubbed}");
        assert!(
            scrubbed.contains(aulo_telemetry::redact::REDACTED),
            "{scrubbed}"
        );
    }
}

#[test]
fn first_run_creates_a_token_and_later_runs_reuse_it() {
    let store = MemoryTokenStore::default();
    assert!(store.load().unwrap().is_none());
    let first = store.load_or_create().unwrap();
    let again = store.load_or_create().unwrap();
    assert_eq!(first.reveal(), again.reveal());
}

#[test]
fn keychain_errors_keep_only_the_message() {
    // `BadEncoding` carries the stored bytes; the message must not.
    let stored = b"aulo_secretbytes".to_vec();
    let error = token::keychain(keyring::Error::BadEncoding(stored));
    assert!(!format!("{error:?}").contains("secretbytes"), "{error:?}");
}
