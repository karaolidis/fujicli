use super::XTransIV;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-S10",
    FujifilmXS10,
    FUJIFILM_X_S10,
    0x04cb,
    0x02ea,
    context = GlobalContext,
    sensor = XTransIV,
    capabilities = [],
);
