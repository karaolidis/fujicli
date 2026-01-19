use super::XTransIII;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_E3: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-E3",
    vendor: 0x04cb,
    product: 0x02d6,
    camera_factory: || Box::new(FujifilmXE3 {}),
};

pub struct FujifilmXE3 {}

impl CameraBase for FujifilmXE3 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_E3
    }
}

impl XTransIII for FujifilmXE3 {}
