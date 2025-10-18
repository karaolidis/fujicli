use std::fmt;

use crate::{
    camera::ptp::hex::{
        FujiClarity, FujiColor, FujiColorChromeEffect, FujiColorChromeFXBlue, FujiCustomSetting,
        FujiCustomSettingName, FujiDynamicRange, FujiDynamicRangePriority, FujiFilmSimulation,
        FujiGrainEffect, FujiHighISONR, FujiHighlightTone, FujiImageQuality, FujiImageSize,
        FujiShadowTone, FujiSharpness, FujiWhiteBalance, FujiWhiteBalanceShift,
        FujiWhiteBalanceTemperature,
    },
    usb,
};

use super::common::{
    file::{Input, Output},
    film::FilmSimulationOptions,
};
use clap::Subcommand;
use log::warn;
use serde::Serialize;
use strum::IntoEnumIterator;

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomSettingRepr {
    pub slot: FujiCustomSetting,
    pub name: FujiCustomSettingName,
}

fn handle_list(json: bool, device_id: Option<&str>) -> anyhow::Result<()> {
    let mut camera = usb::get_camera(device_id)?;

    let mut slots = Vec::new();

    for slot in FujiCustomSetting::iter() {
        camera.set_active_custom_setting(slot)?;
        let name = camera.get_custom_setting_name()?;
        slots.push(CustomSettingRepr { slot, name });
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&slots)?);
    } else {
        println!("Film Simulations:");
        for slot in slots {
            println!("- {}: {}", slot.slot, slot.name);
        }
    }

    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilmSimulationRepr {
    pub name: FujiCustomSettingName,
    pub simulation: FujiFilmSimulation,
    pub size: FujiImageSize,
    pub quality: FujiImageQuality,
    pub highlight: FujiHighlightTone,
    pub shadow: FujiShadowTone,
    pub color: FujiColor,
    pub sharpness: FujiSharpness,
    pub clarity: FujiClarity,
    pub white_balance: FujiWhiteBalance,
    pub white_balance_shift_red: FujiWhiteBalanceShift,
    pub white_balance_shift_blue: FujiWhiteBalanceShift,
    pub white_balance_temperature: FujiWhiteBalanceTemperature,
    pub dynamic_range: FujiDynamicRange,
    pub dynamic_range_priority: FujiDynamicRangePriority,
    pub noise_reduction: FujiHighISONR,
    pub grain: FujiGrainEffect,
    pub color_chrome_effect: FujiColorChromeEffect,
    pub color_chrome_fx_blue: FujiColorChromeFXBlue,
}

impl fmt::Display for FilmSimulationRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Name: {}", self.name)?;
        writeln!(f, "Simulation: {}", self.simulation)?;
        writeln!(f, "Size: {}", self.size)?;
        writeln!(f, "Quality: {}", self.quality)?;
        writeln!(f, "Highlights: {}", self.highlight)?;
        writeln!(f, "Shadows: {}", self.shadow)?;
        writeln!(f, "Color: {}", self.color)?;
        writeln!(f, "Sharpness: {}", self.sharpness)?;
        writeln!(f, "Clarity: {}", self.clarity)?;
        writeln!(f, "White Balance: {}", self.white_balance)?;
        writeln!(
            f,
            "White Balance Shift (R/B): {} / {}",
            self.white_balance_shift_red, self.white_balance_shift_blue
        )?;
        writeln!(
            f,
            "White Balance Temperature: {}K",
            self.white_balance_temperature
        )?;
        writeln!(f, "Dynamic Range: {}", self.dynamic_range)?;
        writeln!(f, "Dynamic Range Priority: {}", self.dynamic_range_priority)?;
        writeln!(f, "Noise Reduction: {}", self.noise_reduction)?;
        writeln!(f, "Grain: {}", self.grain)?;
        writeln!(f, "Color Chrome Effect: {}", self.color_chrome_effect)?;
        writeln!(f, "Color Chrome FX Blue: {}", self.color_chrome_fx_blue)
    }
}

fn handle_get(json: bool, device_id: Option<&str>, slot: FujiCustomSetting) -> anyhow::Result<()> {
    let mut camera = usb::get_camera(device_id)?;
    camera.set_active_custom_setting(slot)?;

    let repr = FilmSimulationRepr {
        name: camera.get_custom_setting_name()?,
        simulation: camera.get_film_simulation()?,
        size: camera.get_image_size()?,
        quality: camera.get_image_quality()?,
        highlight: camera.get_highlight_tone()?,
        shadow: camera.get_shadow_tone()?,
        color: camera.get_color()?,
        sharpness: camera.get_sharpness()?,
        clarity: camera.get_clarity()?,
        white_balance: camera.get_white_balance()?,
        white_balance_shift_red: camera.get_white_balance_shift_red()?,
        white_balance_shift_blue: camera.get_white_balance_shift_blue()?,
        white_balance_temperature: camera.get_white_balance_temperature()?,
        dynamic_range: camera.get_dynamic_range()?,
        dynamic_range_priority: camera.get_dynamic_range_priority()?,
        noise_reduction: camera.get_high_iso_nr()?,
        grain: camera.get_grain_effect()?,
        color_chrome_effect: camera.get_color_chrome_effect()?,
        color_chrome_fx_blue: camera.get_color_chrome_fx_blue()?,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&repr)?);
    } else {
        println!("{repr}");
    }

    Ok(())
}

fn handle_set(
    device_id: Option<&str>,
    slot: FujiCustomSetting,
    options: &FilmSimulationOptions,
) -> anyhow::Result<()> {
    let mut camera = usb::get_camera(device_id)?;
    camera.set_active_custom_setting(slot)?;

    // General
    if let Some(name) = &options.name {
        camera.set_custom_setting_name(name)?;
    }

    if let Some(size) = &options.size {
        camera.set_image_size(*size)?;
    }

    if let Some(quality) = &options.quality {
        camera.set_image_quality(*quality)?;
    }

    // Style
    if let Some(simulation) = &options.simulation {
        camera.set_film_simulation(*simulation)?;
    }

    if let Some(color) = &options.color {
        camera.set_color(*color)?;
    }

    if let Some(sharpness) = &options.sharpness {
        camera.set_sharpness(*sharpness)?;
    }

    if let Some(clarity) = &options.clarity {
        camera.set_clarity(*clarity)?;
    }

    if let Some(noise_reduction) = &options.noise_reduction {
        camera.set_high_iso_nr(*noise_reduction)?;
    }

    if let Some(grain) = &options.grain {
        camera.set_grain_effect(*grain)?;
    }

    if let Some(color_chrome_effect) = &options.color_chrome_effect {
        camera.set_color_chrome_effect(*color_chrome_effect)?;
    }

    if let Some(color_chrome_fx_blue) = &options.color_chrome_fx_blue {
        camera.set_color_chrome_fx_blue(*color_chrome_fx_blue)?;
    }

    // White Balance
    if let Some(white_balance) = &options.white_balance {
        camera.set_white_balance(*white_balance)?;
    }

    if let Some(temperature) = &options.white_balance_temperature {
        let white_balance = if let Some(white_balance) = &options.white_balance {
            white_balance
        } else {
            &camera.get_white_balance()?
        };

        if *white_balance != FujiWhiteBalance::Temperature {
            warn!("White Balance mode is not set to 'Temperature', refusing to set temperature")
        } else {
            camera.set_white_balance_temperature(*temperature)?;
        }
    }

    if let Some(shift_red) = &options.white_balance_shift_red {
        camera.set_white_balance_shift_red(*shift_red)?;
    }

    if let Some(shift_blue) = &options.white_balance_shift_blue {
        camera.set_white_balance_shift_blue(*shift_blue)?;
    }

    // Exposure
    if let Some(dynamic_range_priority) = &options.dynamic_range_priority {
        camera.set_dynamic_range_priority(*dynamic_range_priority)?;
    }

    if options.dynamic_range.is_some() || options.highlight.is_some() || options.shadow.is_some() {
        let dynamic_range_priority =
            if let Some(dynamic_range_priority) = &options.dynamic_range_priority {
                dynamic_range_priority
            } else {
                &camera.get_dynamic_range_priority()?
            };

        if let Some(dynamic_range) = &options.dynamic_range {
            if *dynamic_range_priority != FujiDynamicRangePriority::Off {
                warn!("Dynamic Range Priority is enabled, refusing to set dynamic range")
            } else {
                camera.set_dynamic_range(*dynamic_range)?;
            }
        }

        if let Some(highlights) = &options.highlight {
            if *dynamic_range_priority != FujiDynamicRangePriority::Off {
                warn!("Dynamic Range Priority is enabled, refusing to set highlight tone")
            } else {
                camera.set_highlight_tone(*highlights)?;
            }
        }

        if let Some(shadows) = &options.shadow {
            if *dynamic_range_priority != FujiDynamicRangePriority::Off {
                warn!("Dynamic Range Priority is enabled, refusing to set shadow tone")
            } else {
                camera.set_shadow_tone(*shadows)?;
            }
        }
    }

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
            film_simulation_options,
        } => handle_set(device_id, slot, &film_simulation_options),
        SimulationCmd::Export { slot, output_file } => handle_export(device_id, slot, &output_file),
        SimulationCmd::Import { slot, input_file } => handle_import(device_id, slot, &input_file),
    }
}
