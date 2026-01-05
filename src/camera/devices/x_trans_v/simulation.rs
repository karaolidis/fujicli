use std::{any::Any, fmt};

use log::error;
use serde::{Deserialize, Serialize};

use crate::camera::{
    features::simulation::{CameraSimulations, simulation::Simulation},
    ptp::{
        Ptp,
        hex::{
            DevicePropCode, FujiClarity, FujiColor, FujiColorChromeEffect, FujiColorChromeFXBlue,
            FujiColorSpace, FujiCustomSetting, FujiCustomSettingName, FujiDynamicRange,
            FujiDynamicRangePriority, FujiFilmSimulation, FujiGrainEffect, FujiHighISONR,
            FujiHighlightTone, FujiImageQuality, FujiImageSize, FujiLensModulationOptimizer,
            FujiMonochromaticColorShift, FujiShadowTone, FujiSharpness, FujiSmoothSkinEffect,
            FujiWhiteBalance, FujiWhiteBalanceShift, FujiWhiteBalanceTemperature,
        },
    },
};

use super::XTransV;

// NOTE: Naively assuming that all cameras using the same sensor
// also have the same simulation feature set.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XTransVSimulation {
    pub name: FujiCustomSettingName,
    pub size: FujiImageSize,
    pub quality: FujiImageQuality,
    #[allow(clippy::struct_field_names)]
    pub simulation: FujiFilmSimulation,
    pub monochromatic_color_temperature: FujiMonochromaticColorShift,
    pub monochromatic_color_tint: FujiMonochromaticColorShift,
    pub dynamic_range_priority: FujiDynamicRangePriority,
    pub dynamic_range: FujiDynamicRange,
    pub highlight: FujiHighlightTone,
    pub shadow: FujiShadowTone,
    pub color: FujiColor,
    pub sharpness: FujiSharpness,
    pub clarity: FujiClarity,
    pub noise_reduction: FujiHighISONR,
    pub grain: FujiGrainEffect,
    pub color_chrome_effect: FujiColorChromeEffect,
    pub color_chrome_fx_blue: FujiColorChromeFXBlue,
    pub smooth_skin_effect: FujiSmoothSkinEffect,
    pub white_balance: FujiWhiteBalance,
    pub white_balance_shift_red: FujiWhiteBalanceShift,
    pub white_balance_shift_blue: FujiWhiteBalanceShift,
    pub white_balance_temperature: FujiWhiteBalanceTemperature,
    pub lens_modulation_optimizer: FujiLensModulationOptimizer,
    pub color_space: FujiColorSpace,
}

impl fmt::Display for XTransVSimulation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Name: {}", self.name)?;
        writeln!(f, "Size: {}", self.size)?;
        writeln!(f, "Quality: {}", self.quality)?;
        writeln!(f, "Simulation: {}", self.simulation)?;
        writeln!(
            f,
            "Monochromatic Color Temperature: {}",
            self.monochromatic_color_temperature
        )?;
        writeln!(
            f,
            "Monochromatic Color Tint: {}",
            self.monochromatic_color_tint
        )?;
        writeln!(f, "Dynamic Range Priority: {}", self.dynamic_range_priority)?;
        writeln!(f, "Dynamic Range: {}", self.dynamic_range)?;
        writeln!(f, "Highlights: {}", self.highlight)?;
        writeln!(f, "Shadows: {}", self.shadow)?;
        writeln!(f, "Color: {}", self.color)?;
        writeln!(f, "Sharpness: {}", self.sharpness)?;
        writeln!(f, "Clarity: {}", self.clarity)?;
        writeln!(f, "Noise Reduction: {}", self.noise_reduction)?;
        writeln!(f, "Grain: {}", self.grain)?;
        writeln!(f, "Color Chrome Effect: {}", self.color_chrome_effect)?;
        writeln!(f, "Color Chrome FX Blue: {}", self.color_chrome_fx_blue)?;
        writeln!(f, "Smooth Skin Effect: {}", self.smooth_skin_effect)?;

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
        writeln!(
            f,
            "Lens Modulation Optimizer: {}",
            self.lens_modulation_optimizer
        )?;
        writeln!(f, "Color Space: {}", self.color_space)
    }
}

impl Simulation for XTransVSimulation {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn get_name(&self) -> anyhow::Result<FujiCustomSettingName> {
        Ok(self.name.clone())
    }

    fn set_name(&mut self, value: &FujiCustomSettingName) -> anyhow::Result<()> {
        self.name = value.clone();
        Ok(())
    }

    fn set_size(&mut self, value: &FujiImageSize) -> anyhow::Result<()> {
        self.size = *value;
        Ok(())
    }

    fn set_quality(&mut self, value: &FujiImageQuality) -> anyhow::Result<()> {
        self.quality = *value;
        Ok(())
    }

    fn set_simulation(&mut self, value: &FujiFilmSimulation) -> anyhow::Result<()> {
        self.simulation = *value;
        Ok(())
    }

    fn set_monochromatic_color_temperature(
        &mut self,
        value: &FujiMonochromaticColorShift,
    ) -> anyhow::Result<()> {
        self.monochromatic_color_temperature = *value;
        Ok(())
    }

    fn set_monochromatic_color_tint(
        &mut self,
        value: &FujiMonochromaticColorShift,
    ) -> anyhow::Result<()> {
        self.monochromatic_color_tint = *value;
        Ok(())
    }

    fn set_dynamic_range_priority(
        &mut self,
        value: &FujiDynamicRangePriority,
    ) -> anyhow::Result<()> {
        self.dynamic_range_priority = *value;
        Ok(())
    }

    fn set_dynamic_range(&mut self, value: &FujiDynamicRange) -> anyhow::Result<()> {
        self.dynamic_range = *value;
        Ok(())
    }

    fn set_highlight(&mut self, value: &FujiHighlightTone) -> anyhow::Result<()> {
        self.highlight = *value;
        Ok(())
    }

    fn set_shadow(&mut self, value: &FujiShadowTone) -> anyhow::Result<()> {
        self.shadow = *value;
        Ok(())
    }

    fn set_color(&mut self, value: &FujiColor) -> anyhow::Result<()> {
        self.color = *value;
        Ok(())
    }

    fn set_sharpness(&mut self, value: &FujiSharpness) -> anyhow::Result<()> {
        self.sharpness = *value;
        Ok(())
    }

    fn set_clarity(&mut self, value: &FujiClarity) -> anyhow::Result<()> {
        self.clarity = *value;
        Ok(())
    }

    fn set_noise_reduction(&mut self, value: &FujiHighISONR) -> anyhow::Result<()> {
        self.noise_reduction = *value;
        Ok(())
    }

    fn set_grain(&mut self, value: &FujiGrainEffect) -> anyhow::Result<()> {
        self.grain = *value;
        Ok(())
    }

    fn set_color_chrome_effect(&mut self, value: &FujiColorChromeEffect) -> anyhow::Result<()> {
        self.color_chrome_effect = *value;
        Ok(())
    }

    fn set_color_chrome_fx_blue(&mut self, value: &FujiColorChromeFXBlue) -> anyhow::Result<()> {
        self.color_chrome_fx_blue = *value;
        Ok(())
    }

    fn set_smooth_skin_effect(&mut self, value: &FujiSmoothSkinEffect) -> anyhow::Result<()> {
        self.smooth_skin_effect = *value;
        Ok(())
    }

    fn set_white_balance(&mut self, value: &FujiWhiteBalance) -> anyhow::Result<()> {
        self.white_balance = *value;
        Ok(())
    }

    fn set_white_balance_shift_red(&mut self, value: &FujiWhiteBalanceShift) -> anyhow::Result<()> {
        self.white_balance_shift_red = *value;
        Ok(())
    }

    fn set_white_balance_shift_blue(
        &mut self,
        value: &FujiWhiteBalanceShift,
    ) -> anyhow::Result<()> {
        self.white_balance_shift_blue = *value;
        Ok(())
    }

    fn set_white_balance_temperature(
        &mut self,
        value: &FujiWhiteBalanceTemperature,
    ) -> anyhow::Result<()> {
        self.white_balance_temperature = *value;
        Ok(())
    }

    fn set_lens_modulation_optimizer(
        &mut self,
        value: &FujiLensModulationOptimizer,
    ) -> anyhow::Result<()> {
        self.lens_modulation_optimizer = *value;
        Ok(())
    }

    fn set_color_space(&mut self, value: &FujiColorSpace) -> anyhow::Result<()> {
        self.color_space = *value;
        Ok(())
    }
}

impl<T> CameraSimulations for T
where
    T: XTransV,
{
    fn get_simulation(
        &self,
        ptp: &mut Ptp,
        slot: FujiCustomSetting,
    ) -> anyhow::Result<Box<dyn Simulation>> {
        ptp.set_prop(DevicePropCode::FujiCustomSetting, &slot)?;

        let name = ptp.get_prop(DevicePropCode::FujiCustomSettingName)?;
        let size = ptp.get_prop(DevicePropCode::FujiCustomSettingImageSize)?;
        let quality = ptp.get_prop(DevicePropCode::FujiCustomSettingImageQuality)?;
        let simulation: FujiFilmSimulation =
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
        let smooth_skin_effect = ptp.get_prop(DevicePropCode::FujiCustomSettingSmoothSkinEffect)?;
        let white_balance =
            ptp.get_prop::<FujiWhiteBalance>(DevicePropCode::FujiCustomSettingWhiteBalance)?;
        let white_balance_shift_red =
            ptp.get_prop(DevicePropCode::FujiCustomSettingWhiteBalanceShiftRed)?;
        let white_balance_shift_blue =
            ptp.get_prop(DevicePropCode::FujiCustomSettingWhiteBalanceShiftBlue)?;
        let white_balance_temperature =
            ptp.get_prop(DevicePropCode::FujiCustomSettingWhiteBalanceTemperature)?;
        let lens_modulation_optimizer =
            ptp.get_prop(DevicePropCode::FujiCustomSettingLensModulationOptimizer)?;
        let color_space = ptp.get_prop(DevicePropCode::FujiCustomSettingColorSpace)?;

        let sim = XTransVSimulation {
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
            smooth_skin_effect,
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
        slot: FujiCustomSetting,
        simulation_modifier: &mut dyn FnMut(&mut dyn Simulation) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let original_simulation = self.get_simulation(ptp, slot)?;
        let original_simulation = original_simulation
            .as_any()
            .downcast_ref::<XTransVSimulation>()
            .unwrap();

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
        slot: FujiCustomSetting,
        simulation: &dyn Simulation,
    ) -> anyhow::Result<()> {
        let simulation = simulation
            .as_any()
            .downcast_ref::<XTransVSimulation>()
            .unwrap();

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
            DevicePropCode::FujiCustomSettingSmoothSkinEffect,
            &simulation.smooth_skin_effect,
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

    fn export_simulation(&self, ptp: &mut Ptp, slot: FujiCustomSetting) -> anyhow::Result<Vec<u8>> {
        let simulation = self.get_simulation(ptp, slot)?;
        Ok(serde_json::to_vec(&simulation)?)
    }

    fn import_simulation(
        &self,
        ptp: &mut Ptp,
        slot: FujiCustomSetting,
        simulation: &[u8],
    ) -> anyhow::Result<()> {
        let simulation: XTransVSimulation = serde_json::from_slice(simulation)?;
        self.set_simulation(ptp, slot, &simulation)
    }
}
