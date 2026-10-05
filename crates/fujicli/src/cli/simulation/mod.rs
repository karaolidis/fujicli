use anyhow::Context;
use clap::Subcommand;
use fujicore::{
    features::simulation::SimulationListItem,
    generated::{cli::SimulationArgs, options::CustomSetting, simulations::SimulationBase},
};

use super::common::file::{Input, Output};
use crate::cli::{GlobalOptions, common::usb};

#[derive(Subcommand, Debug)]
pub enum SimulationCmd {
    /// List simulations
    #[command(alias = "l")]
    List,

    /// Get simulation
    #[command(alias = "g")]
    Get {
        /// Simulation slot number
        slot: CustomSetting,
    },

    /// Set simulation parameters
    #[command(alias = "s")]
    Set {
        /// Simulation slot number
        slot: CustomSetting,

        #[command(flatten)]
        simulation: SimulationArgs,
    },

    /// Export simulation
    #[command(alias = "e")]
    Export {
        /// Simulation slot number
        slot: CustomSetting,

        /// Output file (use '-' to write to stdout)
        output: Output,
    },

    /// Import simulation
    #[command(alias = "i")]
    Import {
        /// Simulation slot number
        slot: CustomSetting,

        /// Input file (use '-' to read from stdin)
        input: Input,
    },
}

#[allow(clippy::needless_pass_by_value)]
fn handle_list(options: GlobalOptions) -> anyhow::Result<()> {
    let GlobalOptions {
        json,
        device,
        emulate,
        ..
    } = options;

    let mut camera = usb::get_camera(device, emulate)?;

    let slot_list = camera
        .custom_settings_slots()
        .context("listing simulation slots")?;

    let mut slots: Vec<SimulationListItem> = Vec::with_capacity(slot_list.len());
    for slot in slot_list {
        let simulation = camera
            .get_simulation(slot)
            .with_context(|| format!("reading simulation slot {slot}"))?;
        slots.push(SimulationListItem {
            slot,
            name: simulation.name(),
        });
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&slots).context("serializing slot list to JSON")?
        );
    } else {
        for slot in slots {
            println!("- {slot}");
        }
    }

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn handle_get(options: GlobalOptions, slot: CustomSetting) -> anyhow::Result<()> {
    let GlobalOptions {
        json,
        device,
        emulate,
        ..
    } = options;

    let mut camera = usb::get_camera(device, emulate)?;

    let simulation = camera
        .get_simulation(slot)
        .with_context(|| format!("reading simulation slot {slot}"))?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&simulation).context("serializing simulation to JSON")?
        );
    } else {
        println!("{simulation}");
    }

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn handle_set(
    options: GlobalOptions,
    simulation: SimulationArgs,
    slot: CustomSetting,
) -> anyhow::Result<()> {
    let GlobalOptions {
        device, emulate, ..
    } = options;

    let mut camera = usb::get_camera(device, emulate)?;
    let partial: SimulationBase = simulation.into();
    camera
        .update_simulation(slot, partial)
        .with_context(|| format!("updating simulation slot {slot}"))?;
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn handle_export(
    options: GlobalOptions,
    slot: CustomSetting,
    output: Output,
) -> anyhow::Result<()> {
    let GlobalOptions {
        device, emulate, ..
    } = options;

    let mut camera = usb::get_camera(device, emulate)?;

    let simulation = camera
        .get_simulation(slot)
        .with_context(|| format!("reading simulation slot {slot}"))?;
    let simulation = camera
        .serialize_simulation(&*simulation)
        .context("serializing simulation")?;
    let mut writer = output.get_writer()?;
    writer
        .write_all(&simulation)
        .context("writing serialized simulation to output")?;

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn handle_import(options: GlobalOptions, slot: CustomSetting, input: Input) -> anyhow::Result<()> {
    let GlobalOptions {
        device, emulate, ..
    } = options;

    let mut camera = usb::get_camera(device, emulate)?;

    let mut reader = input.get_reader()?;
    let mut buffer = Vec::new();
    reader
        .read_to_end(&mut buffer)
        .context("reading serialized simulation from input")?;
    let simulation = camera
        .deserialize_simulation(&buffer)
        .context("deserializing simulation")?;
    camera
        .set_simulation(slot, &*simulation)
        .with_context(|| format!("writing simulation to slot {slot}"))?;

    Ok(())
}

pub fn handle(cmd: SimulationCmd, options: GlobalOptions) -> anyhow::Result<()> {
    match cmd {
        SimulationCmd::List => handle_list(options),
        SimulationCmd::Get { slot } => handle_get(options, slot),
        SimulationCmd::Set { slot, simulation } => handle_set(options, simulation, slot),
        SimulationCmd::Export { slot, output } => handle_export(options, slot, output),
        SimulationCmd::Import { slot, input } => handle_import(options, slot, input),
    }
}
