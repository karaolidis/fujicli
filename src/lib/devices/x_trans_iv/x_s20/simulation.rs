use std::fmt;

use log::error;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use crate::{
    devices::x_trans_iv::x_s20::FujifilmXS20,
    features::simulation::{
        CameraSimulationManager, CameraSimulationParser, Simulation, define_simulation,
    },
    ptp::{DevicePropCode, Ptp, fuji},
};

define_simulation!(
    XS20Simulation,
    [
        ImageSize,
        ImageQuality,
        FilmSimulation,
        MonochromaticColorShift,
        Highlight,
        Shadow,
        Color,
        Sharpness,
        Clarity,
        NoiseReduction,
        GrainEffect,
        ColorChromeEffect,
        ColorChromeFXBlue,
        WhiteBalance,
        WhiteBalanceShift,
        WhiteBalanceTemperature,
        DynamicRange,
        DynamicRangePriority,
        LensModulationOptimizer,
        ColorSpace,
    ]
);

impl CameraSimulationParser for FujifilmXS20 {
    fn deserialize_simulation(&self, simulation: &[u8]) -> anyhow::Result<Box<dyn Simulation>> {
        let simulation: XS20Simulation = serde_json::from_slice(simulation)?;
        Ok(Box::new(simulation))
    }

    fn serialize_simulation(&self, simulation: &dyn Simulation) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(&simulation)?)
    }
}

impl CameraSimulationManager for FujifilmXS20 {
    fn custom_settings_slots(&self) -> Vec<fuji::CustomSetting> {
        fuji::CustomSetting::iter().take(4).collect()
    }

    fn get_simulation(
        &self,
        ptp: &mut Ptp,
        slot: fuji::CustomSetting,
    ) -> anyhow::Result<Box<dyn Simulation>> {
        ptp.set_prop(DevicePropCode::FujiCustomSetting, &slot)?;

        let name = ptp.get_prop(DevicePropCode::FujiCustomSettingName)?;
        let size = ptp.get_prop(DevicePropCode::FujiCustomSettingImageSize)?;
        let quality = ptp.get_prop(DevicePropCode::FujiCustomSettingImageQuality)?;
        let simulation: fuji::FilmSimulation =
            ptp.get_prop(DevicePropCode::FujiCustomSettingFilmSimulation)?;
        let monochromatic_color_temperature =
            ptp.get_prop(DevicePropCode::FujiCustomSettingMonochromaticColorTemperature)?;
        let monochromatic_color_tint =
            ptp.get_prop(DevicePropCode::FujiCustomSettingMonochromaticColorTint)?;
        let dynamic_range_priority =
            ptp.get_prop(DevicePropCode::FujiCustomSettingDynamicRangePriority)?;
        let dynamic_range = ptp.get_prop(DevicePropCode::FujiCustomSettingDynamicRange)?;
        let highlight = ptp.get_prop(DevicePropCode::FujiCustomSettingHighlightTone)?;
        let shadow = ptp.get_prop(DevicePropCode::FujiCustomSettingShadowTone)?;
        let color = ptp.get_prop(DevicePropCode::FujiCustomSettingColor)?;
        let sharpness = ptp.get_prop(DevicePropCode::FujiCustomSettingSharpness)?;
        let clarity = ptp.get_prop(DevicePropCode::FujiCustomSettingClarity)?;
        let noise_reduction = ptp.get_prop(DevicePropCode::FujiCustomSettingHighISONR)?;
        let grain = ptp.get_prop(DevicePropCode::FujiCustomSettingGrainEffect)?;
        let color_chrome_effect =
            ptp.get_prop(DevicePropCode::FujiCustomSettingColorChromeEffect)?;
        let color_chrome_fx_blue =
            ptp.get_prop(DevicePropCode::FujiCustomSettingColorChromeFXBlue)?;
        let white_balance =
            ptp.get_prop::<fuji::WhiteBalance>(DevicePropCode::FujiCustomSettingWhiteBalance)?;
        let white_balance_shift_red =
            ptp.get_prop(DevicePropCode::FujiCustomSettingWhiteBalanceShiftRed)?;
        let white_balance_shift_blue =
            ptp.get_prop(DevicePropCode::FujiCustomSettingWhiteBalanceShiftBlue)?;
        let white_balance_temperature =
            ptp.get_prop(DevicePropCode::FujiCustomSettingWhiteBalanceTemperature)?;
        let lens_modulation_optimizer =
            ptp.get_prop(DevicePropCode::FujiCustomSettingLensModulationOptimizer)?;
        let color_space = ptp.get_prop(DevicePropCode::FujiCustomSettingColorSpace)?;

        let sim = XS20Simulation {
            name,
            size,
            quality,
            simulation,
            monochromatic_color_temperature,
            monochromatic_color_tint,
            dynamic_range_priority,
            dynamic_range,
            highlight,
            shadow,
            color,
            sharpness,
            clarity,
            noise_reduction,
            grain,
            color_chrome_effect,
            color_chrome_fx_blue,
            white_balance,
            white_balance_shift_red,
            white_balance_shift_blue,
            white_balance_temperature,
            lens_modulation_optimizer,
            color_space,
        };

        Ok(Box::new(sim))
    }

    fn update_simulation(
        &self,
        ptp: &mut Ptp,
        slot: fuji::CustomSetting,
        simulation_modifier: &mut dyn FnMut(&mut dyn Simulation) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let original_simulation = self.get_simulation(ptp, slot)?;
        let original_simulation = original_simulation
            .as_any()
            .downcast_ref::<XS20Simulation>()
            .expect("Simulation type mismatch");

        let mut updated_simulation = original_simulation.clone();
        simulation_modifier(&mut updated_simulation)?;

        if let Err(error) = self.set_simulation(ptp, slot, &updated_simulation) {
            error!("Error updating simulation options: {error}. Restoring previous options.");
            self.set_simulation(ptp, slot, original_simulation)?;
        }

        Ok(())
    }

    fn set_simulation(
        &self,
        ptp: &mut Ptp,
        slot: fuji::CustomSetting,
        simulation: &dyn Simulation,
    ) -> anyhow::Result<()> {
        let simulation = simulation
            .as_any()
            .downcast_ref::<XS20Simulation>()
            .expect("Simulation type mismatch");

        ptp.set_prop(DevicePropCode::FujiCustomSetting, &slot)?;

        ptp.set_prop(DevicePropCode::FujiCustomSettingName, &simulation.name)?;
        ptp.set_prop(DevicePropCode::FujiCustomSettingImageSize, &simulation.size)?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingImageQuality,
            &simulation.quality,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingFilmSimulation,
            &simulation.simulation,
        )?;
        if simulation.simulation.is_black_and_white() {
            ptp.set_prop(
                DevicePropCode::FujiCustomSettingMonochromaticColorTemperature,
                &simulation.monochromatic_color_temperature,
            )?;
            ptp.set_prop(
                DevicePropCode::FujiCustomSettingMonochromaticColorTint,
                &simulation.monochromatic_color_tint,
            )?;
        }
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingDynamicRangePriority,
            &simulation.dynamic_range_priority,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingDynamicRange,
            &simulation.dynamic_range,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingHighlightTone,
            &simulation.highlight,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingShadowTone,
            &simulation.shadow,
        )?;
        ptp.set_prop(DevicePropCode::FujiCustomSettingColor, &simulation.color)?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingSharpness,
            &simulation.sharpness,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingClarity,
            &simulation.clarity,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingHighISONR,
            &simulation.noise_reduction,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingGrainEffect,
            &simulation.grain,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingColorChromeEffect,
            &simulation.color_chrome_effect,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingColorChromeFXBlue,
            &simulation.color_chrome_fx_blue,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingWhiteBalance,
            &simulation.white_balance,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingWhiteBalanceShiftRed,
            &simulation.white_balance_shift_red,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingWhiteBalanceShiftBlue,
            &simulation.white_balance_shift_blue,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingWhiteBalanceTemperature,
            &simulation.white_balance_temperature,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingLensModulationOptimizer,
            &simulation.lens_modulation_optimizer,
        )?;
        ptp.set_prop(
            DevicePropCode::FujiCustomSettingColorSpace,
            &simulation.color_space,
        )?;

        Ok(())
    }
}
