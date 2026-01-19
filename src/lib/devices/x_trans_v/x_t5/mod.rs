use rusb::GlobalContext;

use crate::{SupportedCamera, features::base::CameraBase};

use super::XTransV;

pub const FUJIFILM_XT5: SupportedCamera = SupportedCamera {
    name: "FUJIFILM XT-5",
    vendor: 0x04cb,
    product: 0x02fc,
    camera_factory: || Box::new(FujifilmXT5 {}),
};

pub struct FujifilmXT5 {}

impl CameraBase for FujifilmXT5 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_XT5
    }

    fn chunk_size(&self) -> usize {
        // TODO: Experiment with this
        // 15.75 * 1024^2
        16128 * 1024
    }

    fn as_backups(
        &self,
    ) -> Option<&dyn crate::features::backup::CameraBackups<Context = Self::Context>> {
        Some(self)
    }

    fn as_simulations(
        &self,
    ) -> Option<&dyn crate::features::simulation::CameraSimulations<Context = Self::Context>> {
        Some(self)
    }

    fn as_renders(
        &self,
    ) -> Option<&dyn crate::features::render::CameraRenders<Context = Self::Context>> {
        Some(self)
    }
}

impl XTransV for FujifilmXT5 {}
