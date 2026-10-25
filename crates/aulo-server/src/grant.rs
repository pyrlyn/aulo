//! `aulo.v1.ApprovalService` for standing grants.
//!
//! Policy (T5.3) is what inserts a grant, when a person answers always-allow
//! or always-deny. This service only lists and revokes rows that are already
//! stored. `Decide` answers `NOT_FOUND` for every call: nothing is waiting,
//! and a decision never becomes a grant here. `ListPending` is empty for the
//! same reason.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use aulo_proto::Timestamp;
use aulo_proto::aulo::v1::approval_service_server::{ApprovalService, ApprovalServiceServer};
use aulo_proto::aulo::v1::{
    ApprovalDecision, DecideRequest, DecideResponse, GrantDecision, ListGrantsRequest,
    ListGrantsResponse, ListPendingRequest, ListPendingResponse, RevokeGrantRequest,
    RevokeGrantResponse,
};
use aulo_store::{Store, StoreError};
use tonic::{Request, Response, Status};

use crate::chat::ChatApi;
use crate::chat::input::{fetch_limit, page_size, trim_page};
use crate::error::ServerError;
use crate::server::ApiServer;

/// Lists and revokes grants. Clone-cheap: clones share the chat service's store.
#[derive(Debug, Clone)]
pub struct GrantApi {
    store: Arc<Mutex<Store>>,
}

impl GrantApi {
    #[must_use]
    pub fn new(store: Arc<Mutex<Store>>) -> Self {
        Self { store }
    }

    /// The tonic service `ApiServer::add_service` mounts.
    #[must_use]
    pub fn service(self) -> ApprovalServiceServer<Self> {
        ApprovalServiceServer::new(self)
    }

    async fn with_store<T, F>(&self, op: &'static str, f: F) -> Result<T, Status>
    where
        T: Send + 'static,
        F: FnOnce(&mut Store) -> Result<T, Status> + Send + 'static,
    {
        let store = Arc::clone(&self.store);
        tokio::task::spawn_blocking(move || {
            let mut guard = store.lock().unwrap_or_else(PoisonError::into_inner);
            f(&mut guard)
        })
        .await
        .map_err(|e| {
            tracing::error!(op, error = %e, "grant storage task failed");
            internal()
        })?
    }
}

/// Opens `{home}/aulo.db` and mounts [`ChatApi`] and [`GrantApi`] on one connection.
pub fn mount_database(server: ApiServer, home: &Path) -> Result<ApiServer, ServerError> {
    let path = aulo_store::database_path(home);
    let Some(path_str) = path.to_str() else {
        return Err(ServerError::DatabasePath(path));
    };
    let store = Store::open(path_str).map_err(|source| ServerError::Database {
        path: path.clone(),
        source,
    })?;
    let store = Arc::new(Mutex::new(store));
    Ok(server
        .add_service(ChatApi::new(Arc::clone(&store)).service())
        .add_service(GrantApi::new(store).service()))
}

fn internal() -> Status {
    Status::internal("internal storage error")
}

fn storage(op: &'static str, error: &StoreError) -> Status {
    tracing::error!(op, error = %error, "grant storage call failed");
    internal()
}

fn invalid(msg: &'static str) -> Status {
    Status::invalid_argument(msg)
}

/// Parsed and re-printed so a lowercase id still finds the stored canonical form.
fn canonical_ulid(raw: &str, what: &'static str) -> Result<String, Status> {
    raw.parse::<ulid::Ulid>()
        .map(|id| id.to_string())
        .map_err(|_| invalid(what))
}

fn known_decision(raw: i32) -> Result<ApprovalDecision, Status> {
    ApprovalDecision::try_from(raw)
        .ok()
        .filter(|decision| *decision != ApprovalDecision::Unspecified)
        .ok_or_else(|| invalid("decision is not a known approval"))
}

const GRANT_CURSOR_PREFIX: &str = "g1.";

fn grant_cursor(token: &str) -> Result<Option<String>, Status> {
    if token.is_empty() {
        return Ok(None);
    }
    if token.len() > 128 {
        return Err(invalid("page_token is not valid"));
    }
    let Some(id) = token.strip_prefix(GRANT_CURSOR_PREFIX) else {
        return Err(invalid("page_token is not valid"));
    };
    canonical_ulid(id, "page_token is not valid").map(Some)
}

fn timestamp(ms: i64) -> Timestamp {
    Timestamp {
        seconds: ms.div_euclid(1000),
        nanos: i32::try_from(ms.rem_euclid(1000) * 1_000_000).unwrap_or_default(),
    }
}

fn decision_code(stored: &str) -> i32 {
    match stored {
        "allow" => GrantDecision::Allow,
        "deny" => GrantDecision::Deny,
        _ => GrantDecision::Unspecified,
    }
    .into()
}

fn grant_message(row: aulo_store::Grant) -> aulo_proto::aulo::v1::Grant {
    aulo_proto::aulo::v1::Grant {
        id: row.id,
        subject: row.subject,
        scope: row.scope,
        decision: decision_code(&row.decision),
        expires_at: row.expires_at.map(timestamp),
        created_at: Some(timestamp(row.created_at)),
    }
}

#[tonic::async_trait]
impl ApprovalService for GrantApi {
    async fn list_pending(
        &self,
        request: Request<ListPendingRequest>,
    ) -> Result<Response<ListPendingResponse>, Status> {
        if let Some(chat_id) = request.into_inner().chat_id {
            canonical_ulid(&chat_id, "chat_id is not a valid ULID")?;
        }
        Ok(Response::new(ListPendingResponse {
            approvals: Vec::new(),
        }))
    }

    async fn decide(
        &self,
        request: Request<DecideRequest>,
    ) -> Result<Response<DecideResponse>, Status> {
        let req = request.into_inner();
        canonical_ulid(&req.call_id, "call_id is not a valid ULID")?;
        known_decision(req.decision)?;
        // No approval is waiting. Returning not-found, rather than writing a
        // grant from the call id, is the gate until T5.3 owns Decide.
        Err(Status::not_found("approval not found"))
    }

    async fn list_grants(
        &self,
        request: Request<ListGrantsRequest>,
    ) -> Result<Response<ListGrantsResponse>, Status> {
        let req = request.into_inner();
        let size = page_size(req.page_size);
        let after = grant_cursor(&req.page_token)?;
        let mut rows = self
            .with_store("list_grants", move |store| {
                store
                    .list_grants_page(after.as_deref(), fetch_limit(size))
                    .map_err(|error| storage("list_grants", &error))
            })
            .await?;
        let next_page_token = trim_page(&mut rows, size)
            .map(|row| format!("{GRANT_CURSOR_PREFIX}{}", row.id))
            .unwrap_or_default();
        Ok(Response::new(ListGrantsResponse {
            grants: rows.into_iter().map(grant_message).collect(),
            next_page_token,
        }))
    }

    async fn revoke_grant(
        &self,
        request: Request<RevokeGrantRequest>,
    ) -> Result<Response<RevokeGrantResponse>, Status> {
        let id = canonical_ulid(
            &request.into_inner().grant_id,
            "grant_id is not a valid ULID",
        )?;
        let removed = self
            .with_store("revoke_grant", move |store| {
                store
                    .revoke_grant(&id)
                    .map_err(|error| storage("revoke_grant", &error))
            })
            .await?;
        if !removed {
            return Err(Status::not_found("grant not found"));
        }
        Ok(Response::new(RevokeGrantResponse {}))
    }
}
