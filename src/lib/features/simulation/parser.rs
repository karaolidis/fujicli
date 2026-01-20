use crate::features::simulation::Simulation;

pub trait CameraSimulationParser {
    fn deserialize_simulation(&self, simulation: &[u8]) -> anyhow::Result<Box<dyn Simulation>>;

    fn serialize_simulation(&self, simulation: &dyn Simulation) -> anyhow::Result<Vec<u8>>;
}
