//! Binds the endpoints clients reach: the owner-only local socket (a Unix
//! socket, or a named pipe on Windows) and the optional TCP port.
//!
//! Binding is separate from serving so `aulod` fails fast on a busy socket or
//! a refused address before it starts anything else.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use tonic::transport::server::TcpIncoming;

use crate::error::ServerError;

/// Where the local socket lives under `AULO_HOME`. The dedicated `run`
/// directory is what [`bind`] restricts to the owner, so the home directory
/// itself keeps whatever mode the user gave it.
pub fn local_socket_path(aulo_home: &Path) -> PathBuf {
    aulo_home.join("run").join("aulod.sock")
}

/// An endpoint to bind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listen {
    /// Absolute socket path; see [`local_socket_path`].
    #[cfg(unix)]
    Unix(PathBuf),
    /// Pipe name such as `\\.\pipe\aulod-<user>`.
    #[cfg(windows)]
    NamedPipe(String),
    Tcp(TcpListen),
}

/// The remote endpoint (`daemon.listen`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpListen {
    pub addr: SocketAddr,
    /// Set only once TLS (T3.8) is configured. `ApiServer::serve` already
    /// refuses any TCP listener without a token, but a token alone is not
    /// enough off loopback: without TLS it would cross the network in clear.
    /// Until then a non-loopback address is refused.
    pub remote_auth_configured: bool,
}

/// A bound endpoint, ready for `ApiServer::serve`.
#[derive(Debug)]
pub struct Bound(pub(crate) BoundKind);

#[derive(Debug)]
pub(crate) enum BoundKind {
    #[cfg(unix)]
    Unix(tokio::net::UnixListener, SocketFile),
    #[cfg(windows)]
    NamedPipe(tokio::net::windows::named_pipe::NamedPipeServer, String),
    Tcp(TcpIncoming, SocketAddr),
}

impl Bound {
    /// The bound TCP address, which tells callers the port when they asked
    /// for port 0.
    pub fn tcp_addr(&self) -> Option<SocketAddr> {
        match &self.0 {
            BoundKind::Tcp(_, addr) => Some(*addr),
            _ => None,
        }
    }
}

/// Binds one endpoint.
pub async fn bind(listen: &Listen) -> Result<Bound, ServerError> {
    match listen {
        #[cfg(unix)]
        Listen::Unix(path) => unix::bind(path).await,
        #[cfg(windows)]
        Listen::NamedPipe(name) => crate::pipe::bind(name),
        Listen::Tcp(tcp) => bind_tcp(*tcp).await,
    }
}

async fn bind_tcp(tcp: TcpListen) -> Result<Bound, ServerError> {
    if !tcp.addr.ip().is_loopback() && !tcp.remote_auth_configured {
        return Err(ServerError::UnauthenticatedRemote(tcp.addr));
    }
    let listener = tokio::net::TcpListener::bind(tcp.addr)
        .await
        .map_err(ServerError::io("bind", tcp.addr))?;
    let addr = listener
        .local_addr()
        .map_err(ServerError::io("read local address of", tcp.addr))?;
    // gRPC traffic is many small frames; Nagle would delay each one.
    let incoming = TcpIncoming::from(listener).with_nodelay(Some(true));
    Ok(Bound(BoundKind::Tcp(incoming, addr)))
}

/// Removes the socket file when the server stops, so the next start does not
/// have to probe a stale one.
#[cfg(unix)]
#[derive(Debug)]
pub(crate) struct SocketFile(PathBuf);

#[cfg(unix)]
impl Drop for SocketFile {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.0) {
            tracing::warn!(path = %self.0.display(), error = %e, "could not remove the socket file");
        }
    }
}

#[cfg(unix)]
mod unix {
    use std::fs::{self, DirBuilder, Permissions};
    use std::io::ErrorKind;
    use std::os::unix::fs::{DirBuilderExt, FileTypeExt, PermissionsExt};
    use std::path::Path;
    use std::time::Duration;

    use tokio::net::{UnixListener, UnixStream};

    use super::{Bound, BoundKind, SocketFile};
    use crate::error::ServerError;

    // A live server answers a local connect at once; one that takes longer is
    // still treated as live, because removing its socket would cut it off.
    const PROBE_TIMEOUT: Duration = Duration::from_secs(1);

    pub(super) async fn bind(path: &Path) -> Result<Bound, ServerError> {
        let parent = path
            .parent()
            .filter(|p| path.is_absolute() && !p.as_os_str().is_empty())
            .ok_or_else(|| ServerError::InvalidSocketPath(path.to_owned()))?;
        owner_only_dir(parent)?;
        clear_stale(path).await?;
        let listener = UnixListener::bind(path).map_err(ServerError::io("bind", path.display()))?;
        let file = SocketFile(path.to_owned());
        // The 0700 directory already keeps other users out between bind and
        // chmod; the socket mode is the second fence if the directory changes.
        fs::set_permissions(path, Permissions::from_mode(0o600))
            .map_err(ServerError::io("restrict", path.display()))?;
        Ok(Bound(BoundKind::Unix(listener, file)))
    }

    /// Creates the directory as 0700, or accepts an existing one only if no
    /// other user can enter it: whoever can write there can swap the socket.
    fn owner_only_dir(dir: &Path) -> Result<(), ServerError> {
        match fs::symlink_metadata(dir) {
            Ok(meta) if meta.is_dir() => {
                if meta.permissions().mode() & 0o077 != 0 {
                    return Err(ServerError::InsecureDirectory(dir.to_owned()));
                }
                Ok(())
            }
            // A symlink is refused too: its target could be shared.
            Ok(_) => Err(ServerError::NotADirectory(dir.to_owned())),
            Err(e) if e.kind() == ErrorKind::NotFound => DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)
                .map_err(ServerError::io("create", dir.display())),
            Err(e) => Err(ServerError::io("inspect", dir.display())(e)),
        }
    }

    /// Removes a socket left by a crashed server, and nothing else: a socket
    /// that still answers belongs to a running server, and a non-socket file
    /// is not ours to delete.
    async fn clear_stale(path: &Path) -> Result<(), ServerError> {
        let meta = match fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(ServerError::io("inspect", path.display())(e)),
        };
        if !meta.file_type().is_socket() {
            return Err(ServerError::NotASocket(path.to_owned()));
        }
        match tokio::time::timeout(PROBE_TIMEOUT, UnixStream::connect(path)).await {
            Ok(Err(e)) if e.kind() == ErrorKind::ConnectionRefused => {
                tracing::info!(path = %path.display(), "removing a stale socket");
                fs::remove_file(path).map_err(ServerError::io("remove stale", path.display()))
            }
            Ok(Err(e)) => Err(ServerError::io("probe", path.display())(e)),
            Ok(Ok(_)) | Err(_) => Err(ServerError::SocketInUse(path.to_owned())),
        }
    }
}
