use super::XTransIV;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-T4",
    FujifilmXT4,
    FUJIFILM_X_T4,
    0x04cb,
    0x02e6,
    context = GlobalContext,
    sensor = XTransIV,
    capabilities = [],
);
