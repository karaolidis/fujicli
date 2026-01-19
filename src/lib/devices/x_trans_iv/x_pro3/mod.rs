use super::XTransIV;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_PRO3: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-Pro3",
    vendor: 0x04cb,
    product: 0x02e4,
    camera_factory: || Box::new(FujifilmXPro3 {}),
};

pub struct FujifilmXPro3 {}

impl CameraBase for FujifilmXPro3 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_PRO3
    }
}

impl XTransIV for FujifilmXPro3 {}
