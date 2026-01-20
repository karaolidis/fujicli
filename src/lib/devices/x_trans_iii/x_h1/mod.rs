use super::XTransIII;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-H1",
    FujifilmXH1,
    FUJIFILM_X_H1,
    0x04cb,
    0x02d7,
    context = GlobalContext,
    sensor = XTransIII,
    capabilities = [],
);
