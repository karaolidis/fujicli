use super::XTransIII;
use crate::camera::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_PRO2: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-Pro2",
    vendor: 0x04cb,
    product: 0x02cb,
    camera_factory: || Box::new(FujifilmXPro2 {}),
};

pub struct FujifilmXPro2 {}

impl CameraBase for FujifilmXPro2 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_PRO2
    }
}

impl XTransIII for FujifilmXPro2 {}
