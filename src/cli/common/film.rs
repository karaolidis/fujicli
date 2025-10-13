use std::error::Error;

use clap::Args;

#[derive(Debug, Clone)]
pub enum SimulationSelector {
    Slot(u8),
    Name(String),
}

impl std::str::FromStr for SimulationSelector {
    type Err = Box<dyn Error + Send + Sync>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(slot) = s.parse::<u8>() {
            return Ok(SimulationSelector::Slot(slot));
        }

        if s.is_empty() {
            Err("Simulation name cannot be empty".into())
        } else {
            Ok(SimulationSelector::Name(s.to_string()))
        }
    }
}

#[derive(Args, Debug)]
pub struct FilmSimulationOptions {}
