//! `aulo config`: inspect the effective configuration.

use std::io::Write;

use anyhow::{Context, Result};
use aulo_config::{Config, DEFAULT_TOML, LoadOptions};
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the effective configuration (secrets masked).
    Show {
        /// Print `key = value  # origin` lines instead of TOML.
        #[arg(long)]
        origin: bool,
    },
    /// Print the commented-out default configuration file.
    Default,
}

pub fn run(command: ConfigCommand, out: &mut impl Write) -> Result<()> {
    let text = match command {
        ConfigCommand::Default => DEFAULT_TOML.to_owned(),
        ConfigCommand::Show { origin } => show(origin)?,
    };
    out.write_all(text.as_bytes())
        .context("cannot write to stdout")
}

fn show(origin: bool) -> Result<String> {
    // The project layer is the working directory's `.aulo.toml`, as for `aulod`.
    let project_dir = std::env::current_dir().context("cannot read the working directory")?;
    let loaded = Config::load(&LoadOptions {
        project_dir: Some(project_dir),
        ..LoadOptions::default()
    })?;
    Ok(if origin {
        loaded.render_origins()?
    } else {
        loaded.config.to_toml_redacted()?
    })
}
