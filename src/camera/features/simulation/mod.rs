#[allow(clippy::module_inception)]
pub mod simulation;

use simulation::Simulation;
use strum::IntoEnumIterator;

use crate::camera::ptp::{Ptp, hex::FujiCustomSetting};

use super::base::CameraBase;

pub trait CameraSimulations: CameraBase {
    fn custom_settings_slots(&self) -> Vec<FujiCustomSetting> {
        FujiCustomSetting::iter().collect()
    }

    fn get_simulation(
        &self,
        ptp: &mut Ptp,
        slot: FujiCustomSetting,
    ) -> anyhow::Result<Box<dyn Simulation>>;

    fn update_simulation(
        &self,
        ptp: &mut Ptp,
        slot: FujiCustomSetting,
        simulation_modifier: &mut dyn FnMut(&mut dyn Simulation) -> anyhow::Result<()>,
    ) -> anyhow::Result<()>;

    fn set_simulation(
        &self,
        ptp: &mut Ptp,
        slot: FujiCustomSetting,
        simulation: &dyn Simulation,
    ) -> anyhow::Result<()>;

    fn export_simulation(&self, ptp: &mut Ptp, slot: FujiCustomSetting) -> anyhow::Result<Vec<u8>>;

    fn import_simulation(
        &self,
        ptp: &mut Ptp,
        slot: FujiCustomSetting,
        simulation: &[u8],
    ) -> anyhow::Result<()>;
}
