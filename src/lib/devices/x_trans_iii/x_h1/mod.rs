use super::XTransIII;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_H1: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-H1",
    vendor: 0x04cb,
    product: 0x02d7,
    camera_factory: || Box::new(FujifilmXH1 {}),
};

pub struct FujifilmXH1 {}

impl CameraBase for FujifilmXH1 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_H1
    }
}

impl XTransIII for FujifilmXH1 {}
