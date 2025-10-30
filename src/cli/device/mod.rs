use clap::Subcommand;

use crate::{camera::features::base::info::CameraInfoListItem, usb};

#[derive(Subcommand, Debug, Clone, Copy)]
pub enum DeviceCmd {
    /// List cameras
    #[command(alias = "l")]
    List,

    /// Get camera info
    #[command(alias = "i")]
    Info,
}

fn handle_list(json: bool) -> anyhow::Result<()> {
    let cameras: Vec<CameraInfoListItem> = usb::get_connected_cameras()?
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

fn handle_info(json: bool, device_id: Option<&str>) -> anyhow::Result<()> {
    let mut camera = usb::get_camera(device_id)?;

    let repr = camera.get_info()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&repr)?);
        return Ok(());
    }

    println!("{repr}");
    Ok(())
}

pub fn handle(cmd: DeviceCmd, json: bool, device_id: Option<&str>) -> anyhow::Result<()> {
    match cmd {
        DeviceCmd::List => handle_list(json),
        DeviceCmd::Info => handle_info(json, device_id),
    }
}
