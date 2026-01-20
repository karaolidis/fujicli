use super::XTransIV;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X100V",
    FujifilmX100V,
    FUJIFILM_X100V,
    0x04cb,
    0x02e5,
    context = GlobalContext,
    sensor = XTransIV,
    capabilities = [],
);
