use super::XTransIII;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-T20",
    FujifilmXT20,
    FUJIFILM_X_T20,
    0x04cb,
    0x02d4,
    context = GlobalContext,
    sensor = XTransIII,
    capabilities = [],
);
