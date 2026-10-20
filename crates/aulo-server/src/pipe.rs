//! The Windows named-pipe listener.
//!
//! A pipe has no listener object: each instance serves one client, so a task
//! keeps one spare instance open and hands connected ones to tonic. tonic only
//! accepts IO types that implement `Connected`, hence the wrapper.
//!
//! The client's account is read when the instance connects and carried as
//! [`PipeConnectInfo`](crate::auth::PipeConnectInfo). The auth interceptor
//! grants `LocalOwner` only when that account is the daemon's user.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::server::Connected;

use crate::error::ServerError;
use crate::listener::{Bound, BoundKind};

pub(crate) fn bind(name: &str) -> Result<Bound, ServerError> {
    // `first_pipe_instance` fails if the name already exists, so another
    // process cannot squat the name and receive our clients.
    let first = options()
        .first_pipe_instance(true)
        .create(name)
        .map_err(ServerError::io("create pipe", name))?;
    Ok(Bound(BoundKind::NamedPipe(first, name.to_owned())))
}

fn options() -> ServerOptions {
    let mut options = ServerOptions::new();
    options.reject_remote_clients(true);
    options
}

pub(crate) fn incoming(
    first: NamedPipeServer,
    name: String,
    stop: CancellationToken,
) -> ReceiverStream<io::Result<PipeIo>> {
    let (tx, rx) = mpsc::channel(1);
    tokio::spawn(async move {
        let mut spare = first;
        loop {
            let connected = tokio::select! {
                result = spare.connect() => result,
                () = stop.cancelled() => break,
            };
            // The next instance exists before this one is handed off, so a
            // client never finds the name without a listening instance.
            let next = match options().create(&name) {
                Ok(next) => next,
                Err(e) => {
                    tracing::error!(pipe = %name, error = %e, "named pipe listener stopped");
                    break;
                }
            };
            let current = std::mem::replace(&mut spare, next);
            if let Err(e) = connected {
                tracing::warn!(pipe = %name, error = %e, "named pipe connect failed");
                continue;
            }
            // Fixed at accept time, so a later request on this connection
            // cannot present a different identity.
            let client = crate::auth::pipe_client(&current);
            let io = PipeIo {
                io: current,
                info: crate::auth::PipeConnectInfo { client },
            };
            if tx.send(Ok(io)).await.is_err() {
                break;
            }
        }
    });
    ReceiverStream::new(rx)
}

#[derive(Debug)]
pub(crate) struct PipeIo {
    io: NamedPipeServer,
    info: crate::auth::PipeConnectInfo,
}

impl Connected for PipeIo {
    type ConnectInfo = crate::auth::PipeConnectInfo;

    fn connect_info(&self) -> Self::ConnectInfo {
        self.info
    }
}

impl AsyncRead for PipeIo {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_read(cx, buf)
    }
}

impl AsyncWrite for PipeIo {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().io).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_shutdown(cx)
    }
}
