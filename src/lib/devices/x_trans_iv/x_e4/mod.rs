use super::XTransIV;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-E4",
    FujifilmXE4,
    FUJIFILM_X_E4,
    0x04cb,
    0x02e8,
    context = GlobalContext,
    sensor = XTransIV,
    capabilities = [],
);
