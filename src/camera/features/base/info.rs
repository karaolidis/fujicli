use std::fmt;

use erased_serde::serialize_trait_object;
use serde::Serialize;

use crate::camera::{Camera, ptp::hex::FujiUsbMode};

pub trait CameraInfo: fmt::Display + erased_serde::Serialize {}
serialize_trait_object!(CameraInfo);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultCameraInfo {
    pub manufacturer: String,
    pub model: String,
    pub device_version: String,
    pub serial_number: String,
    pub mode: FujiUsbMode,
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
    pub usb_id: String,
    pub vendor_id: String,
    pub product_id: String,
}

impl From<&Camera> for CameraInfoListItem {
    fn from(camera: &Camera) -> Self {
        Self {
            name: camera.name(),
            usb_id: camera.connected_usb_id(),
            vendor_id: format!("0x{:04x}", camera.vendor_id()),
            product_id: format!("0x{:04x}", camera.product_id()),
        }
    }
}

impl fmt::Display for CameraInfoListItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}:{}) (USB ID: {})",
            self.name, self.vendor_id, self.product_id, self.usb_id
        )
    }
}
