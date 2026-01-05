use super::XTransIV;
use crate::camera::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_S10: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-S10",
    vendor: 0x04cb,
    product: 0x02ea,
    camera_factory: || Box::new(FujifilmXS10 {}),
};

pub struct FujifilmXS10 {}

impl CameraBase for FujifilmXS10 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_S10
    }
}

impl XTransIV for FujifilmXS10 {}
