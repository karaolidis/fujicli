use super::XTransV;
use crate::camera::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_H2S: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-H2S",
    vendor: 0x04cb,
    product: 0x02f0,
    camera_factory: || Box::new(FujifilmXH2S {}),
};

pub struct FujifilmXH2S {}

impl CameraBase for FujifilmXH2S {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_H2S
    }
}

impl XTransV for FujifilmXH2S {}
