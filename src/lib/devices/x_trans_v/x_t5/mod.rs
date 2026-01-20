pub mod render;
pub mod simulation;

use super::XTransV;
use crate::devices::define_camera;

define_camera!(
    "FUJIFILM X-T5",
    FujifilmXT5,
    FUJIFILM_X_T5,
    0x04cb,
    0x02fc,
    XTransV,
    [
        CameraBackupManager,
        CameraSimulationParser,
        CameraSimulationManager,
        CameraRenderManager,
    ],
    16128 * 1024,
);
