use super::XTransII;
use crate::{SupportedCamera, features::base::CameraBase};
use rusb::GlobalContext;

pub const FUJIFILM_X_T10: SupportedCamera = SupportedCamera {
    name: "FUJIFILM X-T10",
    vendor: 0x04cb,
    product: 0x02c8,
    camera_factory: || Box::new(FujifilmXT10 {}),
};

pub struct FujifilmXT10 {}

impl CameraBase for FujifilmXT10 {
    type Context = GlobalContext;

    fn camera_definition(&self) -> &'static SupportedCamera {
        &FUJIFILM_X_T10
    }
}

impl XTransII for FujifilmXT10 {}
