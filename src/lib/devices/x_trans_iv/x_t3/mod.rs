use super::XTransIV;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-T3",
    FujifilmXT3,
    FUJIFILM_X_T3,
    0x04cb,
    0x02dd,
    context = GlobalContext,
    sensor = XTransIV,
    capabilities = [],
);
