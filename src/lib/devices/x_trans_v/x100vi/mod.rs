use super::XTransV;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X100VI",
    FujifilmX100VI,
    FUJIFILM_X100VI,
    0x04cb,
    0x0305,
    context = GlobalContext,
    sensor = XTransV,
    capabilities = [],
);
