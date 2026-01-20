use super::XTrans;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-E1",
    FujifilmXE1,
    FUJIFILM_X_E1,
    0x04cb,
    0x0283,
    context = GlobalContext,
    sensor = XTrans,
    capabilities = [],
);
