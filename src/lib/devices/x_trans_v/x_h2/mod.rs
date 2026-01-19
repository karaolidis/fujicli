use super::XTransV;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_H2: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-H2",
    vendor: 0x04cb,
    product: 0x02f2,
    camera_factory: || Box::new(FujifilmXH2 {}),
};

pub struct FujifilmXH2 {}

impl CameraBase for FujifilmXH2 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_H2
    }
}

impl XTransV for FujifilmXH2 {}
