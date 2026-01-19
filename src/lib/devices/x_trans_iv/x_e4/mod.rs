use super::XTransIV;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_E4: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-E4",
    vendor: 0x04cb,
    product: 0x02e8,
    camera_factory: || Box::new(FujifilmXE4 {}),
};

pub struct FujifilmXE4 {}

impl CameraBase for FujifilmXE4 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_E4
    }
}

impl XTransIV for FujifilmXE4 {}
