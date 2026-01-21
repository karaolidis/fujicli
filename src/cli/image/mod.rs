use fujicli::{features::image::extract_simulation, ptp::fuji, usb};

use super::common::{
    file::{Input, Output},
    film::FilmSimulationOptions,
};
use crate::cli::GlobalOptions;
use clap::{Args, Subcommand};

#[derive(Subcommand, Debug)]
pub enum ImageCmd {
    /// Render image
    #[command(alias = "r")]
    Render {
        /// Simulation slot number
        #[arg(long, conflicts_with = "simulation_file", conflicts_with = "like")]
        slot: Option<fuji::CustomSetting>,

        /// Path to exported simulation file
        #[arg(long, conflicts_with = "slot", conflicts_with = "like")]
        simulation_file: Option<Input>,

        /// Path to image to mimic simulation settings from (use '-' to read from stdin)
        #[arg(long, conflicts_with = "slot", conflicts_with = "simulation_file")]
        like: Option<Input>,

        #[command(flatten)]
        render_options: RenderOptions,

        #[command(flatten)]
        film_simulation_options: FilmSimulationOptions,

        /// RAF input file (use '-' to read from stdin)
        input: Input,

        /// Output file (use '-' to write to stdout)
        output: Output,
    },

    /// Extract simulation from image
    #[command(alias = "e")]
    Extract {
        /// Input file (use '-' to read from stdin)
        input: Input,

        /// Output file (use '-' to write to stdout)
        output: Output,
    },
}

#[derive(Args, Debug)]
pub struct RenderOptions {
    /// Render a lower quality image
    #[clap(long)]
    draft: bool,

    /// Output file format
    #[clap(long)]
    file_type: Option<fuji::FileType>,

    /// Push/Pull exposure compensation
    #[clap(long, allow_hyphen_values(true))]
    exposure_offset: Option<fuji::ExposureOffset>,

    /// Teleconverter
    #[clap(long)]
    teleconverter: Option<fuji::Teleconverter>,
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

#[allow(clippy::too_many_arguments)]
fn handle_render(
    options: &GlobalOptions,
    render_options: &RenderOptions,
    film_options: &FilmSimulationOptions,
    input: Input,
    output: Output,
    slot: Option<fuji::CustomSetting>,
    simulation_file: Option<Input>,
    like: Option<Input>,
) -> anyhow::Result<()> {
    let GlobalOptions {
        device, emulate, ..
    } = options;

    let mut camera = usb::get_camera(device.as_deref(), emulate.as_deref())?;

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
    } = film_options;

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
    } else if let Some(like) = like {
        let like = like.as_path()?;
        Some(extract_simulation(&*like)?)
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

fn handle_extract(input: Input, output: Output) -> anyhow::Result<()> {
    let input = input.as_path()?;

    let simulation = extract_simulation(&*input)?;

    let serialized = simulation.serialize()?;
    let mut writer = output.get_writer()?;
    writer.write_all(&serialized)?;

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
pub fn handle(cmd: ImageCmd, options: &GlobalOptions) -> anyhow::Result<()> {
    match cmd {
        ImageCmd::Render {
            slot,
            simulation_file,
            like,
            input,
            output,
            render_options,
            film_simulation_options,
        } => handle_render(
            options,
            &render_options,
            &film_simulation_options,
            input,
            output,
            slot,
            simulation_file,
            like,
        ),
        ImageCmd::Extract { input, output } => handle_extract(input, output),
    }
}
