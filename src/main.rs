use std::error::Error;

use clap::Parser;
use cli::Commands;

mod cli;
mod hardware;
mod log;
mod usb;

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let cli = cli::Cli::parse();

    log::init(cli.quiet, cli.verbose)?;

    match cli.command {
        Commands::Device(device_cmd) => {
            cli::device::handle(device_cmd, cli.json, cli.device.as_deref())?
        }
        _ => todo!(),
    }

    Ok(())
}
