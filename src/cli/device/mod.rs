use std::fmt;

use clap::Subcommand;
use serde::Serialize;

use crate::{
    hardware::{CameraImpl, UsbMode},
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
pub struct CameraItemRepr {
    pub name: String,
    pub id: String,
    pub vendor_id: String,
    pub product_id: String,
}

impl From<&Box<dyn CameraImpl>> for CameraItemRepr {
    fn from(camera: &Box<dyn CameraImpl>) -> Self {
        Self {
            id: camera.usb_id(),
            name: camera.id().name.to_string(),
            vendor_id: format!("0x{:04x}", camera.id().vendor),
            product_id: format!("0x{:04x}", camera.id().product),
        }
    }
}

impl fmt::Display for CameraItemRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}:{}) (ID: {})",
            self.name, self.vendor_id, self.product_id, self.id
        )
    }
}

fn handle_list(json: bool) -> Result<(), anyhow::Error> {
    let cameras: Vec<CameraItemRepr> = usb::get_connected_camers()?
        .iter()
        .map(std::convert::Into::into)
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&cameras)?);
        return Ok(());
    }

    if cameras.is_empty() {
        println!("No supported cameras connected.");
        return Ok(());
    }

    println!("Connected cameras:");
    for d in cameras {
        println!("- {d}");
    }

    Ok(())
}

#[derive(Serialize)]
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
        writeln!(f, "ID: {}", self.device.id)?;
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

fn handle_info(json: bool, device_id: Option<&str>) -> Result<(), anyhow::Error> {
    let camera = usb::get_camera(device_id)?;
    let mut ptp = camera.ptp();

    let info = camera.get_info(&mut ptp)?;

    let mut ptp = camera.open_session(ptp)?;
    let mode = camera.get_usb_mode(&mut ptp)?;
    let battery = camera.get_battery_info(&mut ptp)?;

    let repr = CameraRepr {
        device: (&camera).into(),
        manufacturer: info.Manufacturer.clone(),
        model: info.Model.clone(),
        device_version: info.DeviceVersion.clone(),
        serial_number: info.SerialNumber,
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

pub fn handle(cmd: DeviceCmd, json: bool, device_id: Option<&str>) -> Result<(), anyhow::Error> {
    match cmd {
        DeviceCmd::List => handle_list(json),
        DeviceCmd::Info => handle_info(json, device_id),
    }
}
