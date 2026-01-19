use super::XTrans;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_M1: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-M1",
    vendor: 0x04cb,
    product: 0x02b6,
    camera_factory: || Box::new(FujifilmXM1 {}),
};

pub struct FujifilmXM1 {}

impl CameraBase for FujifilmXM1 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_M1
    }
}

impl XTrans for FujifilmXM1 {}
