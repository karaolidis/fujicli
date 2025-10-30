use crate::{
    camera::{
        features::simulation::simulation::SimulationListItem,
        ptp::hex::{FujiCustomSetting, FujiCustomSettingName},
    },
    usb,
};

use super::common::{
    file::{Input, Output},
    film::FilmSimulationOptions,
};
use clap::{Args, Subcommand};

#[derive(Subcommand, Debug)]
pub enum SimulationCmd {
    /// List simulations
    #[command(alias = "l")]
    List,

    /// Get simulation
    #[command(alias = "g")]
    Get {
        /// Simulation slot number
        slot: FujiCustomSetting,
    },

    /// Set simulation parameters
    #[command(alias = "s")]
    Set {
        /// Simulation slot number
        slot: FujiCustomSetting,

        #[command(flatten)]
        set_film_simulation_options: SetFilmSimulationOptions,

        #[command(flatten)]
        film_simulation_options: FilmSimulationOptions,
    },

    /// Export simulation
    #[command(alias = "e")]
    Export {
        /// Simulation slot number
        slot: FujiCustomSetting,

        /// Output file (use '-' to write to stdout)
        output_file: Output,
    },

    /// Import simulation
    #[command(alias = "i")]
    Import {
        /// Simulation slot number
        slot: FujiCustomSetting,

        /// Input file (use '-' to read from stdin)
        input_file: Input,
    },
}

#[derive(Args, Debug)]
pub struct SetFilmSimulationOptions {
    /// The name of the slot
    #[clap(long)]
    pub name: Option<FujiCustomSettingName>,
}

fn handle_list(json: bool, device_id: Option<&str>) -> anyhow::Result<()> {
    let mut camera = usb::get_camera(device_id)?;

    let slots: Vec<SimulationListItem> = camera
        .custom_settings_slots()?
        .into_iter()
        .map(|slot| -> anyhow::Result<SimulationListItem> {
            let simulation = camera.get_simulation(slot)?;
            let name = simulation.get_name()?;
            Ok(SimulationListItem { slot, name })
        })
        .collect::<anyhow::Result<Vec<SimulationListItem>>>()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&slots)?);
    } else {
        for slot in slots {
            println!("- {slot}");
        }
    }

    Ok(())
}

fn handle_get(json: bool, device_id: Option<&str>, slot: FujiCustomSetting) -> anyhow::Result<()> {
    let mut camera = usb::get_camera(device_id)?;
    let simulation = camera.get_simulation(slot)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&simulation)?);
    } else {
        println!("{simulation}");
    }

    Ok(())
}

macro_rules! update_simulation {
    ($sim_var:ident, [
        $($local_field:ident => $setter:ident,)*
    ]) => {
        $(
            if let Some(value) = $local_field {
                $sim_var.$setter(value)?;
            }
        )*
    };
}

#[allow(clippy::cognitive_complexity)]
#[allow(clippy::too_many_lines)]
fn handle_set(
    device_id: Option<&str>,
    slot: FujiCustomSetting,
    set_options: &SetFilmSimulationOptions,
    options: &FilmSimulationOptions,
) -> anyhow::Result<()> {
    let mut camera = usb::get_camera(device_id)?;

    let SetFilmSimulationOptions { name } = set_options;

    let FilmSimulationOptions {
        simulation: film_simulation,
        monochromatic_color_temperature,
        monochromatic_color_tint,
        size,
        quality,
        highlight,
        shadow,
        color,
        sharpness,
        clarity,
        white_balance,
        white_balance_shift_red,
        white_balance_shift_blue,
        white_balance_temperature,
        dynamic_range,
        dynamic_range_priority,
        noise_reduction,
        grain,
        color_chrome_effect,
        color_chrome_fx_blue,
        smooth_skin_effect,
        lens_modulation_optimizer,
        color_space,
    } = options;

    camera.update_simulation(slot, &mut |simulation| {
        update_simulation! {
            simulation,
            [
                name => set_name,
                film_simulation => set_simulation,
                monochromatic_color_temperature => set_monochromatic_color_temperature,
                monochromatic_color_tint => set_monochromatic_color_tint,
                size => set_size,
                quality => set_quality,
                highlight => set_highlight,
                shadow => set_shadow,
                color => set_color,
                sharpness => set_sharpness,
                clarity => set_clarity,
                white_balance => set_white_balance,
                white_balance_shift_red => set_white_balance_shift_red,
                white_balance_shift_blue => set_white_balance_shift_blue,
                white_balance_temperature => set_white_balance_temperature,
                dynamic_range => set_dynamic_range,
                dynamic_range_priority => set_dynamic_range_priority,
                noise_reduction => set_noise_reduction,
                grain => set_grain,
                color_chrome_effect => set_color_chrome_effect,
                color_chrome_fx_blue => set_color_chrome_fx_blue,
                smooth_skin_effect => set_smooth_skin_effect,
                lens_modulation_optimizer => set_lens_modulation_optimizer,
                color_space => set_color_space,
            ]
        };

        Ok(())
    })?;

    Ok(())
}

fn handle_export(
    _device_id: Option<&str>,
    _slot: FujiCustomSetting,
    _output: &Output,
) -> anyhow::Result<()> {
    todo!();
}

fn handle_import(
    _device_id: Option<&str>,
    _slot: FujiCustomSetting,
    _input: &Input,
) -> anyhow::Result<()> {
    todo!();
}

pub fn handle(cmd: SimulationCmd, json: bool, device_id: Option<&str>) -> anyhow::Result<()> {
    match cmd {
        SimulationCmd::List => handle_list(json, device_id),
        SimulationCmd::Get { slot } => handle_get(json, device_id, slot),
        SimulationCmd::Set {
            slot,
            set_film_simulation_options,
            film_simulation_options,
        } => handle_set(
            device_id,
            slot,
            &set_film_simulation_options,
            &film_simulation_options,
        ),
        SimulationCmd::Export { slot, output_file } => handle_export(device_id, slot, &output_file),
        SimulationCmd::Import { slot, input_file } => handle_import(device_id, slot, &input_file),
    }
}
