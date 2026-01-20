pub mod render;
pub mod simulation;

use super::XTransV;
use crate::devices::define_camera;
use rusb::GlobalContext;

define_camera!(
    "FUJIFILM X-T5",
    FujifilmXT5,
    FUJIFILM_X_T5,
    0x04cb,
    0x02fc,
    context = GlobalContext,
    sensor = XTransV,
    capabilities = [
        CameraBackupManager,
        CameraSimulationParser,
        CameraSimulationManager,
        CameraRenderManager,
    ],
    chunk_size = 16128 * 1024,
);
