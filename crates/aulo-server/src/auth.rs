//! Who may call the API, decided per listener before any service runs.
//!
//! - The local socket admits only the daemon's own user: the peer's uid comes
//!   from the kernel (`SO_PEERCRED` / `getpeereid`), so the 0700 directory and
//!   0600 socket are a second fence, not the only one.
//! - The named pipe admits only a client whose process token is that same
//!   user (`GetNamedPipeClientProcessId`, then the process token). A client
//!   the kernel cannot identify is refused. `reject_remote_clients` and the
//!   pipe DACL stay as a second fence, not the only one.
//! - TCP admits only a request carrying `authorization: Bearer <token>`.
//!
//! Every admitted request carries a [`Caller`] in its extensions; a service
//! reads it with [`caller`]. A request without one is refused, so a service
//! mounted without these checks fails closed instead of open.

mod token;

#[cfg(windows)]
mod pipe_owner;

#[cfg(test)]
mod tests;

use std::net::SocketAddr;

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tonic::{Request, Status};

pub use token::{ApiToken, KeychainTokenStore, MemoryTokenStore, TokenError, TokenStore};

const AUTHORIZATION: &str = "authorization";
const BEARER: &str = "bearer";
/// Bytes in a SHA-256 digest.
const DIGEST_LEN: usize = 32;

/// The authenticated identity of one request, attached by the listener's
/// check. Later tasks narrow it with token scopes (spec §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Caller {
    /// The daemon's own user over the local socket or named pipe.
    LocalOwner,
    /// A TCP client that presented the API token.
    TokenHolder { remote: Option<SocketAddr> },
}

/// The [`Caller`] the listener attached, or `UNAUTHENTICATED` if none did.
pub fn caller<T>(request: &Request<T>) -> Result<Caller, Status> {
    request
        .extensions()
        .get::<Caller>()
        .copied()
        .ok_or_else(unauthenticated)
}

// One message for every failure, so a client cannot tell a missing header
// from a malformed or wrong one, nor learn anything about the expected token.
fn unauthenticated() -> Status {
    Status::unauthenticated("authentication required")
}

/// Checks presented tokens against the API token. Keeps only its SHA-256
/// (spec §11 stores tokens hashed), so the server never holds the token
/// itself and the comparison covers a fixed 32 bytes whatever is presented.
#[derive(Clone)]
pub(crate) struct TokenVerifier {
    digest: [u8; DIGEST_LEN],
}

impl std::fmt::Debug for TokenVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenVerifier").finish_non_exhaustive()
    }
}

impl TokenVerifier {
    pub(crate) fn new(token: &ApiToken) -> Self {
        Self {
            digest: Sha256::digest(token.reveal().as_bytes()).into(),
        }
    }

    fn matches(&self, presented: &str) -> bool {
        let digest: [u8; DIGEST_LEN] = Sha256::digest(presented.as_bytes()).into();
        bool::from(digest.ct_eq(&self.digest))
    }
}

/// The interceptor for the TCP listener.
pub(crate) fn bearer(
    verifier: TokenVerifier,
) -> impl FnMut(Request<()>) -> Result<Request<()>, Status> + Clone + Send + Sync + 'static {
    move |request| authorize_bearer(request, &verifier)
}

pub(crate) fn authorize_bearer(
    mut request: Request<()>,
    verifier: &TokenVerifier,
) -> Result<Request<()>, Status> {
    let mut headers = request.metadata().get_all(AUTHORIZATION).iter();
    // Exactly one header: with two, a proxy and the server could disagree on
    // which one counts.
    let presented = match (headers.next(), headers.next()) {
        (Some(value), None) => value.to_str().ok().and_then(bearer_token),
        _ => None,
    };
    let admitted = presented.is_some_and(|token| verifier.matches(token));
    // Services never see the token, so none of them can log or echo it.
    request.metadata_mut().remove(AUTHORIZATION);
    if !admitted {
        return Err(unauthenticated());
    }
    let remote = request.remote_addr();
    request
        .extensions_mut()
        .insert(Caller::TokenHolder { remote });
    Ok(request)
}

/// The token of an `authorization` value. The scheme is case-insensitive
/// (RFC 9110 §11.1); the token itself is compared exactly.
fn bearer_token(value: &str) -> Option<&str> {
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim_matches(' ');
    (scheme.eq_ignore_ascii_case(BEARER) && !token.is_empty()).then_some(token)
}

/// The effective uid the local socket admits: the daemon's own.
#[cfg(unix)]
pub(crate) fn daemon_uid() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// The interceptor for the local socket.
#[cfg(unix)]
pub(crate) fn local_owner(
    owner: u32,
) -> impl FnMut(Request<()>) -> Result<Request<()>, Status> + Clone + Send + Sync + 'static {
    move |request| authorize_local(request, owner)
}

#[cfg(unix)]
pub(crate) fn authorize_local(mut request: Request<()>, owner: u32) -> Result<Request<()>, Status> {
    let peer = request
        .extensions()
        .get::<tonic::transport::server::UdsConnectInfo>()
        .and_then(|info| info.peer_cred)
        .map(|cred| cred.uid());
    if !is_owner(peer, owner) {
        tracing::warn!(
            ?peer,
            "refused a local client that is not the daemon's user"
        );
        return Err(unauthenticated());
    }
    request.metadata_mut().remove(AUTHORIZATION);
    request.extensions_mut().insert(Caller::LocalOwner);
    Ok(request)
}

/// A peer whose credentials the kernel could not report is refused too.
#[cfg(unix)]
pub(crate) fn is_owner(peer: Option<u32>, owner: u32) -> bool {
    peer == Some(owner)
}

/// What the kernel reported about the process on the other end of a pipe.
#[cfg(any(windows, test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PipeClient {
    /// Its process token is the daemon's user, and the pid still matches.
    Owner,
    /// The kernel named a different account.
    Other,
    /// No pid, no token, or the pid changed while it was being read.
    Unknown,
}

/// Identity fixed when the pipe instance accepts a client. tonic copies it
/// into every request on that connection; a client cannot set it.
#[cfg(any(windows, test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PipeConnectInfo {
    pub(crate) client: PipeClient,
}

/// `SECURITY_MAX_SID_SIZE`: a SID is at most this many bytes.
#[cfg(any(windows, test))]
const SID_CAP: usize = 68;

/// A Windows account SID copied out of a process token.
///
/// Empty values cannot be built, and [`same_account`] still rejects a zero
/// length, so a failed read cannot look like a match.
#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AccountSid {
    bytes: [u8; SID_CAP],
    len: u8,
}

#[cfg(any(windows, test))]
impl AccountSid {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() || bytes.len() > SID_CAP {
            return None;
        }
        let Ok(len) = u8::try_from(bytes.len()) else {
            return None;
        };
        let mut sid = Self {
            bytes: [0; SID_CAP],
            len,
        };
        sid.bytes[..bytes.len()].copy_from_slice(bytes);
        Some(sid)
    }
}

/// Both SIDs were read and name the same account.
#[cfg(any(windows, test))]
fn same_account(client: &AccountSid, owner: &AccountSid) -> bool {
    let Some(client_bytes) = client.bytes.get(..usize::from(client.len)) else {
        return false;
    };
    let Some(owner_bytes) = owner.bytes.get(..usize::from(owner.len)) else {
        return false;
    };
    !client_bytes.is_empty() && client_bytes == owner_bytes
}

/// Decides the pipe client from the two account reads and whether the pid
/// was still the one `GetNamedPipeClientProcessId` returned afterwards.
///
/// `pid_unchanged` is false when the process id was reused, or the second
/// read failed, between the two calls. That client is refused.
#[cfg(any(windows, test))]
fn classify_pipe_client(
    client: Option<&AccountSid>,
    owner: Option<&AccountSid>,
    pid_unchanged: bool,
) -> PipeClient {
    if !pid_unchanged {
        return PipeClient::Unknown;
    }
    match (client, owner) {
        (Some(client), Some(owner)) if same_account(client, owner) => PipeClient::Owner,
        (Some(_), Some(_)) => PipeClient::Other,
        _ => PipeClient::Unknown,
    }
}

/// The client's account, read from the connected server end.
#[cfg(windows)]
pub(crate) fn pipe_client(server: &tokio::net::windows::named_pipe::NamedPipeServer) -> PipeClient {
    pipe_owner::probe(server)
}

/// The interceptor for the named pipe. `LocalOwner` is granted only when the
/// accept path recorded [`PipeClient::Owner`]. Anything else, including a
/// request that carries no [`PipeConnectInfo`], is refused.
#[cfg(any(windows, test))]
pub(crate) fn authorize_pipe(mut request: Request<()>) -> Result<Request<()>, Status> {
    let client = request
        .extensions()
        .get::<PipeConnectInfo>()
        .map(|info| info.client)
        .unwrap_or(PipeClient::Unknown);
    if client != PipeClient::Owner {
        tracing::warn!(
            ?client,
            "refused a named-pipe client that is not the daemon's user"
        );
        return Err(unauthenticated());
    }
    request.metadata_mut().remove(AUTHORIZATION);
    request.extensions_mut().insert(Caller::LocalOwner);
    Ok(request)
}

#[cfg(windows)]
pub(crate) use authorize_pipe as local_pipe;
