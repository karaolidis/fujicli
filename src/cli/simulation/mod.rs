use fujicli::{features::simulation::SimulationListItem, ptp::fuji, usb};

use super::common::{
    file::{Input, Output},
    film::FilmSimulationOptions,
};
use crate::cli::GlobalOptions;
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
        slot: fuji::CustomSetting,
    },

    /// Set simulation parameters
    #[command(alias = "s")]
    Set {
        /// Simulation slot number
        slot: fuji::CustomSetting,

        #[command(flatten)]
        set_film_simulation_options: SetFilmSimulationOptions,

        #[command(flatten)]
        film_simulation_options: FilmSimulationOptions,
    },

    /// Export simulation
    #[command(alias = "e")]
    Export {
        /// Simulation slot number
        slot: fuji::CustomSetting,

        /// Output file (use '-' to write to stdout)
        output_file: Output,
    },

    /// Import simulation
    #[command(alias = "i")]
    Import {
        /// Simulation slot number
        slot: fuji::CustomSetting,

        /// Input file (use '-' to read from stdin)
        input_file: Input,
    },
}

#[derive(Args, Debug)]
pub struct SetFilmSimulationOptions {
    /// The name of the slot
    #[clap(long)]
    pub name: Option<fuji::CustomSettingName>,
}

fn handle_list(options: &GlobalOptions) -> anyhow::Result<()> {
    let GlobalOptions {
        json,
        device,
        emulate,
        ..
    } = options;

    let mut camera = usb::get_camera(device.as_deref(), emulate.as_deref())?;

    let slots: Vec<SimulationListItem> = camera
        .custom_settings_slots()?
        .into_iter()
        .map(|slot| -> anyhow::Result<SimulationListItem> {
            let simulation = camera.get_simulation(slot)?;
            let name = simulation.get_name()?;
            Ok(SimulationListItem { slot, name })
        })
        .collect::<anyhow::Result<Vec<SimulationListItem>>>()?;

    if *json {
        println!("{}", serde_json::to_string_pretty(&slots)?);
    } else {
        for slot in slots {
            println!("- {slot}");
        }
    }

    Ok(())
}

fn handle_get(options: &GlobalOptions, slot: fuji::CustomSetting) -> anyhow::Result<()> {
    let GlobalOptions {
        json,
        device,
        emulate,
        ..
    } = options;

    let mut camera = usb::get_camera(device.as_deref(), emulate.as_deref())?;

    let simulation = camera.get_simulation(slot)?;

    if *json {
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

fn handle_set(
    options: &GlobalOptions,
    slot: fuji::CustomSetting,
    set_options: &SetFilmSimulationOptions,
    film_options: &FilmSimulationOptions,
) -> anyhow::Result<()> {
    let GlobalOptions {
        device, emulate, ..
    } = options;

    let mut camera = usb::get_camera(device.as_deref(), emulate.as_deref())?;

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
    } = film_options;

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
    options: &GlobalOptions,
    slot: fuji::CustomSetting,
    output: &Output,
) -> anyhow::Result<()> {
    let GlobalOptions {
        device, emulate, ..
    } = options;

    let mut camera = usb::get_camera(device.as_deref(), emulate.as_deref())?;

    let mut writer = output.get_writer()?;
    let simulation = camera.get_simulation(slot)?;
    let simulation = camera.serialize_simulation(&*simulation)?;
    writer.write_all(&simulation)?;

    Ok(())
}

fn handle_import(
    options: &GlobalOptions,
    slot: fuji::CustomSetting,
    input: &Input,
) -> anyhow::Result<()> {
    let GlobalOptions {
        device, emulate, ..
    } = options;

    let mut camera = usb::get_camera(device.as_deref(), emulate.as_deref())?;

    let mut reader = input.get_reader()?;
    let mut simulation = Vec::new();
    reader.read_to_end(&mut simulation)?;
    let simulation = camera.deserialize_simulation(&simulation)?;
    camera.set_simulation(slot, &*simulation)?;

    Ok(())
}

pub fn handle(cmd: SimulationCmd, options: &GlobalOptions) -> anyhow::Result<()> {
    match cmd {
        SimulationCmd::List => handle_list(options),
        SimulationCmd::Get { slot } => handle_get(options, slot),
        SimulationCmd::Set {
            slot,
            set_film_simulation_options,
            film_simulation_options,
        } => handle_set(
            options,
            slot,
            &set_film_simulation_options,
            &film_simulation_options,
        ),
        SimulationCmd::Export { slot, output_file } => handle_export(options, slot, &output_file),
        SimulationCmd::Import { slot, input_file } => handle_import(options, slot, &input_file),
    }
}
