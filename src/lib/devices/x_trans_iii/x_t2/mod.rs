use super::XTransIII;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-T2",
    FujifilmXT2,
    FUJIFILM_X_T2,
    0x04cb,
    0x02cd,
    context = GlobalContext,
    sensor = XTransIII,
    capabilities = [],
);
