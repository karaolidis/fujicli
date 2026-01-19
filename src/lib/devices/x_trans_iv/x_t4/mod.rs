use super::XTransIV;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_T4: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-T4",
    vendor: 0x04cb,
    product: 0x02e6,
    camera_factory: || Box::new(FujifilmXT4 {}),
};

pub struct FujifilmXT4 {}

impl CameraBase for FujifilmXT4 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_T4
    }
}

impl XTransIV for FujifilmXT4 {}
