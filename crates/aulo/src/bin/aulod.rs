//! The aulo daemon: argument parsing and exit codes. Everything else lives in
//! `daemon`, so this file stays a thin surface over it.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};

#[path = "../daemon/mod.rs"]
mod daemon;

#[derive(Debug, Parser)]
#[command(name = "aulod", version, about = "The aulo daemon")]
struct Cli {
    /// Directory holding config.toml (default: AULO_HOME)
    #[arg(long, global = true, value_name = "DIR")]
    config: Option<PathBuf>,
    /// TCP address for remote clients; overrides daemon.listen
    #[arg(long, global = true, value_name = "ADDR")]
    listen: Option<SocketAddr>,
    /// Home directory for this run; the Windows service passes it instead of AULO_HOME
    #[arg(long, hide = true, value_name = "DIR")]
    home: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the daemon in the foreground (the default)
    Run,
    /// Manage aulod as a user service that starts at login
    Service {
        #[command(subcommand)]
        action: daemon::service::Action,
    },
}

fn main() -> ExitCode {
    match start(Cli::parse()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("aulod: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn start(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        // Naming `run` or not means the same.
        Some(Command::Run) | None => {
            daemon::run(daemon::Options {
                home: cli.home,
                config_dir: cli.config,
                listen: cli.listen,
            })?;
            Ok(ExitCode::SUCCESS)
        }
        Some(Command::Service { action }) => {
            // The unit runs plain `aulod run`; dropping a flag silently would
            // leave the user believing it was installed.
            if cli.config.is_some() || cli.listen.is_some() || cli.home.is_some() {
                bail!(
                    "--config, --listen and --home do not apply to `service`; use AULO_HOME and config.toml"
                );
            }
            daemon::service::execute(action)
        }
    }
}
