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

macro_rules! impl_camera_base {
    (
        $camera:ty,
        $def:expr,
        [ $( $cap:ident ),* $(,)? ]
        $(, $chunk:expr )?
    ) => {
        impl crate::features::base::CameraBase for $camera {
            type Context = rusb::GlobalContext;

            fn camera_definition(&self) -> &'static crate::SupportedCamera {
                $def
            }

            $(
                fn chunk_size(&self) -> usize {
                    $chunk
                }
            )?

            $(
                crate::features::base::impl_camera_base!(@cap self, $cap);
            )*
        }
    };

    (@cap $self:ident, CameraBackupManager) => {
        fn as_backup_manager(
            &$self,
        ) -> Option<&dyn crate::features::backup::CameraBackupManager<Context = rusb::GlobalContext>> {
            Some($self)
        }
    };

    (@cap $self:ident, CameraSimulationParser) => {
        fn as_simulation_parser(
            &$self,
        ) -> Option<&dyn crate::features::simulation::CameraSimulationParser> {
            Some($self)
        }
    };

    (@cap $self:ident, CameraSimulationManager) => {
        fn as_simulation_manager(
            &$self,
        ) -> Option<&dyn crate::features::simulation::CameraSimulationManager<Context = rusb::GlobalContext>> {
            Some($self)
        }
    };

    (@cap $self:ident, CameraRenderManager) => {
        fn as_render_manager(
            &$self,
        ) -> Option<&dyn crate::features::render::CameraRenderManager<Context = rusb::GlobalContext>> {
            Some($self)
        }
    };
}

pub(crate) use impl_camera_base;

macro_rules! define_camera {
    (
        $name:literal,
        $struct_name:ident,
        $const_name:ident,
        $vendor:expr,
        $product:expr,
        $sensor:ident,
        [ $( $cap:ident ),* $(,)? ],
        $( $chunk:expr, )?
    ) => {
        pub struct $struct_name;

        pub const $const_name: crate::SupportedCamera = crate::SupportedCamera {
            name: $name,
            vendor: $vendor,
            product: $product,
            camera_factory: || Box::new($struct_name {}),
        };

        crate::features::base::impl_camera_base!(
            $struct_name,
            &$const_name,
            [ $( $cap ),* ]
            $( , $chunk )?
        );

        impl $sensor for $struct_name {}
    };
}

pub(crate) use define_camera;

#[allow(dead_code)]
trait UnknownSensor {}

define_camera!(
    "Unknown Camera",
    UnknownCamera,
    UNKNOWN_CAMERA,
    0x0000,
    0x0000,
    UnknownSensor,
    [],
);
