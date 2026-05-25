#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_docs_in_private_items, clippy::similar_names)]

use clap::Parser;

mod cli;
mod log;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    log::init(cli.options.verbose)?;
    cli::handle(cli)?;
    Ok(())
}
