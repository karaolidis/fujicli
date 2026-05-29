pub mod info;

use info::{CameraInfo, DefaultCameraInfo};
use log::debug;

use crate::{
    SupportedCamera,
    error::{Capability, CoreError, CoreResult},
    features::{
        backup::CameraBackupManager,
        render::CameraRenderManager,
        simulation::{manager::CameraSimulationManager, parser::CameraSimulationParser},
    },
    generated::options::UsbMode,
    ptp::{DevicePropCode, Ptp, option::SimulationSetting},
};

pub trait CameraBase {
    type Context: rusb::UsbContext;

    fn camera_definition(&self) -> &'static SupportedCamera;

    fn chunk_size(&self) -> usize {
        // Default conservative estimate.
        1024 * 1024
    }

    fn capabilities(&self) -> &'static [Capability] {
        &[]
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
    fn get_info(&self, ptp: &mut Ptp) -> CoreResult<Box<dyn CameraInfo>> {
        let info = ptp.get_info()?;

        let mode = UsbMode::try_pull(ptp)?;

        let battery_string: String = ptp.get_prop(DevicePropCode::FujiBatteryInfo2)?;
        debug!("Raw battery string: {battery_string}");

        let battery_raw =
            battery_string
                .split(',')
                .next()
                .ok_or_else(|| CoreError::DeviceInfoMalformed {
                    prop: DevicePropCode::FujiBatteryInfo2,
                    reason: "missing comma-separated prefix".to_owned(),
                })?;

        let battery: u32 = battery_raw.parse().map_err(|e: std::num::ParseIntError| {
            CoreError::DeviceInfoMalformed {
                prop: DevicePropCode::FujiBatteryInfo2,
                reason: e.to_string(),
            }
        })?;

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

pub struct UnknownCamera;

pub const UNKNOWN_CAMERA: SupportedCamera = SupportedCamera {
    name: "Unknown Camera",
    vendor: 0x0000,
    product: 0x0000,
    camera_factory: || Box::new(UnknownCamera),
    simulation: None,
};

impl CameraBase for UnknownCamera {
    type Context = rusb::GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &UNKNOWN_CAMERA
    }
}
