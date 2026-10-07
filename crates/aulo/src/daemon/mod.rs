//! The daemon's startup sequence: one instance per home, config, logging,
//! then the API server until a termination signal.

mod instance;
mod serve;

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use aulo_config::{Config, LoadOptions};
use aulo_telemetry::Settings;

/// What the command line decided; everything else comes from the config.
#[derive(Debug)]
pub struct Options {
    pub config_dir: Option<PathBuf>,
    pub listen: Option<SocketAddr>,
}

pub fn run(options: Options) -> Result<()> {
    let home =
        aulo_config::aulo_home().context("cannot find the aulo home: set AULO_HOME or HOME")?;
    // First, so a second instance reports the running one and does not touch
    // its logs or sockets.
    let _instance = instance::Instance::acquire(&home)?;

    let loaded = Config::load(&LoadOptions {
        home: Some(options.config_dir.unwrap_or_else(|| home.clone())),
        project_dir: None,
        // Through the config layers so `--listen` is validated like the file key.
        overrides: options
            .listen
            .map(|addr| serde_json::json!({ "daemon": { "listen": addr.to_string() } })),
    })?;

    let mut logging = Settings::new("info", home.join("logs"));
    // A daemon runs unattended; the files are its log.
    logging.stderr = false;
    let _logs = aulo_telemetry::init(&logging).context("cannot start logging")?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("cannot start the async runtime")?;
    runtime.block_on(serve::serve(&loaded.config, &home))
}
