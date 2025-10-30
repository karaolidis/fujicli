use std::path::PathBuf;

use super::common::{
    file::{Input, Output},
    film::FilmSimulationOptions,
};
use clap::Args;

#[derive(Args, Debug)]
pub struct RenderCmd {
    /// Simulation slot number
    #[arg(long, conflicts_with = "simulation_file")]
    simulation: Option<u8>,

    /// Path to exported simulation file
    #[arg(long, conflicts_with = "simulation")]
    simulation_file: Option<PathBuf>,

    #[command(flatten)]
    film_simulation_options: FilmSimulationOptions,

    /// RAF input file (use '-' to read from stdin)
    input: Input,

    /// Output file (use '-' to write to stdout)
    output: Output,
}
