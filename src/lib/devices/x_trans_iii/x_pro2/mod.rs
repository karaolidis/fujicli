use super::XTransIII;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-Pro2",
    FujifilmXPro2,
    FUJIFILM_X_PRO2,
    0x04cb,
    0x02cb,
    context = GlobalContext,
    sensor = XTransIII,
    capabilities = [],
);
