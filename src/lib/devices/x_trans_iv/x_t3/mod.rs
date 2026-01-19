use super::XTransIV;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_T3: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-T3",
    vendor: 0x04cb,
    product: 0x02dd,
    camera_factory: || Box::new(FujifilmXT3 {}),
};

pub struct FujifilmXT3 {}

impl CameraBase for FujifilmXT3 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_T3
    }
}

impl XTransIV for FujifilmXT3 {}
