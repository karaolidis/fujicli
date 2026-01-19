use super::XTransII;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X70: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X70",
    vendor: 0x04cb,
    product: 0x02ba,
    camera_factory: || Box::new(FujifilmX70 {}),
};

pub struct FujifilmX70 {}

impl CameraBase for FujifilmX70 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X70
    }
}

impl XTransII for FujifilmX70 {}
