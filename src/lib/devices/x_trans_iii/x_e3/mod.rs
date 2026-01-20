use super::XTransIII;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-E3",
    FujifilmXE3,
    FUJIFILM_X_E3,
    0x04cb,
    0x02d6,
    context = GlobalContext,
    sensor = XTransIII,
    capabilities = [],
);
