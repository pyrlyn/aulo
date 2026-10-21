//! ApprovalService over a real socket: list, page, revoke, and the gate that
//! a decision does not insert a grant.
#![cfg(unix)]
#![allow(clippy::unwrap_used)]

mod common;

use std::sync::{Arc, Mutex};

use aulo_proto::aulo::v1::approval_service_client::ApprovalServiceClient;
use aulo_proto::aulo::v1::{
    ApprovalDecision, DecideRequest, GrantDecision, ListGrantsRequest, ListPendingRequest,
    RevokeGrantRequest,
};
use aulo_server::GrantApi;
use aulo_store::Store;
use common::{Running, connect, start_with};
use tonic::transport::Channel;
use tonic::{Code, Request};

const MISSING: &str = "01J00000000000000000000000";

struct Fixture {
    _server: Running,
    _db: tempfile::TempDir,
    store: Arc<Mutex<Store>>,
    client: ApprovalServiceClient<Channel>,
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("aulo.db");
    let store = Arc::new(Mutex::new(Store::open(path.to_str().unwrap()).unwrap()));
    let api = GrantApi::new(Arc::clone(&store));
    let server = start_with(aulo_server::Limits::default(), |server| {
        server.add_service(api.service())
    })
    .await;
    let client = ApprovalServiceClient::new(connect(&server.socket).await);
    Fixture {
        _server: server,
        _db: dir,
        store,
        client,
    }
}

fn put(store: &Arc<Mutex<Store>>, subject: &str, decision: &str) -> String {
    store
        .lock()
        .unwrap()
        .put_grant(subject, "browse", decision, None)
        .unwrap()
        .id
}

#[tokio::test]
async fn grants_page_and_revoke_without_decide_inserting_one() {
    let mut f = fixture().await;
    let first = put(&f.store, "app:Safari", "allow");
    let second = put(&f.store, "app:Mail", "deny");
    let third = put(&f.store, "app:Notes", "later");

    let pending = f
        .client
        .list_pending(ListPendingRequest::default())
        .await
        .unwrap()
        .into_inner();
    assert!(pending.approvals.is_empty());

    let bad_chat = f
        .client
        .list_pending(ListPendingRequest {
            chat_id: Some("nope".into()),
        })
        .await
        .unwrap_err();
    assert_eq!(bad_chat.code(), Code::InvalidArgument);

    let page = f
        .client
        .list_grants(ListGrantsRequest {
            page_size: 2,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(page.grants.len(), 2);
    assert_eq!(page.grants[0].id, first);
    assert_eq!(page.grants[0].decision(), GrantDecision::Allow);
    assert_eq!(page.grants[1].id, second);
    assert_eq!(page.grants[1].subject, "app:Mail");
    assert_eq!(page.grants[1].decision(), GrantDecision::Deny);
    assert!(!page.next_page_token.is_empty());

    let rest = f
        .client
        .list_grants(ListGrantsRequest {
            page_size: 2,
            page_token: page.next_page_token,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(rest.grants.len(), 1);
    assert_eq!(rest.grants[0].id, third);
    assert_eq!(rest.grants[0].decision(), GrantDecision::Unspecified);
    assert!(rest.next_page_token.is_empty());

    let decided = f
        .client
        .decide(DecideRequest {
            call_id: MISSING.into(),
            decision: ApprovalDecision::AllowAlways.into(),
        })
        .await
        .unwrap_err();
    assert_eq!(decided.code(), Code::NotFound);
    assert_eq!(f.store.lock().unwrap().list_grants().unwrap().len(), 3);

    let unspecified = f
        .client
        .decide(DecideRequest {
            call_id: MISSING.into(),
            decision: 0,
        })
        .await
        .unwrap_err();
    assert_eq!(unspecified.code(), Code::InvalidArgument);

    let bad_token = f
        .client
        .list_grants(ListGrantsRequest {
            page_token: "c1.0.01J00000000000000000000000".into(),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert_eq!(bad_token.code(), Code::InvalidArgument);
    let bad_id = f
        .client
        .revoke_grant(RevokeGrantRequest {
            grant_id: "not-a-grant".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(bad_id.code(), Code::InvalidArgument);

    f.client
        .revoke_grant(RevokeGrantRequest {
            grant_id: first.to_lowercase(),
        })
        .await
        .unwrap();
    let missing = f
        .client
        .revoke_grant(RevokeGrantRequest {
            grant_id: first.clone(),
        })
        .await
        .unwrap_err();
    assert_eq!(missing.code(), Code::NotFound);

    let left = f
        .client
        .list_grants(Request::new(ListGrantsRequest::default()))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(left.grants.len(), 2);
    assert!(left.grants.iter().all(|grant| grant.id != first));
}
