use std::fmt;

use clap::Subcommand;
use serde::Serialize;

use crate::{
    camera::{Camera, ptp::hex::UsbMode},
    usb,
};

#[derive(Subcommand, Debug, Clone, Copy)]
pub enum DeviceCmd {
    /// List cameras
    #[command(alias = "l")]
    List,

    /// Get camera info
    #[command(alias = "i")]
    Info,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraItemRepr {
    pub name: &'static str,
    pub usb_id: String,
    pub vendor_id: String,
    pub product_id: String,
}

impl From<&Camera> for CameraItemRepr {
    fn from(camera: &Camera) -> Self {
        Self {
            name: camera.name(),
            usb_id: camera.connected_usb_id(),
            vendor_id: format!("0x{:04x}", camera.vendor_id()),
            product_id: format!("0x{:04x}", camera.product_id()),
        }
    }
}

impl fmt::Display for CameraItemRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}:{}) (USB ID: {})",
            self.name, self.vendor_id, self.product_id, self.usb_id
        )
    }
}

fn handle_list(json: bool) -> anyhow::Result<()> {
    let cameras: Vec<CameraItemRepr> = usb::get_connected_cameras()?
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

    println!("Connected Cameras:");
    for d in cameras {
        println!("- {d}");
    }

    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraRepr {
    #[serde(flatten)]
    pub device: CameraItemRepr,

    pub manufacturer: String,
    pub model: String,
    pub device_version: String,
    pub serial_number: String,
    pub mode: UsbMode,
    pub battery: u32,
}

impl fmt::Display for CameraRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Name: {}", self.device.name)?;
        writeln!(f, "USB ID: {}", self.device.usb_id)?;
        writeln!(
            f,
            "Vendor ID: {}, Product ID: {}",
            self.device.vendor_id, self.device.product_id
        )?;
        writeln!(f, "Manufacturer: {}", self.manufacturer)?;
        writeln!(f, "Model: {}", self.model)?;
        writeln!(f, "Version: {}", self.device_version)?;
        writeln!(f, "Serial Number: {}", self.serial_number)?;
        writeln!(f, "Mode: {}", self.mode)?;
        write!(f, "Battery: {}%", self.battery)
    }
}

fn handle_info(json: bool, device_id: Option<&str>) -> anyhow::Result<()> {
    let mut camera = usb::get_camera(device_id)?;

    let info = camera.get_info()?;
    let mode = camera.get_usb_mode()?;
    let battery = camera.get_battery_info()?;

    let repr = CameraRepr {
        device: (&camera).into(),
        manufacturer: info.manufacturer.clone(),
        model: info.model.clone(),
        device_version: info.device_version.clone(),
        serial_number: info.serial_number,
        mode,
        battery,
    };

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
