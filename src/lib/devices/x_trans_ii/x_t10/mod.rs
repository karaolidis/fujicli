use super::XTransII;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-T10",
    FujifilmXT10,
    FUJIFILM_X_T10,
    0x04cb,
    0x02c8,
    context = GlobalContext,
    sensor = XTransII,
    capabilities = [],
);
