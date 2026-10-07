//! The aulo daemon: argument parsing and exit codes. Everything else lives in
//! `daemon`, so this file stays a thin surface over it.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

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
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the daemon in the foreground (the default)
    Run,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // `run` is the only command, so naming it or not means the same.
    let (Some(Command::Run) | None) = cli.command;
    let options = daemon::Options {
        config_dir: cli.config,
        listen: cli.listen,
    };
    match daemon::run(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("aulod: {error:#}");
            ExitCode::FAILURE
        }
    }
}
