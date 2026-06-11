use std::io::{Read, Write};

use anyhow::Context;
use clap::Subcommand;
use fujicore::{
    features::image::extract_simulation,
    generated::{
        cli::RenderArgs, options::CustomSetting, renders::RenderBase, simulations::SimulationBase,
    },
};

use super::common::file::{Input, Output};
use crate::cli::{GlobalOptions, common::usb};

#[derive(Subcommand, Debug)]
pub enum ImageCmd {
    /// Render image
    #[command(alias = "r")]
    Render {
        /// Simulation slot number
        #[arg(long, conflicts_with = "simulation_file", conflicts_with = "like")]
        slot: Option<CustomSetting>,

        /// Path to exported simulation file
        #[arg(long, conflicts_with = "slot", conflicts_with = "like")]
        simulation_file: Option<Input>,

        /// Path to image whose embedded simulation should be applied (use '-' to read from stdin)
        #[arg(long, conflicts_with = "slot", conflicts_with = "simulation_file")]
        like: Option<Input>,

        /// Render a lower-quality (faster) preview
        #[arg(long)]
        draft: bool,

        #[command(flatten)]
        render: RenderArgs,

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

#[allow(clippy::needless_pass_by_value)]
fn handle_render(
    options: GlobalOptions,
    slot: Option<CustomSetting>,
    simulation_file: Option<Input>,
    like: Option<Input>,
    draft: bool,
    render: RenderArgs,
    input: Input,
    output: Output,
) -> anyhow::Result<()> {
    let GlobalOptions {
        device, emulate, ..
    } = options;

    let mut camera = usb::get_camera(device, emulate)?;

    let mut reader = input.get_reader()?;
    let mut image = Vec::new();
    reader
        .read_to_end(&mut image)
        .context("reading source image from input")?;

    let simulation_base: Option<SimulationBase> = if let Some(slot) = slot {
        Some(
            camera
                .get_simulation(slot)
                .with_context(|| format!("reading simulation slot {slot}"))?
                .to_base(),
        )
    } else if let Some(file) = simulation_file {
        let mut reader = file.get_reader()?;
        let mut buffer = Vec::new();
        reader
            .read_to_end(&mut buffer)
            .context("reading simulation file")?;
        Some(
            camera
                .deserialize_simulation(&buffer)
                .context("deserializing simulation file")?
                .to_base(),
        )
    } else if let Some(like) = like {
        let path = like.into_path()?;
        Some(
            extract_simulation(&path)
                .context("extracting simulation from reference image")?
                .to_base(),
        )
    } else {
        None
    };

    let mut base = RenderBase::default();
    if let Some(sim) = simulation_base {
        base.try_update_from(&sim);
    }
    base.merge(&render.into());

    camera
        .send_image(&image)
        .context("sending image to camera")?;
    camera
        .apply_profile(&base)
        .context("applying conversion profile")?;
    let rendered = camera.render(draft).context("rendering image on camera")?;

    let mut writer = output.get_writer()?;
    writer
        .write_all(&rendered)
        .context("writing rendered image to output")?;

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn handle_extract(input: Input, output: Output) -> anyhow::Result<()> {
    let input = input.into_path()?;

    let simulation =
        extract_simulation(&input).context("extracting simulation from source image")?;

    let serialized =
        serde_json::to_vec(&*simulation).context("serializing extracted simulation to JSON")?;
    let mut writer = output.get_writer()?;
    writer
        .write_all(&serialized)
        .context("writing serialized simulation to output")?;

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
pub fn handle(cmd: ImageCmd, options: GlobalOptions) -> anyhow::Result<()> {
    match cmd {
        ImageCmd::Render {
            slot,
            simulation_file,
            like,
            draft,
            render,
            input,
            output,
        } => handle_render(
            options,
            slot,
            simulation_file,
            like,
            draft,
            render,
            input,
            output,
        ),
        ImageCmd::Extract { input, output } => handle_extract(input, output),
    }
}
