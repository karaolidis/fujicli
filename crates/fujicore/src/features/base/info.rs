use std::fmt;

use erased_serde::serialize_trait_object;
use serde::Serialize;

use crate::{SupportedCamera, UsbId, generated::options::UsbMode, ptp::transport::Device};

pub trait CameraInfo: fmt::Display + erased_serde::Serialize {}
serialize_trait_object!(CameraInfo);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultCameraInfo {
    pub manufacturer: String,
    pub model: String,
    pub device_version: String,
    pub serial_number: String,
    pub mode: UsbMode,
    pub battery: u32,
}

impl fmt::Display for DefaultCameraInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Manufacturer: {}", self.manufacturer)?;
        writeln!(f, "Model: {}", self.model)?;
        writeln!(f, "Version: {}", self.device_version)?;
        writeln!(f, "Serial Number: {}", self.serial_number)?;
        writeln!(f, "Mode: {}", self.mode)?;
        write!(f, "Battery: {}%", self.battery)
    }
}

impl CameraInfo for DefaultCameraInfo {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraInfoListItem {
    pub name: &'static str,
    pub usb_id: UsbId,
    pub serial_number: Option<String>,
}

impl CameraInfoListItem {
    #[must_use]
    pub fn from_device(device: &Device) -> Option<Self> {
        let camera = SupportedCamera::find(device.usb_id())?;
        Some(Self {
            name: camera.name,
            usb_id: camera.usb_id,
            serial_number: device.serial().map(str::to_owned),
        })
    }
}

impl fmt::Display for CameraInfoListItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.usb_id)?;
        if let Some(serial) = &self.serial_number {
            write!(f, " (serial {serial})")?;
        }
        Ok(())
    }
}
