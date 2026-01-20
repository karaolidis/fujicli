pub mod simulation;

use super::XTransIV;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-S20",
    FujifilmXS20,
    FUJIFILM_X_S20,
    0x04cb,
    0x02f7,
    context = GlobalContext,
    sensor = XTransIV,
    capabilities = [
        CameraBackupManager,
        CameraSimulationParser,
        CameraSimulationManager,
    ],
);
