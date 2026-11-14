mod app;
mod lyrics;
mod player;
mod ui;

use anyhow::{bail, Context, Result};
use clap::Parser;
use std::io::IsTerminal;
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(name = "lyrix", version, about = "Show song lyrics in your terminal")]
struct Args {
    /// Select a playerctl player (for example: spotify, firefox, vlc)
    #[arg(short, long)]
    player: Option<String>,

    /// Adjust synchronized lyrics in milliseconds (negative values show lines earlier)
    #[arg(long, default_value_t = -300)]
    offset_ms: i64,

    /// Media polling interval in milliseconds (100–2000)
    #[arg(long, default_value_t = 250, value_parser = clap::value_parser!(u64).range(100..=2000))]
    interval_ms: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if !std::io::stdin().is_terminal() {
        bail!("lyrix needs an interactive terminal (TTY)");
    }
    Command::new("playerctl")
        .arg("--version")
        .output()
        .context("playerctl is required but was not found; install it with your system package manager")?;

    app::run(args.player, args.offset_ms, Duration::from_millis(args.interval_ms))
}
