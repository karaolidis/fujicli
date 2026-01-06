pub mod backup;
pub mod common;
pub mod device;
pub mod render;
pub mod simulation;

use clap::{ArgAction, Args, Parser, Subcommand};

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

    #[command(flatten)]
    pub options: GlobalOptions,
}

#[derive(Args, Debug)]
pub struct GlobalOptions {
    /// Format output using json
    #[arg(long, short = 'j', global = true)]
    pub json: bool,

    /// Log extra debugging information (multiple instances increase verbosity)
    #[arg(long, short = 'v', action = ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Manually specify target device using USB <BUS>.<ADDRESS>
    #[arg(long, short = 'd', global = true)]
    pub device: Option<String>,

    /// Treat device as a different model using <`VENDOR_ID>`:<`PRODUCT_ID`>
    #[arg(long, global = true, requires = "device")]
    pub emulate: Option<String>,
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

    /// Render images using in-camera processor
    #[command(alias = "r")]
    Render(RenderCmd),
}
