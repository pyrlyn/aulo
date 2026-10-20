//! Shared bits of the commands that talk to aulod.

use std::path::PathBuf;

use anyhow::{Context, Result};
use tokio::runtime::Runtime;

pub fn home() -> Result<PathBuf> {
    aulo_config::aulo_home().context("cannot find the aulo home: set AULO_HOME or HOME")
}

pub fn runtime() -> Result<Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("cannot start the async runtime")
}
