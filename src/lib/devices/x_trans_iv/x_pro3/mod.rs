use super::XTransIV;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-Pro3",
    FujifilmXPro3,
    FUJIFILM_X_PRO3,
    0x04cb,
    0x02e4,
    context = GlobalContext,
    sensor = XTransIV,
    capabilities = [],
);
