use anyhow::bail;
use clap::Args;

#[derive(Debug, Clone)]
pub enum SimulationSelector {
    Slot(u8),
    Name(String),
}

impl std::str::FromStr for SimulationSelector {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(slot) = s.parse::<u8>() {
            return Ok(Self::Slot(slot));
        }

        if s.is_empty() {
            bail!("Simulation name cannot be empty")
        }

        Ok(Self::Name(s.to_string()))
    }
}

#[derive(Args, Debug)]
pub struct FilmSimulationOptions {}
