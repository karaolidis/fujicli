use super::XTransIV;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X100V: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X100V",
    vendor: 0x04cb,
    product: 0x02e5,
    camera_factory: || Box::new(FujifilmX100V {}),
};

pub struct FujifilmX100V {}

impl CameraBase for FujifilmX100V {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X100V
    }
}

impl XTransIV for FujifilmX100V {}
