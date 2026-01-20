use super::XTransV;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-H2",
    FujifilmXH2,
    FUJIFILM_X_H2,
    0x04cb,
    0x02f2,
    context = GlobalContext,
    sensor = XTransV,
    capabilities = [],
);
