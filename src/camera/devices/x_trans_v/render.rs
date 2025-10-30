use std::io::{self, Cursor, Write};

use ptp_cursor::{PtpDeserialize, PtpSerialize};
use ptp_macro::{PtpDeserialize, PtpSerialize};

use crate::camera::ptp::hex::{
    FujiClarity, FujiColor, FujiColorChromeEffect, FujiColorChromeFXBlue, FujiColorSpace,
    FujiDynamicRange, FujiDynamicRangePriority, FujiExposureOffset, FujiFileType,
    FujiFilmSimulation, FujiGrainEffect, FujiHighISONR, FujiHighlightTone, FujiImageQuality,
    FujiImageSize, FujiLensModulationOptimizer, FujiMonochromaticColorShift, FujiShadowTone,
    FujiSharpness, FujiSmoothSkinEffect, FujiTeleconverter, FujiWhiteBalance,
    FujiWhiteBalanceAsShot, FujiWhiteBalanceShift, FujiWhiteBalanceTemperature,
};

// NOTE: Naively assuming that all sensors share the same conversion profile.
// This is almost certainly false.
#[derive(Debug, PtpSerialize, PtpDeserialize)]
struct XTransVConversionProfileRaw {
    // TODO: What is this, and why is it always 0x2?
    unknown_0: i32,
    file_type: u32,
    size: u32,
    quality: u32,
    exposure_offset: i32,
    dynamic_range: u32,
    dynamic_range_priority: u32,
    simulation: u32,
    grain: u32,
    color_chrome_effect: u32,
    white_balance_as_shot: u32,
    white_balance: u32,
    white_balance_shift_red: i32,
    white_balance_shift_blue: i32,
    white_balance_temperature: i32,
    highlight: i32,
    shadow: i32,
    color: i32,
    sharpness: i32,
    noise_reduction: u32,
    lens_modulation_optimizer: u32,
    color_space: u32,
    monochromatic_color_temperature: i32,
    smooth_skin_effect: u32,
    color_chrome_fx_blue: u32,
    monochromatic_color_tint: i32,
    clarity: i32,
    teleconverter: u32,
}

#[derive(Debug)]
pub enum XTransVConversionProfileWhiteBalance {
    AsShot {
        // Store the original values for serialization
        white_balance: u32,
        white_balance_shift_red: i32,
        white_balance_shift_blue: i32,
        white_balance_temperature: i32,
    },
    Custom {
        white_balance: FujiWhiteBalance,
        white_balance_shift_red: FujiWhiteBalanceShift,
        white_balance_shift_blue: FujiWhiteBalanceShift,
        white_balance_temperature: FujiWhiteBalanceTemperature,
    },
}

#[derive(Debug)]
pub struct XTransVConversionProfile {
    pub unknown_0: i32,
    pub file_type: FujiFileType,
    pub size: FujiImageSize,
    pub quality: FujiImageQuality,
    pub exposure_offset: FujiExposureOffset,
    pub dynamic_range: FujiDynamicRange,
    pub dynamic_range_priority: FujiDynamicRangePriority,
    pub simulation: FujiFilmSimulation,
    pub grain: FujiGrainEffect,
    pub color_chrome_effect: FujiColorChromeEffect,
    pub white_balance: XTransVConversionProfileWhiteBalance,
    pub highlight: FujiHighlightTone,
    pub shadow: FujiShadowTone,
    pub color: FujiColor,
    pub sharpness: FujiSharpness,
    pub noise_reduction: FujiHighISONR,
    pub lens_modulation_optimizer: FujiLensModulationOptimizer,
    pub color_space: FujiColorSpace,
    pub monochromatic_color_temperature: FujiMonochromaticColorShift,
    pub smooth_skin_effect: FujiSmoothSkinEffect,
    pub color_chrome_fx_blue: FujiColorChromeFXBlue,
    pub monochromatic_color_tint: FujiMonochromaticColorShift,
    pub clarity: FujiClarity,
    pub teleconverter: FujiTeleconverter,
}

impl XTransVConversionProfile {
    const EXPECTED_N_PROPS: i16 = 29;
    // This is FF179502 in the XML. God knows wtf is going on here.
    const EXPECTED_PROFILE_CODE: &str = "FF17950";
    const PADDING: usize = 0x1EE;
}

macro_rules! map_conv_err {
    ($expr:expr) => {
        $expr.map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid value: {}", e),
            )
        })?
    };
}

impl PtpDeserialize for XTransVConversionProfile {
    fn try_from_ptp(buf: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(buf);
        let value = Self::try_read_ptp(&mut cur)?;
        Ok(value)
    }

    fn try_read_ptp<R: ptp_cursor::Read>(cur: &mut R) -> io::Result<Self> {
        let n_props = <i16>::try_read_ptp(cur)?;
        if n_props != Self::EXPECTED_N_PROPS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected {} props, got {n_props}", Self::EXPECTED_N_PROPS),
            ));
        }

        let profile_code = String::try_read_ptp(cur)?;
        if profile_code != Self::EXPECTED_PROFILE_CODE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Expected profile code '{}', got '{profile_code}'",
                    Self::EXPECTED_PROFILE_CODE
                ),
            ));
        }

        let mut padding = [0u8; Self::PADDING];
        cur.read_exact(&mut padding)?;

        let contents = XTransVConversionProfileRaw::try_read_ptp(cur)?;

        let unknown_0 = contents.unknown_0;

        let exposure_offset = map_conv_err!(contents.exposure_offset.try_into());
        let file_type = map_conv_err!(contents.file_type.try_into());
        let size = map_conv_err!(contents.size.try_into());
        let quality = map_conv_err!(contents.quality.try_into());
        let simulation = map_conv_err!(contents.simulation.try_into());
        let monochromatic_color_temperature =
            map_conv_err!(contents.monochromatic_color_temperature.try_into());
        let monochromatic_color_tint = map_conv_err!(contents.monochromatic_color_tint.try_into());
        let highlight = map_conv_err!(contents.highlight.try_into());
        let shadow = map_conv_err!(contents.shadow.try_into());
        let color = map_conv_err!(contents.color.try_into());
        let sharpness = map_conv_err!(contents.sharpness.try_into());
        let clarity = map_conv_err!(contents.clarity.try_into());
        let noise_reduction = map_conv_err!(contents.noise_reduction.try_into());
        let grain = map_conv_err!(contents.grain.try_into());
        let color_chrome_effect = map_conv_err!(contents.color_chrome_effect.try_into());
        let color_chrome_fx_blue = map_conv_err!(contents.color_chrome_fx_blue.try_into());
        let smooth_skin_effect = map_conv_err!(contents.smooth_skin_effect.try_into());
        let white_balance_as_shot = map_conv_err!(contents.white_balance_as_shot.try_into());
        let white_balance = match white_balance_as_shot {
            FujiWhiteBalanceAsShot::False => XTransVConversionProfileWhiteBalance::Custom {
                white_balance: map_conv_err!(contents.white_balance.try_into()),
                white_balance_shift_red: map_conv_err!(contents.white_balance_shift_red.try_into()),
                white_balance_shift_blue: map_conv_err!(
                    contents.white_balance_shift_blue.try_into()
                ),
                white_balance_temperature: map_conv_err!(
                    contents.white_balance_temperature.try_into()
                ),
            },
            FujiWhiteBalanceAsShot::True => XTransVConversionProfileWhiteBalance::AsShot {
                white_balance: contents.white_balance,
                white_balance_shift_red: contents.white_balance_shift_red,
                white_balance_shift_blue: contents.white_balance_shift_blue,
                white_balance_temperature: contents.white_balance_temperature,
            },
        };

        let dynamic_range = map_conv_err!(contents.dynamic_range.try_into());
        let dynamic_range_priority = map_conv_err!(contents.dynamic_range_priority.try_into());
        let lens_modulation_optimizer =
            map_conv_err!(contents.lens_modulation_optimizer.try_into());
        let color_space = map_conv_err!(contents.color_space.try_into());
        let teleconverter = map_conv_err!(contents.teleconverter.try_into());

        Ok(Self {
            unknown_0,
            file_type,
            size,
            quality,
            exposure_offset,
            dynamic_range,
            dynamic_range_priority,
            simulation,
            grain,
            color_chrome_effect,
            white_balance,
            highlight,
            shadow,
            color,
            sharpness,
            noise_reduction,
            lens_modulation_optimizer,
            color_space,
            monochromatic_color_temperature,
            smooth_skin_effect,
            color_chrome_fx_blue,
            monochromatic_color_tint,
            clarity,
            teleconverter,
        })
    }
}

impl PtpSerialize for XTransVConversionProfile {
    fn try_into_ptp(&self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.try_write_ptp(&mut buf)?;
        Ok(buf)
    }

    fn try_write_ptp(&self, buf: &mut Vec<u8>) -> io::Result<()> {
        Self::EXPECTED_N_PROPS.try_write_ptp(buf)?;
        Self::EXPECTED_PROFILE_CODE.try_write_ptp(buf)?;

        let padding = [0u8; Self::PADDING];
        buf.write_all(&padding)?;

        let unknown_0 = self.unknown_0;
        let file_type = self.file_type.into();
        let size = self.size.into();
        let quality = self.quality.into();
        let exposure_offset = self.exposure_offset.into();
        let dynamic_range = self.dynamic_range.into();
        let dynamic_range_priority = self.dynamic_range_priority.into();
        let simulation = self.simulation.into();
        let grain = self.grain.into();
        let color_chrome_effect = self.color_chrome_effect.into();

        let (
            white_balance_as_shot,
            white_balance,
            white_balance_shift_red,
            white_balance_shift_blue,
            white_balance_temperature,
        ) = match &self.white_balance {
            XTransVConversionProfileWhiteBalance::Custom {
                white_balance,
                white_balance_shift_red,
                white_balance_shift_blue,
                white_balance_temperature,
            } => (
                FujiWhiteBalanceAsShot::False.into(),
                (*white_balance).into(),
                (*white_balance_shift_red).into(),
                (*white_balance_shift_blue).into(),
                (*white_balance_temperature).into(),
            ),
            XTransVConversionProfileWhiteBalance::AsShot {
                white_balance,
                white_balance_shift_red,
                white_balance_shift_blue,
                white_balance_temperature,
            } => (
                FujiWhiteBalanceAsShot::True.into(),
                *white_balance,
                *white_balance_shift_red,
                *white_balance_shift_blue,
                *white_balance_temperature,
            ),
        };

        let highlight = self.highlight.into();
        let shadow = self.shadow.into();
        let color = self.color.into();
        let sharpness = self.sharpness.into();
        let noise_reduction = self.noise_reduction.into();
        let lens_modulation_optimizer = self.lens_modulation_optimizer.into();
        let color_space = self.color_space.into();
        let monochromatic_color_temperature = self.monochromatic_color_temperature.into();
        let smooth_skin_effect = self.smooth_skin_effect.into();
        let color_chrome_fx_blue = self.color_chrome_fx_blue.into();
        let monochromatic_color_tint = self.monochromatic_color_tint.into();
        let clarity = self.clarity.into();
        let teleconverter = self.teleconverter.into();

        let raw = XTransVConversionProfileRaw {
            unknown_0,
            file_type,
            size,
            quality,
            exposure_offset,
            dynamic_range,
            dynamic_range_priority,
            simulation,
            grain,
            color_chrome_effect,
            white_balance_as_shot,
            white_balance,
            white_balance_shift_red,
            white_balance_shift_blue,
            white_balance_temperature,
            highlight,
            shadow,
            color,
            sharpness,
            noise_reduction,
            lens_modulation_optimizer,
            color_space,
            monochromatic_color_temperature,
            smooth_skin_effect,
            color_chrome_fx_blue,
            monochromatic_color_tint,
            clarity,
            teleconverter,
        };
        raw.try_write_ptp(buf)?;

        Ok(())
    }
}
