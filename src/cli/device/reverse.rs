use clap::Subcommand;
use fujicli::{
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
    let transport = options.transport()?;
    let GlobalOptions { device, .. } = options;
    let mut camera = usb::get_unknown_camera(transport, device.as_deref(), "backup export")?;

    let mut writer = output.get_writer()?;
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
    writer.write_all(&backup)?;

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn handle_backup_import(options: GlobalOptions, input: Input) -> anyhow::Result<()> {
    let transport = options.transport()?;
    let GlobalOptions { device, .. } = options;
    let mut camera = usb::get_unknown_camera(transport, device.as_deref(), "backup import")?;

    let mut reader = input.get_reader()?;
    let mut backup = Vec::new();
    reader.read_to_end(&mut backup)?;

    let backup_info = BackupObjectInfo::new(backup.len())?;

    try_call!(camera.ptp.send(
        CommandCode::SendObjectInfo,
        &backup::IMPORT_OBJECT_INFO_HANDLE,
        Some(&backup_info.try_into_ptp()?),
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
    let transport = options.transport()?;
    let GlobalOptions { device, .. } = options;
    let mut camera = usb::get_unknown_camera(transport, device.as_deref(), "info dump")?;

    let _ = try_call!(camera.ptp.get_info());
    let _ = try_call!(camera.ptp.get_prop_raw(UsbMode::prop_code()));
    let _ = try_call!(camera.ptp.get_prop_raw(DevicePropCode::FujiBatteryInfo2));

    Ok(())
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
#[allow(clippy::needless_pass_by_value)]
fn handle_simulation(options: GlobalOptions) -> anyhow::Result<()> {
    let transport = options.transport()?;
    let GlobalOptions { device, .. } = options;

    let mut camera = usb::get_unknown_camera(transport, device.as_deref(), "simulation prop dump")?;

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
