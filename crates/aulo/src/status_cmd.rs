//! `aulo status`: is the local daemon up?

use std::io::Write;

use anyhow::{Context, Result, bail};
use aulo_server::{DaemonClient, DaemonStatus};

use crate::runtime::{home, runtime};

pub fn run(out: &mut impl Write) -> Result<()> {
    let home = home()?;
    let status = runtime()?.block_on(async {
        let client = DaemonClient::connect(&home).await?;
        client.status().await
    })?;
    match status {
        DaemonStatus::Serving => {
            writeln!(out, "serving").context("cannot write to stdout")?;
            Ok(())
        }
        DaemonStatus::NotServing => bail!("aulod is not serving"),
    }
}
