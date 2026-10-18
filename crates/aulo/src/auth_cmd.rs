//! `aulo auth`: provider API keys in the OS keychain.
//!
//! The key is read from stdin so it is not a process argument. Commands print
//! whether a key is set, never the key.

use std::io::{self, Write};

use anyhow::{Context, Result};
use aulo_config::{
    Config, KeyState, LoadOptions, configured_keys, delete_key, key_state, store_key, system_store,
};
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Store a provider key in the OS keychain. The key is one line on stdin.
    Set {
        /// Endpoint name from `[providers.endpoints]`.
        provider: String,
    },
    /// Remove a provider key from the OS keychain.
    Rm {
        /// Endpoint name from `[providers.endpoints]`.
        provider: String,
    },
    /// List configured providers and whether a key is available.
    Ls,
}

pub fn run(command: AuthCommand, out: &mut impl Write) -> Result<()> {
    match command {
        AuthCommand::Set { provider } => set(&provider, out),
        AuthCommand::Rm { provider } => remove(&provider, out),
        AuthCommand::Ls => list(out),
    }
}

fn set(provider: &str, out: &mut impl Write) -> Result<()> {
    let key = read_key()?;
    let store = system_store();
    store_key(provider, &key, store.as_ref())?;
    drop(key);
    writeln!(out, "stored the key for {provider} in the OS keychain")
        .context("cannot write to stdout")?;
    if loaded()?.providers.endpoints.contains_key(provider) {
        Ok(())
    } else {
        writeln!(
            out,
            "{provider} is not in the configuration yet; add [providers.endpoints.{provider}] before a tier can use it"
        )
        .context("cannot write to stdout")
    }
}

fn remove(provider: &str, out: &mut impl Write) -> Result<()> {
    let store = system_store();
    delete_key(provider, store.as_ref())?;
    writeln!(out, "removed the keychain entry for {provider}").context("cannot write to stdout")?;
    if let Ok(config) = loaded() {
        let kind = config
            .providers
            .endpoints
            .get(provider)
            .map(|endpoint| endpoint.kind);
        if let Ok(KeyState::Env { variable }) = key_state(provider, kind, store.as_ref()) {
            writeln!(out, "{variable} is still set and takes precedence")
                .context("cannot write to stdout")?;
        }
    }
    Ok(())
}

fn list(out: &mut impl Write) -> Result<()> {
    let config = loaded()?;
    let listed = configured_keys(&config, system_store().as_ref())?;
    if listed.is_empty() {
        writeln!(out, "no providers configured").context("cannot write to stdout")?;
        return Ok(());
    }
    for info in listed {
        writeln!(out, "{}\t{}\t{}", info.id, info.kind.as_str(), info.state)
            .context("cannot write to stdout")?;
    }
    Ok(())
}

fn loaded() -> Result<Config> {
    let project_dir = std::env::current_dir().context("cannot read the working directory")?;
    Ok(Config::load(&LoadOptions {
        project_dir: Some(project_dir),
        ..LoadOptions::default()
    })?
    .config)
}

/// One line from stdin, without the trailing newline. Surrounding whitespace
/// is removed by [`store_key`](aulo_config::store_key).
fn read_key() -> Result<String> {
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .context("cannot read the key from stdin")?;
    Ok(line)
}
