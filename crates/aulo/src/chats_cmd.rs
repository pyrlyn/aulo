//! `aulo chats ls` and `aulo chats rm`.

use std::io::Write;

use anyhow::{Context, Result};
use aulo_server::DaemonClient;
use clap::Subcommand;

use crate::runtime::{home, runtime};

#[derive(Debug, Subcommand)]
pub enum ChatsCommand {
    /// List chats, newest activity first.
    Ls,
    /// Delete a chat and its messages.
    Rm {
        /// Chat id.
        chat_id: String,
    },
}

pub fn run(command: ChatsCommand, out: &mut impl Write) -> Result<()> {
    let home = home()?;
    let rt = runtime()?;
    match command {
        ChatsCommand::Ls => {
            let list = rt.block_on(async {
                let client = DaemonClient::connect(&home).await?;
                client.list_chats().await
            })?;
            if list.chats.is_empty() {
                writeln!(out, "no chats").context("cannot write to stdout")?;
                return Ok(());
            }
            for chat in list.chats {
                writeln!(out, "{}\t{}\t{}", chat.id, chat.title, chat.model)
                    .context("cannot write to stdout")?;
            }
            if list.truncated {
                writeln!(out, "stopped; the list is capped").context("cannot write to stdout")?;
            }
            Ok(())
        }
        ChatsCommand::Rm { chat_id } => {
            rt.block_on(async {
                let client = DaemonClient::connect(&home).await?;
                client.delete_chat(&chat_id).await
            })?;
            writeln!(out, "removed {chat_id}").context("cannot write to stdout")?;
            Ok(())
        }
    }
}
