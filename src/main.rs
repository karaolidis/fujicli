#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_docs_in_private_items)]

use clap::Parser;
use cli::Commands;

mod cli;
mod log;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let options = cli.options;

    log::init(options.verbose)?;

    match cli.command {
        Commands::Device(device_cmd) => cli::device::handle(device_cmd, &options)?,
        Commands::Backup(backup_cmd) => cli::backup::handle(backup_cmd, &options)?,
        Commands::Simulation(simulation_cmd) => {
            cli::simulation::handle(simulation_cmd, &options)?;
        }
        Commands::Render(render_cmd) => cli::render::handle(render_cmd, &options)?,
    }

    Ok(())
}
