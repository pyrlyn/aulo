//! The `aulo` command line. Parsing and error presentation live here; what a
//! command does lives in the library crates it calls.

mod auth_cmd;
mod bench_cmd;
mod chat_cmd;
mod chats_cmd;
mod config_cmd;
mod runtime;
mod status_cmd;

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
    /// Measure the speech engines on the fixture clips.
    #[command(subcommand)]
    Bench(bench_cmd::BenchCommand),
    /// Show or print configuration.
    #[command(subcommand)]
    Config(config_cmd::ConfigCommand),
    /// Store, remove or list provider API keys. Values are never printed.
    #[command(subcommand)]
    Auth(auth_cmd::AuthCommand),
    /// Show whether the local daemon is up.
    Status,
    /// List or delete chats.
    #[command(subcommand)]
    Chats(chats_cmd::ChatsCommand),
    /// Talk to the daemon in this terminal.
    Chat {
        /// Existing chat to continue. Omit to start a new one.
        #[arg(long, value_name = "ID")]
        chat: Option<String>,
    },
}

fn main() -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    match Cli::parse().command {
        Command::Bench(command) => bench_cmd::run(command, &mut stdout),
        Command::Config(command) => config_cmd::run(command, &mut stdout),
        Command::Auth(command) => auth_cmd::run(command, &mut stdout),
        Command::Status => status_cmd::run(&mut stdout),
        Command::Chats(command) => chats_cmd::run(command, &mut stdout),
        Command::Chat { chat } => chat_cmd::run(chat, &mut stdout),
    }
}
