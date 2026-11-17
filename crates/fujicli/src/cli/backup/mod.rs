use anyhow::Context;
use clap::Subcommand;
use fujicore::features::backup::x_t30_iii::{self, SlotEdit};

use super::common::file::{Input, Output};
use crate::cli::{GlobalOptions, common::usb};

#[derive(Subcommand, Debug, Clone)]
pub enum BackupCmd {
    /// Export backup
    #[command(alias = "e")]
    Export {
        /// Output file (use '-' to write to stdout)
        output: Output,
    },

    /// Import backup
    #[command(alias = "i")]
    Import {
        /// Input file (use '-' to read from stdin)
        input: Input,
    },

    /// Show the recipes stored in an X-T30 III backup file (C1-C7 and FS1-FS3)
    #[command(alias = "r")]
    Recipes {
        /// Backup file (use '-' to read from stdin)
        input: Input,

        /// Output JSON file (use '-' to write to stdout)
        output: Output,
    },

    /// Write settings USB can't into an X-T30 III backup file, then load it with `backup import`
    Edit {
        /// Backup file (use '-' to read from stdin)
        input: Input,

        /// Edited backup file (use '-' to write to stdout)
        output: Output,

        /// Custom slot to edit (1-7)
        #[arg(long, conflicts_with = "fs", required_unless_present = "fs")]
        slot: Option<usize>,

        /// Film simulation dial position to edit (1-3)
        #[arg(long)]
        fs: Option<usize>,

        /// Slot Monochromatic Color WC (-18..18)
        #[arg(long, requires = "slot", allow_hyphen_values = true)]
        monochromatic_color_temperature: Option<i64>,

        /// Slot Monochromatic Color MG (-18..18)
        #[arg(long, requires = "slot", allow_hyphen_values = true)]
        monochromatic_color_tint: Option<i64>,

        /// Slot Portrait Enhancer (off, weak, medium, strong)
        #[arg(long, requires = "slot")]
        portrait_enhancer: Option<String>,

        /// Start the FS recipe from a copy of this custom slot (1-7) and switch it on
        #[arg(long, requires = "fs")]
        from_slot: Option<usize>,

        /// FS recipe value, with the keys printed by `backup recipes` (e.g. clarity=-2)
        #[arg(long = "set", value_name = "KEY=VALUE", requires = "fs", value_parser = parse_key_value, allow_hyphen_values = true)]
        set: Vec<(String, String)>,
    },
}

fn parse_key_value(s: &str) -> Result<(String, String), String> {
    s.split_once('=')
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .ok_or_else(|| format!("expected KEY=VALUE, got {s:?}"))
}

#[allow(clippy::needless_pass_by_value)]
fn read_input(input: Input) -> anyhow::Result<Vec<u8>> {
    let mut reader = input.get_reader()?;
    let mut data = Vec::new();
    reader
        .read_to_end(&mut data)
        .context("reading backup file")?;
    Ok(data)
}

#[allow(clippy::needless_pass_by_value)]
fn write_output(output: Output, data: &[u8]) -> anyhow::Result<()> {
    let mut writer = output.get_writer()?;
    writer.write_all(data).context("writing output")?;
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn handle_recipes(input: Input, output: Output) -> anyhow::Result<()> {
    let recipes = x_t30_iii::recipes(&read_input(input)?).context("decoding backup file")?;
    let json = serde_json::to_vec_pretty(&recipes).context("serializing recipes")?;
    write_output(output, &json)
}

#[allow(clippy::needless_pass_by_value)]
fn handle_edit(
    input: Input,
    output: Output,
    slot: Option<usize>,
    fs: Option<usize>,
    edit: SlotEdit,
    from_slot: Option<usize>,
    set: Vec<(String, String)>,
) -> anyhow::Result<()> {
    let data = read_input(input)?;
    let edited = match (slot, fs) {
        (Some(slot), _) => x_t30_iii::edit_slot(&data, slot, &edit),
        (None, Some(fs)) => x_t30_iii::edit_fs(&data, fs, from_slot, &set),
        (None, None) => unreachable!("clap requires --slot or --fs"),
    }
    .context("editing backup file")?;
    write_output(output, &edited)
}

#[allow(clippy::needless_pass_by_value)]
fn handle_export(options: GlobalOptions, output: Output) -> anyhow::Result<()> {
    let GlobalOptions {
        device, emulate, ..
    } = options;

    let mut camera = usb::get_camera(device.as_deref(), emulate)?;

    let backup = camera.export_backup().context("exporting camera backup")?;
    let mut writer = output.get_writer()?;
    writer
        .write_all(&backup)
        .context("writing backup to output")?;

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn handle_import(options: GlobalOptions, input: Input) -> anyhow::Result<()> {
    let GlobalOptions {
        device, emulate, ..
    } = options;

    let mut camera = usb::get_camera(device.as_deref(), emulate)?;

    let mut reader = input.get_reader()?;
    let mut backup = Vec::new();
    reader
        .read_to_end(&mut backup)
        .context("reading backup from input")?;
    camera
        .import_backup(&backup)
        .context("importing camera backup")?;

    Ok(())
}

pub fn handle(cmd: BackupCmd, options: GlobalOptions) -> anyhow::Result<()> {
    match cmd {
        BackupCmd::Export { output } => handle_export(options, output),
        BackupCmd::Import { input } => handle_import(options, input),
        BackupCmd::Recipes { input, output } => handle_recipes(input, output),
        BackupCmd::Edit {
            input,
            output,
            slot,
            fs,
            monochromatic_color_temperature,
            monochromatic_color_tint,
            portrait_enhancer,
            from_slot,
            set,
        } => handle_edit(
            input,
            output,
            slot,
            fs,
            SlotEdit {
                monochromatic_color_temperature,
                monochromatic_color_tint,
                portrait_enhancer,
            },
            from_slot,
            set,
        ),
    }
}
