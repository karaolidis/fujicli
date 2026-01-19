use super::XTransII;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_E2: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-E2",
    vendor: 0x04cb,
    product: 0x02b5,
    camera_factory: || Box::new(FujifilmXE2 {}),
};

pub struct FujifilmXE2 {}

impl CameraBase for FujifilmXE2 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_E2
    }
}

impl XTransII for FujifilmXE2 {}
