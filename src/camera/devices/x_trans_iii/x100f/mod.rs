use super::XTransIII;
use crate::camera::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X100F: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X100F",
    vendor: 0x04cb,
    product: 0x02d1,
    camera_factory: || Box::new(FujifilmX100F {}),
};

pub struct FujifilmX100F {}

impl CameraBase for FujifilmX100F {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X100F
    }
}

impl XTransIII for FujifilmX100F {}
