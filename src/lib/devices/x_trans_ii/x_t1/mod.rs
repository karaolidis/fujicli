use super::XTransII;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-T1",
    FujifilmXT1,
    FUJIFILM_X_T1,
    0x04cb,
    0x02bf,
    context = GlobalContext,
    sensor = XTransII,
    capabilities = [],
);
