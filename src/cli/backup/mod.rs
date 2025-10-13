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

fn handle_export(device_id: Option<&str>, output: &Output) -> Result<(), anyhow::Error> {
    let camera = usb::get_camera(device_id)?;
    let mut ptp = camera.ptp_session()?;

    let mut writer = output.get_writer()?;
    let backup = camera.export_backup(&mut ptp)?;
    writer.write_all(&backup)?;

    Ok(())
}

fn handle_import(device_id: Option<&str>, input: &Input) -> Result<(), anyhow::Error> {
    let camera = usb::get_camera(device_id)?;
    let mut ptp = camera.ptp_session()?;

    let mut reader = input.get_reader()?;
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    camera.import_backup(&mut ptp, &buffer)?;

    Ok(())
}

pub fn handle(cmd: BackupCmd, device_id: Option<&str>) -> Result<(), anyhow::Error> {
    match cmd {
        BackupCmd::Export { output_file } => handle_export(device_id, &output_file),
        BackupCmd::Import { input_file } => handle_import(device_id, &input_file),
    }
}
