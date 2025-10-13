mod backup;
mod common;
mod device;
mod render;
mod simulation;

use clap::{Parser, Subcommand};

use backup::BackupCmd;
use device::DeviceCmd;
use render::RenderCmd;
use simulation::SimulationCmd;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, author)]
pub struct Cli {
    /// Subcommands
    #[command(subcommand)]
    pub command: Commands,

    /// Format output using json
    #[arg(long, short = 'j', global = true)]
    pub json: bool,

    /// Only log warnings and errors
    #[arg(long, short = 'q', global = true, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Log extra debugging information
    #[arg(long, short = 'v', global = true, conflicts_with = "quiet")]
    pub verbose: bool,

    /// Manually specify target device
    #[arg(long, short = 'd', global = true)]
    pub device: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage devices
    #[command(alias = "d", subcommand)]
    Device(DeviceCmd),

    /// Manage film simulations
    #[command(alias = "s", subcommand)]
    Simulation(SimulationCmd),

    /// Manage backups
    #[command(alias = "b", subcommand)]
    Backup(BackupCmd),

    /// Render images
    #[command(alias = "r")]
    Render(RenderCmd),
}
