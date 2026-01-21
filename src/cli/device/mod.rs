mod reverse;

use clap::Subcommand;

use crate::cli::{GlobalOptions, common::usb, device::reverse::ReverseCmd};
use fujicli::features::base::info::CameraInfoListItem;

#[derive(Subcommand, Debug, Clone)]
pub enum DeviceCmd {
    /// List cameras
    #[command(alias = "l")]
    List,

    /// Get camera info
    #[command(alias = "i")]
    Info,

    /// Reverse engineer device communication
    ///
    /// Only run this if you have a full device backup and know what
    /// you are doing. Misuse can corrupt your camera or void your warranty.
    #[command(alias = "r", subcommand, hide = true)]
    Reverse(ReverseCmd),
}

#[allow(clippy::needless_pass_by_value)]
fn handle_list(options: GlobalOptions) -> anyhow::Result<()> {
    let GlobalOptions { json, .. } = options;

    let cameras: Vec<CameraInfoListItem> = usb::get_all_cameras()?
        .iter()
        .map(std::convert::Into::into)
        .collect();

    if json {
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

#[allow(clippy::needless_pass_by_value)]
fn handle_info(options: GlobalOptions) -> anyhow::Result<()> {
    let GlobalOptions {
        json,
        device,
        emulate,
        ..
    } = options;

    let mut camera = usb::get_camera(device, emulate)?;

    let repr = camera.get_info()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&repr)?);
        return Ok(());
    }

    println!("{repr}");
    Ok(())
}

pub fn handle(cmd: DeviceCmd, options: GlobalOptions) -> anyhow::Result<()> {
    match cmd {
        DeviceCmd::List => handle_list(options),
        DeviceCmd::Info => handle_info(options),
        DeviceCmd::Reverse(cmd) => reverse::handle(cmd, options),
    }
}
