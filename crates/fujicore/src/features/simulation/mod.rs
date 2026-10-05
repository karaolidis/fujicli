pub mod manager;
pub mod parser;

pub use manager::CameraSimulationManager;
pub use parser::CameraSimulationParser;

use std::{any::Any, fmt};

use erased_serde::serialize_trait_object;
use serde::Serialize;

use crate::{
    generated::{
        options::{CustomSetting, CustomSettingName},
        simulations::SimulationBase,
    },
    ptp::Ptp,
};

pub trait Simulation: fmt::Display + erased_serde::Serialize {
    fn as_any(&self) -> &dyn Any;

    fn name(&self) -> Option<CustomSettingName>;

    fn try_update_from(&mut self, partial: SimulationBase) -> anyhow::Result<()>;

    fn try_pull(ptp: &mut crate::ptp::Ptp) -> anyhow::Result<Self>
    where
        Self: Sized;

    fn try_push(&self, ptp: &mut Ptp) -> anyhow::Result<()>;

    fn to_base(&self) -> SimulationBase;
}

serialize_trait_object!(Simulation);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationListItem {
    pub slot: CustomSetting,
    pub name: Option<CustomSettingName>,
}

impl fmt::Display for SimulationListItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.name {
            Some(name) => write!(f, "{}: {}", self.slot, name),
            None => write!(f, "{}: <unnamed>", self.slot),
        }
    }
}
