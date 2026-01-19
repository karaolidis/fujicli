use std::{thread::sleep, time::Duration};

use clap::Subcommand;
use log::{error, info};
use ptp_cursor::{PtpDeserialize, PtpSerialize};
use strum::IntoEnumIterator;

use crate::cli::{GlobalOptions, common::file::Input};
use fujicli::{
    features::{backup::ptp::FujiBackupObjectInfo, base::info::CameraInfoListItem},
    ptp::{
        hex::{CommandCode, DevicePropCode, FujiCustomSetting, ObjectFormat},
        structs::ObjectInfo,
    },
    usb,
};

#[derive(Subcommand, Debug, Clone)]
pub enum DeviceCmd {
    /// List cameras
    #[command(alias = "l")]
    List,

    /// Get camera info
    #[command(alias = "i")]
    Info,

    /// Dump camera details for debugging purposes
    #[command(alias = "d")]
    Dump {
        /// Optional RAF input file to test rendering (use '-' to read from stdin)
        input: Option<Input>,
    },
}

fn handle_list(options: &GlobalOptions) -> anyhow::Result<()> {
    let GlobalOptions { json, .. } = options;

    let cameras: Vec<CameraInfoListItem> = usb::get_connected_cameras()?
        .iter()
        .map(std::convert::Into::into)
        .collect();

    if *json {
        println!("{}", serde_json::to_string_pretty(&cameras)?);
        return Ok(());
    }

    if cameras.is_empty() {
        println!("No supported cameras connected");
        return Ok(());
    }

    for d in cameras {
        println!("- {d}");
    }

    Ok(())
}

fn handle_info(options: &GlobalOptions) -> anyhow::Result<()> {
    let GlobalOptions {
        json,
        device,
        emulate,
        ..
    } = options;

    let mut camera = usb::get_camera(device.as_deref(), emulate.as_deref())?;

    let repr = camera.get_info()?;

    if *json {
        println!("{}", serde_json::to_string_pretty(&repr)?);
        return Ok(());
    }

    println!("{repr}");
    Ok(())
}

macro_rules! try_call {
    ($call:expr $(,)?) => {{
        let result = $call;
        match &result {
            Ok(value) => info!("{}: {:?}", stringify!($call), value),
            Err(error) => error!("{}: {}", stringify!($call), error),
        }
        result
    }};
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
fn handle_dump(options: &GlobalOptions, input: Option<Input>) -> anyhow::Result<()> {
    let GlobalOptions {
        device, emulate, ..
    } = options;

    let mut camera = usb::get_camera(device.as_deref(), emulate.as_deref())?;

    const BACKUP_HANDLE: u32 = 0x0;
    try_call!(
        camera
            .ptp
            .send(CommandCode::GetObjectInfo, &[BACKUP_HANDLE], None)
    )?;
    let backup = try_call!(
        camera
            .ptp
            .send(CommandCode::GetObject, &[BACKUP_HANDLE], None)
    )?;

    let backup_info = FujiBackupObjectInfo::new(backup.len())?;

    try_call!(camera.ptp.send(
        CommandCode::SendObjectInfo,
        &[0x0, 0x0],
        Some(&backup_info.try_into_ptp()?),
    ))?;
    try_call!(
        camera
            .ptp
            .send(CommandCode::SendObject, &[0x0], Some(&backup))
    )?;

    let _ = try_call!(camera.ptp.get_info());
    let _ = try_call!(camera.ptp.get_prop_raw(DevicePropCode::FujiUsbMode));
    let _ = try_call!(camera.ptp.get_prop_raw(DevicePropCode::FujiBatteryInfo2));

    for slot in FujiCustomSetting::iter() {
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

    'render: {
        if let Some(input) = input {
            let mut reader = input.get_reader()?;
            let mut image = Vec::new();
            reader.read_to_end(&mut image)?;

            let object_info = ObjectInfo {
                object_format: ObjectFormat::FujiRAF,
                compressed_size: u32::try_from(image.len())?,
                filename: String::from("FUP_FILE.dat"),
                ..Default::default()
            };

            if try_call!(camera.ptp.send(
                CommandCode::FujiSendObjectInfo,
                &[0x0, 0x0, 0x0],
                Some(&object_info.try_into_ptp()?),
            ))
            .is_err()
            {
                break 'render;
            }

            if try_call!(
                camera
                    .ptp
                    .send(CommandCode::FujiSendObject, &[], Some(&image))
            )
            .is_err()
            {
                break 'render;
            }

            let Ok(profile) = try_call!(
                camera
                    .ptp
                    .get_prop_raw(DevicePropCode::FujiRawConversionProfile)
            ) else {
                break 'render;
            };

            if try_call!(
                camera
                    .ptp
                    .set_prop(DevicePropCode::FujiRawConversionProfile, &profile)
            )
            .is_err()
            {
                break 'render;
            }

            if try_call!(
                camera
                    .ptp
                    .set_prop(DevicePropCode::FujiRawConversionRun, &1u16)
            )
            .is_err()
            {
                break 'render;
            }

            let handle;
            loop {
                let Ok(raw) = try_call!(camera.ptp.send(
                    CommandCode::GetObjectHandles,
                    &[u32::MAX, 0, 0],
                    None
                )) else {
                    break 'render;
                };

                let response = <Vec<u32>>::try_from_ptp(&raw)?;
                if !response.is_empty() {
                    handle = response[0];
                    break;
                }

                sleep(Duration::from_millis(100));
            }

            if try_call!(camera.ptp.send(CommandCode::GetObject, &[handle], None)).is_err() {
                break 'render;
            }
            if try_call!(camera.ptp.send(CommandCode::DeleteObject, &[handle], None)).is_err() {
                break 'render;
            }
        }
    }

    try_call!(camera.ptp.send(
        CommandCode::SendObjectInfo,
        &[0x0, 0x0],
        Some(&backup_info.try_into_ptp()?),
    ))?;
    try_call!(
        camera
            .ptp
            .send(CommandCode::SendObject, &[0x0], Some(&backup))
    )?;

    Ok(())
}

pub fn handle(cmd: DeviceCmd, options: &GlobalOptions) -> anyhow::Result<()> {
    match cmd {
        DeviceCmd::List => handle_list(options),
        DeviceCmd::Info => handle_info(options),
        DeviceCmd::Dump { input } => handle_dump(options, input),
    }
}
