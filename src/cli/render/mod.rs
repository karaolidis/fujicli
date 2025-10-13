use std::path::PathBuf;

use super::common::{
    file::{Input, Output},
    film::{FilmSimulationOptions, SimulationSelector},
};
use clap::Args;

#[derive(Args, Debug)]
pub struct RenderCmd {
    /// Simulation number or name
    #[arg(long, conflicts_with = "simulation_file")]
    simulation: Option<SimulationSelector>,

    /// Path to exported simulation
    #[arg(long, conflicts_with = "simulation")]
    simulation_file: Option<PathBuf>,

    #[command(flatten)]
    film_simulation_options: FilmSimulationOptions,

    /// RAF input file (use '-' to read from stdin)
    input: Input,

    /// Output file (use '-' to write to stdout)
    output: Output,
}
