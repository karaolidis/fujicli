use super::XTrans;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-M1",
    FujifilmXM1,
    FUJIFILM_X_M1,
    0x04cb,
    0x02b6,
    context = GlobalContext,
    sensor = XTrans,
    capabilities = [],
);
