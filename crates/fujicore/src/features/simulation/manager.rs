use crate::{
    error::CoreResult,
    features::{
        base::CameraBase,
        simulation::{Simulation, parser::CameraSimulationParser},
    },
    generated::{options::CustomSetting, simulations::SimulationBase},
    ptp::Ptp,
};

pub trait CameraSimulationManager: CameraBase + CameraSimulationParser {
    fn custom_settings_slots(&self) -> Vec<CustomSetting>;

    fn get_simulation(&self, ptp: &mut Ptp, slot: CustomSetting)
    -> CoreResult<Box<dyn Simulation>>;

    fn update_simulation(
        &self,
        ptp: &mut Ptp,
        slot: CustomSetting,
        partial: SimulationBase,
    ) -> CoreResult<()>;

    fn set_simulation(
        &self,
        ptp: &mut Ptp,
        slot: CustomSetting,
        simulation: &dyn Simulation,
    ) -> CoreResult<()>;
}
