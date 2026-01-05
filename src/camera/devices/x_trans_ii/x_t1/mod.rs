use super::XTransII;
use crate::camera::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_T1: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-T1",
    vendor: 0x04cb,
    product: 0x02bf,
    camera_factory: || Box::new(FujifilmXT1 {}),
};

pub struct FujifilmXT1 {}

impl CameraBase for FujifilmXT1 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_T1
    }
}

impl XTransII for FujifilmXT1 {}
