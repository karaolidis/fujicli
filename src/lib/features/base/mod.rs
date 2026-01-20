pub mod info;

use anyhow::anyhow;
use info::{CameraInfo, DefaultCameraInfo};
use log::debug;

use crate::{
    SupportedCamera,
    features::{
        backup::CameraBackupManager,
        render::CameraRenderManager,
        simulation::{manager::CameraSimulationManager, parser::CameraSimulationParser},
    },
    ptp::{DevicePropCode, Ptp},
};

pub trait CameraBase {
    type Context: rusb::UsbContext;

    fn camera_definition(&self) -> &'static SupportedCamera;

    fn chunk_size(&self) -> usize {
        // Default conservative estimate.
        1024 * 1024
    }

    fn as_backup_manager(&self) -> Option<&dyn CameraBackupManager<Context = Self::Context>> {
        None
    }

    fn as_simulation_parser(&self) -> Option<&dyn CameraSimulationParser> {
        None
    }

    fn as_simulation_manager(
        &self,
    ) -> Option<&dyn CameraSimulationManager<Context = Self::Context>> {
        None
    }

    fn as_render_manager(&self) -> Option<&dyn CameraRenderManager<Context = Self::Context>> {
        None
    }

    // NOTE: Naively assuming that all cameras can get the same info in the same way.
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
