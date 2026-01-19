use super::XTransIII;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_T2: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-T2",
    vendor: 0x04cb,
    product: 0x02cd,
    camera_factory: || Box::new(FujifilmXT2 {}),
};

pub struct FujifilmXT2 {}

impl CameraBase for FujifilmXT2 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_T2
    }
}

impl XTransIII for FujifilmXT2 {}
