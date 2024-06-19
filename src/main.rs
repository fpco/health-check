use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use pid1::Pid1Settings;

mod cli;
mod line_helper;
mod slack;

fn main() -> Result<()> {
    Pid1Settings::new()
        .enable_log(true)
        .launch()
        .context("pid1: Child process launch failed")?;

    let cli = cli::Cli::parse();
    cli.run()
}
