use super::XTransII;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X70",
    FujifilmX70,
    FUJIFILM_X70,
    0x04cb,
    0x02ba,
    context = GlobalContext,
    sensor = XTransII,
    capabilities = [],
);
