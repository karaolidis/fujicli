use std::{error::Error, fmt};

use clap::Subcommand;
use serde::Serialize;

use crate::usb;

#[derive(Subcommand, Debug)]
pub enum DeviceCmd {
    /// List devices
    #[command(alias = "l")]
    List,

    /// Dump device info
    #[command(alias = "i")]
    Info,
}

#[derive(Serialize)]
pub struct DeviceItemRepr {
    pub name: String,
    pub id: String,
    pub vendor_id: String,
    pub product_id: String,
}

impl From<&usb::Device> for DeviceItemRepr {
    fn from(device: &usb::Device) -> Self {
        DeviceItemRepr {
            id: device.id(),
            name: device.name(),
            vendor_id: format!("0x{:04x}", device.vendor_id()),
            product_id: format!("0x{:04x}", device.product_id()),
        }
    }
}

impl fmt::Display for DeviceItemRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}:{}) (ID: {})",
            self.name, self.vendor_id, self.product_id, self.id
        )
    }
}

pub fn handle_list(json: bool) -> Result<(), Box<dyn Error + Send + Sync>> {
    let devices: Vec<DeviceItemRepr> = usb::get_connected_devices()?
        .iter()
        .map(|d| d.into())
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&devices)?);
        return Ok(());
    }

    if devices.is_empty() {
        println!("No supported devices connected.");
        return Ok(());
    }

    println!("Connected devices:");
    for d in devices {
        println!("- {}", d);
    }

    Ok(())
}

#[derive(Serialize)]
pub struct DeviceRepr {
    #[serde(flatten)]
    pub device: DeviceItemRepr,

    pub manufacturer: String,
    pub model: String,
    pub device_version: String,
    pub serial_number: String,
}

impl DeviceRepr {
    pub fn from_info(device: &usb::Device, info: &libptp::DeviceInfo) -> Self {
        DeviceRepr {
            device: device.into(),
            manufacturer: info.Manufacturer.clone(),
            model: info.Model.clone(),
            device_version: info.DeviceVersion.clone(),
            serial_number: info.SerialNumber.clone(),
        }
    }
}

impl fmt::Display for DeviceRepr {
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
        writeln!(f, "Device Version: {}", self.device_version)?;
        write!(f, "Serial Number: {}", self.serial_number)
    }
}

pub fn handle_info(
    json: bool,
    device_id: Option<&str>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let device = usb::get_device(device_id)?;

    let mut camera = device.camera()?;
    let info = device.model.get_device_info(&mut camera)?;
    let repr = DeviceRepr::from_info(&device, &info);

    if json {
        println!("{}", serde_json::to_string_pretty(&repr)?);
        return Ok(());
    }

    println!("{}", repr);
    Ok(())
}

pub fn handle(
    cmd: DeviceCmd,
    json: bool,
    device_id: Option<&str>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match cmd {
        DeviceCmd::List => handle_list(json),
        DeviceCmd::Info => handle_info(json, device_id),
    }
}
