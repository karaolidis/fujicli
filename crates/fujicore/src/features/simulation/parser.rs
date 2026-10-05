use crate::{error::CoreResult, features::simulation::Simulation};

pub trait CameraSimulationParser {
    fn deserialize_simulation(&self, simulation: &[u8]) -> CoreResult<Box<dyn Simulation>>;

    fn serialize_simulation(&self, simulation: &dyn Simulation) -> CoreResult<Vec<u8>>;
}
