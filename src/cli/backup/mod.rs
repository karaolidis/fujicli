use std::error::Error;

use crate::usb;

use super::common::file::{Input, Output};
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum BackupCmd {
    /// Export backup
    #[command(alias = "e")]
    Export {
        /// Output file (use '-' to write to stdout)
        output_file: Output,
    },

    /// Import backup
    #[command(alias = "i")]
    Import {
        /// Input file (use '-' to read from stdin)
        input_file: Input,
    },
}

fn handle_export(
    device_id: Option<&str>,
    output: Output,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let camera = usb::get_camera(device_id)?;

    let mut writer = output.get_writer()?;

    todo!();

    Ok(())
}

fn handle_import(
    device_id: Option<&str>,
    input: Input,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let camera = usb::get_camera(device_id)?;

    let mut reader = input.get_reader()?;

    todo!();

    Ok(())
}

pub fn handle(cmd: BackupCmd, device_id: Option<&str>) -> Result<(), Box<dyn Error + Send + Sync>> {
    match cmd {
        BackupCmd::Export { output_file } => handle_export(device_id, output_file),
        BackupCmd::Import { input_file } => handle_import(device_id, input_file),
    }
}
