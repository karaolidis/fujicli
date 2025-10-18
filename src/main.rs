#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_docs_in_private_items)]

use clap::Parser;
use cli::Commands;

mod camera;
mod cli;
mod log;
mod usb;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();

    log::init(cli.verbose)?;

    let device_id = cli.device.as_deref();

    match cli.command {
        Commands::Device(device_cmd) => cli::device::handle(device_cmd, cli.json, device_id)?,
        Commands::Backup(backup_cmd) => cli::backup::handle(backup_cmd, device_id)?,
        Commands::Simulation(simulation_cmd) => {
            cli::simulation::handle(simulation_cmd, cli.json, device_id)?;
        }
        Commands::Render(_) => todo!(),
    }

    Ok(())
}
