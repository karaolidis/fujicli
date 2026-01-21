pub mod simulation;

use super::XTransIV;
use crate::features::base::define_camera;

define_camera!(
    "FUJIFILM X-S20",
    FujifilmXS20,
    FUJIFILM_X_S20,
    0x04cb,
    0x02f7,
    XTransIV,
    [
        CameraBackupManager,
        CameraSimulationParser,
        CameraSimulationManager,
    ],
);
