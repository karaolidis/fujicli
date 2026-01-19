use super::XTransV;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X100VI: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X100VI",
    vendor: 0x04cb,
    product: 0x0305,
    camera_factory: || Box::new(FujifilmX100VI {}),
};

pub struct FujifilmX100VI {}

impl CameraBase for FujifilmX100VI {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X100VI
    }
}

impl XTransV for FujifilmX100VI {}
