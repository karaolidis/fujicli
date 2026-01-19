#[allow(clippy::module_inception)]
pub mod simulation;

use simulation::Simulation;
use strum::IntoEnumIterator;

use crate::ptp::{Ptp, hex::FujiCustomSetting};

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

    fn deserialize_simulation(&self, simulation: &[u8]) -> anyhow::Result<Box<dyn Simulation>>;

    fn serialize_simulation(&self, simulation: &dyn Simulation) -> anyhow::Result<Vec<u8>>;
}
