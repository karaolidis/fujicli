use crate::{
    camera::ptp::hex::{FujiCustomSetting, FujiExposureOffset, FujiFileType, FujiTeleconverter},
    usb,
};

use super::common::{
    file::{Input, Output},
    film::FilmSimulationOptions,
};
use clap::Args;

#[derive(Args, Debug)]
pub struct RenderCmd {
    /// Simulation slot number
    #[arg(long, conflicts_with = "simulation_file")]
    slot: Option<FujiCustomSetting>,

    /// Path to exported simulation file
    #[arg(long, conflicts_with = "slot")]
    simulation_file: Option<Input>,

    #[command(flatten)]
    render_options: RenderOptions,

    #[command(flatten)]
    film_simulation_options: FilmSimulationOptions,

    /// RAF input file (use '-' to read from stdin)
    input: Input,

    /// Output file (use '-' to write to stdout)
    output: Output,
}

#[derive(Args, Debug)]
pub struct RenderOptions {
    /// Render a lower quality image
    #[clap(long)]
    draft: bool,

    /// Output file format
    #[clap(long)]
    file_type: Option<FujiFileType>,

    /// Push/Pull exposure compensation
    #[clap(long)]
    exposure_offset: Option<FujiExposureOffset>,

    /// Teleconverter
    #[clap(long)]
    teleconverter: Option<FujiTeleconverter>,
}

macro_rules! update_conversion_profile {
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

fn handle_render(
    device_id: Option<&str>,
    input: &Input,
    output: &Output,
    render_options: &RenderOptions,
    options: &FilmSimulationOptions,
    slot: Option<FujiCustomSetting>,
    simulation_file: Option<Input>,
) -> anyhow::Result<()> {
    let mut camera = usb::get_camera(device_id)?;

    let RenderOptions {
        draft,
        file_type,
        exposure_offset,
        teleconverter,
    } = render_options;

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

    let mut reader = input.get_reader()?;
    let mut image = Vec::new();
    reader.read_to_end(&mut image)?;

    let simulation = if let Some(slot) = slot {
        Some(camera.get_simulation(slot)?)
    } else if let Some(simulation_file) = simulation_file {
        let mut reader = simulation_file.get_reader()?;
        let mut simulation = Vec::new();
        reader.read_to_end(&mut simulation)?;
        Some(camera.deserialize_simulation(&simulation)?)
    } else {
        None
    };

    let rendered = camera.render(
        &image,
        #[allow(clippy::cognitive_complexity)]
        &mut |conversion_profile| {
            if let Some(simulation) = simulation.as_deref() {
                conversion_profile.set_from_simulation(simulation)?;
            }

            update_conversion_profile! {
                conversion_profile,
                [
                    file_type => set_file_type,
                    film_simulation => set_simulation,
                    monochromatic_color_temperature => set_monochromatic_color_temperature,
                    monochromatic_color_tint => set_monochromatic_color_tint,
                    size => set_size,
                    quality => set_quality,
                    exposure_offset => set_exposure_offset,
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
                    teleconverter => set_teleconverter,
                ]
            };

            Ok(())
        },
        *draft,
    )?;

    let mut writer = output.get_writer()?;
    writer.write_all(&rendered)?;

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
pub fn handle(cmd: RenderCmd, device_id: Option<&str>) -> anyhow::Result<()> {
    handle_render(
        device_id,
        &cmd.input,
        &cmd.output,
        &cmd.render_options,
        &cmd.film_simulation_options,
        cmd.slot,
        cmd.simulation_file,
    )
}
