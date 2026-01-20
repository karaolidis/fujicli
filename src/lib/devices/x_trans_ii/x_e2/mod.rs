use super::XTransII;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-E2",
    FujifilmXE2,
    FUJIFILM_X_E2,
    0x04cb,
    0x02b5,
    context = GlobalContext,
    sensor = XTransII,
    capabilities = [],
);
