use super::common::{
    file::{Input, Output},
    film::FilmSimulationOptions,
};
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum SimulationCmd {
    /// List simulations
    #[command(alias = "l")]
    List,

    /// Get simulation
    #[command(alias = "g")]
    Get {
        /// Simulation number or name
        simulation: u8,
    },

    /// Set simulation parameters
    #[command(alias = "s")]
    Set {
        /// Simulation number or name
        simulation: u8,

        #[command(flatten)]
        film_simulation_options: FilmSimulationOptions,
    },

    /// Export simulation
    #[command(alias = "e")]
    Export {
        /// Simulation number or name
        simulation: u8,

        /// Output file (use '-' to write to stdout)
        output_file: Output,
    },

    /// Import simulation
    #[command(alias = "i")]
    Import {
        /// Simulation number
        slot: u8,

        /// Input file (use '-' to read from stdin)
        input_file: Input,
    },
}
