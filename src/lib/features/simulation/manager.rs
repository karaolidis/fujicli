use strum::IntoEnumIterator;

use crate::{
    features::{
        base::CameraBase,
        simulation::{Simulation, parser::CameraSimulationParser},
    },
    ptp::{Ptp, fuji},
};

pub trait CameraSimulationManager: CameraBase + CameraSimulationParser {
    fn custom_settings_slots(&self) -> Vec<fuji::CustomSetting> {
        fuji::CustomSetting::iter().collect()
    }

    fn get_simulation(
        &self,
        ptp: &mut Ptp,
        slot: fuji::CustomSetting,
    ) -> anyhow::Result<Box<dyn Simulation>>;

    fn update_simulation(
        &self,
        ptp: &mut Ptp,
        slot: fuji::CustomSetting,
        simulation_modifier: &mut dyn FnMut(&mut dyn Simulation) -> anyhow::Result<()>,
    ) -> anyhow::Result<()>;

    fn set_simulation(
        &self,
        ptp: &mut Ptp,
        slot: fuji::CustomSetting,
        simulation: &dyn Simulation,
    ) -> anyhow::Result<()>;
}
