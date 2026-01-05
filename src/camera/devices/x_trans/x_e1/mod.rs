use super::XTrans;
use crate::camera::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_E1: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-E1",
    vendor: 0x04cb,
    product: 0x0283,
    camera_factory: || Box::new(FujifilmXE1 {}),
};

pub struct FujifilmXE1 {}

impl CameraBase for FujifilmXE1 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_E1
    }
}

impl XTrans for FujifilmXE1 {}
