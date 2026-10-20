//! Binds the endpoints and serves the API until SIGINT or SIGTERM.

use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use aulo_config::{Config, system_store};
use aulo_server::auth::TokenStore;
use aulo_server::{
    ApiServer, CancellationToken, ChatApi, ConfigApi, Limits, Listen, ScriptedProvider,
    ScriptedSession, TcpListen, bind,
};
use aulo_store::Store;

pub async fn serve(
    config: &Config,
    home: &Path,
    scripted: bool,
    scenario: Option<&Path>,
) -> Result<()> {
    // Before binding, so a signal during startup still takes the clean path.
    let shutdown = CancellationToken::new();
    cancel_on_signal(&shutdown).context("cannot install the signal handlers")?;

    // `ApiServer::serve` refuses a TCP listener without a token (T16.1);
    // load-or-create one only when `daemon.listen` asks for TCP, so a
    // unix-socket daemon never touches the keychain. Each home gets its own
    // account, so a test daemon never reads or replaces the real one's.
    let token = match config.daemon.listen {
        Some(_) => Some(
            aulo_server::auth::KeychainTokenStore::new(
                "dev.aulo.daemon",
                home.display().to_string(),
            )
            .load_or_create()
            .context("cannot load the API token for daemon.listen")?,
        ),
        None => None,
    };

    let mut listeners = vec![
        bind(&local_listen(home))
            .await
            .context("cannot bind the local socket")?,
    ];
    if let Some(addr) = config.daemon.listen {
        let tcp = Listen::Tcp(TcpListen {
            addr,
            // The daemon does not load a certificate from config yet, so a
            // non-loopback address stays refused.
            tls: None,
        });
        listeners.push(bind(&tcp).await.context("cannot bind daemon.listen")?);
    }
    tracing::info!(endpoints = listeners.len(), "aulod serving");

    let mut server = ApiServer::new(Limits::default())?;
    if let Some(token) = &token {
        server = server.with_token(token);
    }
    // Provider keys are write-only on this service. The store is the OS
    // keychain unless AULO_KEYCHAIN is off; constructing it does not prompt.
    server =
        server.add_service(ConfigApi::new(config.clone(), Arc::from(system_store())).service());
    let store = Arc::new(Mutex::new(open_store(home)?));
    server = server.add_service(ChatApi::new(Arc::clone(&store)).into_server());
    if scripted {
        let text = scenario_text(scenario)?;
        let provider = ScriptedProvider::parse(&text)?;
        server = server.add_service(ScriptedSession::new(store, provider).into_server());
        tracing::info!("scripted provider mounted");
    }
    server.serve(listeners, shutdown).await?;
    tracing::info!("aulod stopped");
    Ok(())
}

fn open_store(home: &Path) -> Result<Store> {
    let path = home.join("aulo.db");
    let text = path.to_str().context("the aulo home is not valid UTF-8")?;
    Store::open(text).with_context(|| format!("cannot open {text}"))
}

/// `--scripted-scenario` wins, then `AULO_SCRIPTED_SCENARIO`, then the built-in fixture.
fn scenario_text(path: Option<&Path>) -> Result<String> {
    if let Some(path) = path {
        return std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()));
    }
    if let Some(env) = std::env::var_os("AULO_SCRIPTED_SCENARIO") {
        let path = std::path::PathBuf::from(env);
        return std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read {}", path.display()));
    }
    Ok(aulo_server::DEFAULT_SCENARIO.to_owned())
}

#[cfg(unix)]
fn local_listen(home: &Path) -> Listen {
    Listen::Unix(aulo_server::local_socket_path(home))
}

#[cfg(windows)]
fn local_listen(_home: &Path) -> Listen {
    Listen::NamedPipe(aulo_server::local_pipe_name())
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
