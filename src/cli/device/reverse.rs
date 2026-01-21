use anyhow::anyhow;
use clap::Subcommand;
use fujicli::{
    Camera,
    features::backup,
    ptp::{CommandCode, DevicePropCode, fuji},
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
    let mut camera = Camera::open_unknown(&usb)?;

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
    let GlobalOptions { device, .. } = options;

    let location = device.ok_or_else(|| anyhow!("Device must be specified for backup import"))?;
    let usb = usb::get_usb_device_by_location(location)?;
    let mut camera = Camera::open_unknown(&usb)?;

    let mut reader = input.get_reader()?;
    let mut backup = Vec::new();
    reader.read_to_end(&mut backup)?;

    let backup_info = fuji::BackupObjectInfo::new(backup.len())?;

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
    let GlobalOptions { device, .. } = options;

    let location = device.ok_or_else(|| anyhow!("Device must be specified for info dump"))?;
    let usb = usb::get_usb_device_by_location(location)?;
    let mut camera = Camera::open_unknown(&usb)?;

    let _ = try_call!(camera.ptp.get_info());
    let _ = try_call!(camera.ptp.get_prop_raw(DevicePropCode::FujiUsbMode));
    let _ = try_call!(camera.ptp.get_prop_raw(DevicePropCode::FujiBatteryInfo2));

    Ok(())
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
#[allow(clippy::needless_pass_by_value)]
fn handle_simulation(options: GlobalOptions) -> anyhow::Result<()> {
    let GlobalOptions { device, .. } = options;

    let location =
        device.ok_or_else(|| anyhow!("Device must be specified for simulation prop dump"))?;
    let usb = usb::get_usb_device_by_location(location)?;
    let mut camera = Camera::open_unknown(&usb)?;

    for slot in fuji::CustomSetting::iter() {
        if try_call!(
            camera
                .ptp
                .set_prop(DevicePropCode::FujiCustomSetting, &slot)
        )
        .is_err()
        {
            continue;
        }

        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingName)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingImageSize)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingImageQuality)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingDynamicRange)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingDynamicRangePriority)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingFilmSimulation)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingMonochromaticColorTemperature)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingMonochromaticColorTint)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingGrainEffect)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingColorChromeEffect)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingColorChromeFXBlue)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingSmoothSkinEffect)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingWhiteBalance)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingWhiteBalanceShiftRed)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingWhiteBalanceShiftBlue)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingWhiteBalanceTemperature)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingHighlightTone)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingShadowTone)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingColor)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingSharpness)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingHighISONR)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingClarity)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingLensModulationOptimizer)
        );
        let _ = try_call!(
            camera
                .ptp
                .get_prop_raw(DevicePropCode::FujiCustomSettingColorSpace)
        );
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
