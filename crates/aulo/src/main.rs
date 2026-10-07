//! The `aulo` command line. Parsing and error presentation live here; what a
//! command does lives in the library crates it calls.

mod config_cmd;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "aulo", version, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

// One variant per top-level command; later commands are added here.
#[derive(Debug, Subcommand)]
enum Command {
    /// Show or print configuration.
    #[command(subcommand)]
    Config(config_cmd::ConfigCommand),
}

fn main() -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    match Cli::parse().command {
        Command::Config(command) => config_cmd::run(command, &mut stdout),
    }
}
