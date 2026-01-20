use super::XTransIII;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X100F",
    FujifilmX100F,
    FUJIFILM_X100F,
    0x04cb,
    0x02d1,
    context = GlobalContext,
    sensor = XTransIII,
    capabilities = [],
);
