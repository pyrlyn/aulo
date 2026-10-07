//! `aulo bench`: measure speech engines.

use std::io::Write;

use anyhow::{Context, Result, bail};
use aulo_config::aulo_home;
use aulo_speech_bench::{
    Keys, Workload, discover, render_json, render_table, run as run_bench, with_main_run_loop,
};
use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum BenchCommand {
    /// Word error rate, real-time factor and time to first audio per speech engine.
    ///
    /// Runs every engine that is available here on the built-in clips: Parakeet
    /// and Kokoro when their models are installed, the system voices, and the
    /// OpenAI, Deepgram and ElevenLabs engines when OPENAI_API_KEY,
    /// DEEPGRAM_API_KEY or ELEVENLABS_API_KEY is set (the only case that uses
    /// the network). An engine that cannot run gets a row with the reason.
    Speech(SpeechArgs),
}

#[derive(Debug, Args)]
pub struct SpeechArgs {
    /// Run only the engine with this id, for example `sherpa-parakeet`.
    #[arg(long)]
    engine: Option<String>,
    /// Print JSON instead of a table.
    #[arg(long)]
    json: bool,
}

pub fn run(command: BenchCommand, out: &mut impl Write) -> Result<()> {
    let BenchCommand::Speech(args) = command;
    let home = aulo_home().context("cannot determine AULO_HOME or the home directory")?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .context("cannot start the async runtime")?;
    let workload = Workload::builtin().map_err(anyhow::Error::msg)?;
    let candidates = discover::builtin(&home, runtime.handle(), &Keys::from_env());
    if let Some(id) = &args.engine
        && !candidates.iter().any(|c| &c.id == id)
    {
        bail!("no speech engine `{id}`; run `aulo bench speech` to list them");
    }
    let rows = with_main_run_loop(|| run_bench(candidates, args.engine.as_deref(), &workload));
    let text = if args.json {
        render_json(&rows)
    } else {
        render_table(&rows)
    };
    out.write_all(text.as_bytes())
        .context("cannot write to stdout")
}
