use anyhow::{Context, anyhow};
use clap::Subcommand;
use fujicore::{
    Camera,
    features::backup::{self, manager::BackupObjectInfo},
    generated::{
        cli::SIMULATION_PROP_CODES,
        options::{CustomSetting, UsbMode},
    },
    ptp::{CommandCode, DevicePropCode, option::SimulationSetting},
};
use log::{debug, error};
use ptp_cursor::PtpSerialize;
use strum::IntoEnumIterator;

use crate::cli::{
    GlobalOptions,
    backup::BackupCmd,
    common::{
        file::{Input, Output},
        usb,
    },
};

#[derive(Subcommand, Debug, Clone)]
pub enum ReverseCmd {
    /// Attempt to manage backups
    #[command(alias = "b", subcommand)]
    Backup(BackupCmd),

    /// Attempt to get camera info
    #[command(alias = "i")]
    Info,

    /// Get information about supported simulation management commands
    #[command(alias = "s")]
    Simulation,
}

macro_rules! try_call {
    ($call:expr $(,)?) => {{
        let result = $call;
        match &result {
            Ok(value) => debug!("{}: {:?}", stringify!($call), value),
            Err(error) => error!("{}: {}", stringify!($call), error),
        }
        result
    }};
}

#[allow(clippy::needless_pass_by_value)]
fn handle_backup_export(options: GlobalOptions, output: Output) -> anyhow::Result<()> {
    let GlobalOptions { device, .. } = options;

    let location = device.ok_or_else(|| anyhow!("Device must be specified for backup export"))?;
    let usb = usb::get_usb_device_by_location(location)?;
    let mut camera = Camera::open_unknown(&usb).context("opening device as unknown camera")?;

    try_call!(camera.ptp.send(
        CommandCode::GetObjectInfo,
        &backup::EXPORT_OBJECT_INFO_HANDLE,
        None
    ))?;
    let backup = try_call!(
        camera
            .ptp
            .send(CommandCode::GetObject, &backup::OBJECT_HANDLE, None)
    )?;
    let mut writer = output.get_writer()?;
    writer
        .write_all(&backup)
        .context("writing backup to output")?;

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn handle_backup_import(options: GlobalOptions, input: Input) -> anyhow::Result<()> {
    let GlobalOptions { device, .. } = options;

    let location = device.ok_or_else(|| anyhow!("Device must be specified for backup import"))?;
    let usb = usb::get_usb_device_by_location(location)?;
    let mut camera = Camera::open_unknown(&usb).context("opening device as unknown camera")?;

    let mut reader = input.get_reader()?;
    let mut backup = Vec::new();
    reader
        .read_to_end(&mut backup)
        .context("reading backup from input")?;

    let backup_info = BackupObjectInfo::new(backup.len()).context("preparing backup envelope")?;
    let backup_info_bytes = backup_info
        .try_into_ptp()
        .context("serializing backup envelope")?;

    try_call!(camera.ptp.send(
        CommandCode::SendObjectInfo,
        &backup::IMPORT_OBJECT_INFO_HANDLE,
        Some(&backup_info_bytes),
    ))?;
    try_call!(camera.ptp.send(
        CommandCode::SendObject,
        &backup::OBJECT_HANDLE,
        Some(&backup)
    ))?;

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn handle_info(options: GlobalOptions) -> anyhow::Result<()> {
    let GlobalOptions { device, .. } = options;

    let location = device.ok_or_else(|| anyhow!("Device must be specified for info dump"))?;
    let usb = usb::get_usb_device_by_location(location)?;
    let mut camera = Camera::open_unknown(&usb).context("opening device as unknown camera")?;

    let _ = try_call!(camera.ptp.get_info());
    let _ = try_call!(camera.ptp.get_prop_raw(UsbMode::prop_code()));
    let _ = try_call!(camera.ptp.get_prop_raw(DevicePropCode::FujiBatteryInfo2));

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn handle_simulation(options: GlobalOptions) -> anyhow::Result<()> {
    let GlobalOptions { device, .. } = options;

    let location =
        device.ok_or_else(|| anyhow!("Device must be specified for simulation prop dump"))?;
    let usb = usb::get_usb_device_by_location(location)?;
    let mut camera = Camera::open_unknown(&usb).context("opening device as unknown camera")?;

    for slot in CustomSetting::iter() {
        if try_call!(slot.try_push(&mut camera.ptp)).is_err() {
            continue;
        }

        for &code in SIMULATION_PROP_CODES {
            let _ = try_call!(camera.ptp.get_prop_raw(code));
        }
    }

    Ok(())
}

pub fn handle(cmd: ReverseCmd, options: GlobalOptions) -> anyhow::Result<()> {
    match cmd {
        ReverseCmd::Backup(BackupCmd::Export { output }) => handle_backup_export(options, output),
        ReverseCmd::Backup(BackupCmd::Import { input }) => handle_backup_import(options, input),
        ReverseCmd::Info => handle_info(options),
        ReverseCmd::Simulation => handle_simulation(options),
    }
}
