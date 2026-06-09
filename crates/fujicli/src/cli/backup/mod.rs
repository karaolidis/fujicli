use anyhow::Context;
use clap::Subcommand;

use super::common::file::{Input, Output};
use crate::cli::{GlobalOptions, common::usb};

#[derive(Subcommand, Debug, Clone)]
pub enum BackupCmd {
    /// Export backup
    #[command(alias = "e")]
    Export {
        /// Output file (use '-' to write to stdout)
        output: Output,
    },

    /// Import backup
    #[command(alias = "i")]
    Import {
        /// Input file (use '-' to read from stdin)
        input: Input,
    },
}

#[allow(clippy::needless_pass_by_value)]
fn handle_export(options: GlobalOptions, output: Output) -> anyhow::Result<()> {
    let GlobalOptions {
        device, emulate, ..
    } = options;

    let mut camera = usb::get_camera(device, emulate)?;

    let backup = camera.export_backup().context("exporting camera backup")?;
    let mut writer = output.get_writer()?;
    writer
        .write_all(&backup)
        .context("writing backup to output")?;

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn handle_import(options: GlobalOptions, input: Input) -> anyhow::Result<()> {
    let GlobalOptions {
        device, emulate, ..
    } = options;

    let mut camera = usb::get_camera(device, emulate)?;

    let mut reader = input.get_reader()?;
    let mut backup = Vec::new();
    reader
        .read_to_end(&mut backup)
        .context("reading backup from input")?;
    camera
        .import_backup(&backup)
        .context("importing camera backup")?;

    Ok(())
}

pub fn handle(cmd: BackupCmd, options: GlobalOptions) -> anyhow::Result<()> {
    match cmd {
        BackupCmd::Export { output } => handle_export(options, output),
        BackupCmd::Import { input } => handle_import(options, input),
    }
}
