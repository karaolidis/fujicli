use std::{any::Any, fmt};

use log::error;
use serde::{Deserialize, Serialize};

use crate::{
    devices::x_trans_v::x_t5::FujifilmXT5,
    features::simulation::{CameraSimulationManager, CameraSimulationParser, Simulation},
    ptp::{DevicePropCode, Ptp, fuji},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XT5Simulation {
    pub name: fuji::CustomSettingName,
    pub size: fuji::ImageSize,
    pub quality: fuji::ImageQuality,
    #[allow(clippy::struct_field_names)]
    pub simulation: fuji::FilmSimulation,
    pub monochromatic_color_temperature: fuji::MonochromaticColorShift,
    pub monochromatic_color_tint: fuji::MonochromaticColorShift,
    pub dynamic_range_priority: fuji::DynamicRangePriority,
    pub dynamic_range: fuji::DynamicRange,
    pub highlight: fuji::HighlightTone,
    pub shadow: fuji::ShadowTone,
    pub color: fuji::Color,
    pub sharpness: fuji::Sharpness,
    pub clarity: fuji::Clarity,
    pub noise_reduction: fuji::NoiseReduction,
    pub grain: fuji::GrainEffect,
    pub color_chrome_effect: fuji::ColorChromeEffect,
    pub color_chrome_fx_blue: fuji::ColorChromeFXBlue,
    pub smooth_skin_effect: fuji::SmoothSkinEffect,
    pub white_balance: fuji::WhiteBalance,
    pub white_balance_shift_red: fuji::WhiteBalanceShift,
    pub white_balance_shift_blue: fuji::WhiteBalanceShift,
    pub white_balance_temperature: fuji::WhiteBalanceTemperature,
    pub lens_modulation_optimizer: fuji::LensModulationOptimizer,
    pub color_space: fuji::ColorSpace,
}

impl fmt::Display for XT5Simulation {
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

impl Simulation for XT5Simulation {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn get_name(&self) -> anyhow::Result<fuji::CustomSettingName> {
        Ok(self.name.clone())
    }

    fn set_name(&mut self, value: &fuji::CustomSettingName) -> anyhow::Result<()> {
        self.name = value.clone();
        Ok(())
    }

    fn set_size(&mut self, value: &fuji::ImageSize) -> anyhow::Result<()> {
        self.size = *value;
        Ok(())
    }

    fn set_quality(&mut self, value: &fuji::ImageQuality) -> anyhow::Result<()> {
        self.quality = *value;
        Ok(())
    }

    fn set_simulation(&mut self, value: &fuji::FilmSimulation) -> anyhow::Result<()> {
        self.simulation = *value;
        Ok(())
    }

    fn set_monochromatic_color_temperature(
        &mut self,
        value: &fuji::MonochromaticColorShift,
    ) -> anyhow::Result<()> {
        self.monochromatic_color_temperature = *value;
        Ok(())
    }

    fn set_monochromatic_color_tint(
        &mut self,
        value: &fuji::MonochromaticColorShift,
    ) -> anyhow::Result<()> {
        self.monochromatic_color_tint = *value;
        Ok(())
    }

    fn set_dynamic_range_priority(
        &mut self,
        value: &fuji::DynamicRangePriority,
    ) -> anyhow::Result<()> {
        self.dynamic_range_priority = *value;
        Ok(())
    }

    fn set_dynamic_range(&mut self, value: &fuji::DynamicRange) -> anyhow::Result<()> {
        self.dynamic_range = *value;
        Ok(())
    }

    fn set_highlight(&mut self, value: &fuji::HighlightTone) -> anyhow::Result<()> {
        self.highlight = *value;
        Ok(())
    }

    fn set_shadow(&mut self, value: &fuji::ShadowTone) -> anyhow::Result<()> {
        self.shadow = *value;
        Ok(())
    }

    fn set_color(&mut self, value: &fuji::Color) -> anyhow::Result<()> {
        self.color = *value;
        Ok(())
    }

    fn set_sharpness(&mut self, value: &fuji::Sharpness) -> anyhow::Result<()> {
        self.sharpness = *value;
        Ok(())
    }

    fn set_clarity(&mut self, value: &fuji::Clarity) -> anyhow::Result<()> {
        self.clarity = *value;
        Ok(())
    }

    fn set_noise_reduction(&mut self, value: &fuji::NoiseReduction) -> anyhow::Result<()> {
        self.noise_reduction = *value;
        Ok(())
    }

    fn set_grain(&mut self, value: &fuji::GrainEffect) -> anyhow::Result<()> {
        self.grain = *value;
        Ok(())
    }

    fn set_color_chrome_effect(&mut self, value: &fuji::ColorChromeEffect) -> anyhow::Result<()> {
        self.color_chrome_effect = *value;
        Ok(())
    }

    fn set_color_chrome_fx_blue(&mut self, value: &fuji::ColorChromeFXBlue) -> anyhow::Result<()> {
        self.color_chrome_fx_blue = *value;
        Ok(())
    }

    fn set_smooth_skin_effect(&mut self, value: &fuji::SmoothSkinEffect) -> anyhow::Result<()> {
        self.smooth_skin_effect = *value;
        Ok(())
    }

    fn set_white_balance(&mut self, value: &fuji::WhiteBalance) -> anyhow::Result<()> {
        self.white_balance = *value;
        Ok(())
    }

    fn set_white_balance_shift_red(
        &mut self,
        value: &fuji::WhiteBalanceShift,
    ) -> anyhow::Result<()> {
        self.white_balance_shift_red = *value;
        Ok(())
    }

    fn set_white_balance_shift_blue(
        &mut self,
        value: &fuji::WhiteBalanceShift,
    ) -> anyhow::Result<()> {
        self.white_balance_shift_blue = *value;
        Ok(())
    }

    fn set_white_balance_temperature(
        &mut self,
        value: &fuji::WhiteBalanceTemperature,
    ) -> anyhow::Result<()> {
        self.white_balance_temperature = *value;
        Ok(())
    }

    fn set_lens_modulation_optimizer(
        &mut self,
        value: &fuji::LensModulationOptimizer,
    ) -> anyhow::Result<()> {
        self.lens_modulation_optimizer = *value;
        Ok(())
    }

    fn set_color_space(&mut self, value: &fuji::ColorSpace) -> anyhow::Result<()> {
        self.color_space = *value;
        Ok(())
    }
}

impl CameraSimulationManager for FujifilmXT5 {
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
        let smooth_skin_effect = ptp.get_prop(DevicePropCode::FujiCustomSettingSmoothSkinEffect)?;
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

        let sim = XT5Simulation {
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
        slot: fuji::CustomSetting,
        simulation_modifier: &mut dyn FnMut(&mut dyn Simulation) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let original_simulation = self.get_simulation(ptp, slot)?;
        let original_simulation = original_simulation
            .as_any()
            .downcast_ref::<XT5Simulation>()
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
        slot: fuji::CustomSetting,
        simulation: &dyn Simulation,
    ) -> anyhow::Result<()> {
        let simulation = simulation.as_any().downcast_ref::<XT5Simulation>().unwrap();

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
}

impl CameraSimulationParser for FujifilmXT5 {
    fn deserialize_simulation(&self, simulation: &[u8]) -> anyhow::Result<Box<dyn Simulation>> {
        let simulation: XT5Simulation = serde_json::from_slice(simulation)?;
        Ok(Box::new(simulation))
    }

    fn serialize_simulation(&self, simulation: &dyn Simulation) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(&simulation)?)
    }
}
