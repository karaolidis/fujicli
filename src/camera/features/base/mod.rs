pub mod info;

use anyhow::anyhow;
use info::{CameraInfo, DefaultCameraInfo};
use log::debug;

use crate::camera::{
    SupportedCamera,
    ptp::{Ptp, hex::DevicePropCode},
};

use super::{backup::CameraBackups, simulation::CameraSimulations};

pub trait CameraBase {
    type Context: rusb::UsbContext;

    fn camera_definition(&self) -> &'static SupportedCamera;

    fn chunk_size(&self) -> usize {
        // Default conservative estimate.
        1024 * 1024
    }

    fn as_simulations(&self) -> Option<&dyn CameraSimulations<Context = Self::Context>> {
        None
    }

    fn as_backups(&self) -> Option<&dyn CameraBackups<Context = Self::Context>> {
        None
    }

    // NOTE: Naively assuming that all cameras can get the same info in the same way.
    // The default function should be removed if this is not the case.
    fn get_info(&self, ptp: &mut Ptp) -> anyhow::Result<Box<dyn CameraInfo>> {
        let info = ptp.get_info()?;

        let mode = ptp.get_prop(DevicePropCode::FujiUsbMode)?;

        let battery_string: String = ptp.get_prop(DevicePropCode::FujiBatteryInfo2)?;
        debug!("Raw battery string: {battery_string}");

        let battery: u32 = battery_string
            .split(',')
            .next()
            .ok_or_else(|| anyhow!("Failed to parse battery percentage"))?
            .parse()?;

        let repr = DefaultCameraInfo {
            manufacturer: info.manufacturer,
            model: info.model,
            device_version: info.device_version,
            serial_number: info.serial_number,
            mode,
            battery,
        };

        Ok(Box::new(repr))
    }
}
