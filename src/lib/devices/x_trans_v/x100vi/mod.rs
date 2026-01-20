use super::XTransV;
use crate::devices::define_camera;

define_camera!(
    "FUJIFILM X100VI",
    FujifilmX100VI,
    FUJIFILM_X100VI,
    0x04cb,
    0x0305,
    XTransV,
    [],
);
