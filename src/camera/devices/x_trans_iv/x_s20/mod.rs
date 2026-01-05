use super::XTransIV;
use crate::camera::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_S20: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-S20",
    vendor: 0x04cb,
    product: 0x02f7,
    camera_factory: || Box::new(FujifilmXS20 {}),
};

pub struct FujifilmXS20 {}

impl CameraBase for FujifilmXS20 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_S20
    }
}

impl XTransIV for FujifilmXS20 {}
