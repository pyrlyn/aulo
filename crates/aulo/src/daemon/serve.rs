//! Binds the endpoints and serves the API until SIGINT or SIGTERM.

use std::io;
use std::path::Path;

use anyhow::{Context, Result};
use aulo_config::Config;
use aulo_server::{ApiServer, CancellationToken, Limits, Listen, TcpListen, bind};

pub async fn serve(config: &Config, home: &Path) -> Result<()> {
    // Before binding, so a signal during startup still takes the clean path.
    let shutdown = CancellationToken::new();
    cancel_on_signal(&shutdown).context("cannot install the signal handlers")?;

    let mut listeners = vec![
        bind(&local_listen(home))
            .await
            .context("cannot bind the local socket")?,
    ];
    if let Some(addr) = config.daemon.listen {
        let tcp = Listen::Tcp(TcpListen {
            addr,
            // Token auth (T3.4) and TLS (T3.8) are not installed yet, so a
            // non-loopback address stays refused.
            remote_auth_configured: false,
        });
        listeners.push(bind(&tcp).await.context("cannot bind daemon.listen")?);
    }
    tracing::info!(endpoints = listeners.len(), "aulod serving");

    ApiServer::new(Limits::default())?
        .serve(listeners, shutdown)
        .await?;
    tracing::info!("aulod stopped");
    Ok(())
}

#[cfg(unix)]
fn local_listen(home: &Path) -> Listen {
    Listen::Unix(aulo_server::local_socket_path(home))
}

#[cfg(windows)]
fn local_listen(_home: &Path) -> Listen {
    let user = std::env::var("USERNAME").unwrap_or_default();
    Listen::NamedPipe(format!(r"\\.\pipe\aulod-{user}"))
}

#[cfg(unix)]
fn cancel_on_signal(shutdown: &CancellationToken) -> io::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    let shutdown = shutdown.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = interrupt.recv() => {}
            _ = terminate.recv() => {}
        }
        tracing::info!("shutdown requested");
        shutdown.cancel();
    });
    Ok(())
}

#[cfg(windows)]
fn cancel_on_signal(shutdown: &CancellationToken) -> io::Result<()> {
    let mut ctrl_c = tokio::signal::windows::ctrl_c()?;
    let shutdown = shutdown.clone();
    tokio::spawn(async move {
        ctrl_c.recv().await;
        tracing::info!("shutdown requested");
        shutdown.cancel();
    });
    Ok(())
}
