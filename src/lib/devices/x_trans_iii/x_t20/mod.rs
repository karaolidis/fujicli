use super::XTransIII;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_T20: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-T20",
    vendor: 0x04cb,
    product: 0x02d4,
    camera_factory: || Box::new(FujifilmXT20 {}),
};

pub struct FujifilmXT20 {}

impl CameraBase for FujifilmXT20 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_T20
    }
}

impl XTransIII for FujifilmXT20 {}
