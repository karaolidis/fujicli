#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_docs_in_private_items)]

use clap::Parser;
use cli::Commands;

mod cli;
mod hardware;
mod log;
mod usb;

fn main() -> Result<(), anyhow::Error> {
    let cli = cli::Cli::parse();

    log::init(cli.quiet, cli.verbose)?;

    let device_id = cli.device.as_deref();

    match cli.command {
        Commands::Device(device_cmd) => cli::device::handle(device_cmd, cli.json, device_id)?,
        Commands::Backup(backup_cmd) => cli::backup::handle(backup_cmd, device_id)?,
        _ => todo!(),
    }

    Ok(())
}
