use std::{
    io::{self, Cursor, Write},
    thread::sleep,
    time::Duration,
};

use log::{debug, warn};
use ptp_cursor::{ExactString, PtpDeserialize, PtpSerialize};
use serde::{Deserialize, Serialize};

use crate::camera::{
    devices::x_trans_v::{XTransV, simulation::XTransVSimulation},
    features::{
        render::{CameraRenders, conversion::ConversionProfile},
        simulation::simulation::Simulation,
    },
    ptp::{
        hex::{
            CommandCode, DevicePropCode, FujiClarity, FujiColor, FujiColorChromeEffect,
            FujiColorChromeFXBlue, FujiColorSpace, FujiDynamicRange, FujiDynamicRangePriority,
            FujiExposureOffset, FujiFileType, FujiFilmSimulation, FujiGrainEffect, FujiHighISONR,
            FujiHighlightTone, FujiImageQuality, FujiImageSize, FujiLensModulationOptimizer,
            FujiMonochromaticColorShift, FujiShadowTone, FujiSharpness, FujiSmoothSkinEffect,
            FujiTeleconverter, FujiWhiteBalance, FujiWhiteBalanceAsShot, FujiWhiteBalanceShift,
            FujiWhiteBalanceTemperature, ObjectFormat,
        },
        structs::ObjectInfo,
    },
};

// NOTE: Naively assuming that all sensors share the same conversion profile.
// This is almost certainly false.

impl XTransVConversionProfile {
    const EXPECTED_N_PROPS: i16 = 29;
    const EXPECTED_PROFILE_CODE: u32 = 0xff17_9502;
    const PADDING: usize = 0x1EE;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct XTransVConversionProfile {
    pub unknown_0: i32,
    pub file_type: FujiFileType,
    pub size: FujiImageSize,
    pub quality: FujiImageQuality,
    pub exposure_offset: FujiExposureOffset,
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
    pub white_balance_as_shot: FujiWhiteBalanceAsShot,
    pub white_balance: FujiWhiteBalance,
    pub white_balance_shift_red: FujiWhiteBalanceShift,
    pub white_balance_shift_blue: FujiWhiteBalanceShift,
    pub white_balance_temperature: Option<FujiWhiteBalanceTemperature>,
    pub lens_modulation_optimizer: FujiLensModulationOptimizer,
    pub color_space: FujiColorSpace,
    pub teleconverter: FujiTeleconverter,
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

    #[allow(clippy::too_many_lines)]
    fn try_read_ptp<R: ptp_cursor::Read>(cur: &mut R) -> io::Result<Self> {
        let n_props = <i16>::try_read_ptp(cur)?;
        if n_props != Self::EXPECTED_N_PROPS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected {} props, got {n_props}", Self::EXPECTED_N_PROPS),
            ));
        }

        let profile_code = ExactString::try_read_ptp(cur)?;
        let profile_code = map_conv_err!(u32::from_str_radix(&profile_code, 16));

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

        // Read fields in correct order
        let unknown_0 = i32::try_read_ptp(cur)?;
        let file_type = map_conv_err!(u32::try_read_ptp(cur));
        let size = map_conv_err!(u32::try_read_ptp(cur));
        let quality = map_conv_err!(u32::try_read_ptp(cur));
        let exposure_offset = map_conv_err!(i32::try_read_ptp(cur));
        let dynamic_range = map_conv_err!(u32::try_read_ptp(cur));
        let dynamic_range_priority = map_conv_err!(u32::try_read_ptp(cur));
        let simulation = map_conv_err!(u32::try_read_ptp(cur));
        let grain = map_conv_err!(u32::try_read_ptp(cur));
        let color_chrome_effect = map_conv_err!(u32::try_read_ptp(cur));
        let white_balance_as_shot = map_conv_err!(u32::try_read_ptp(cur));
        let white_balance = map_conv_err!(u32::try_read_ptp(cur));
        let white_balance_shift_red = map_conv_err!(i32::try_read_ptp(cur));
        let white_balance_shift_blue = map_conv_err!(i32::try_read_ptp(cur));
        let white_balance_temperature = map_conv_err!(i32::try_read_ptp(cur));
        let highlight = map_conv_err!(i32::try_read_ptp(cur));
        let shadow = map_conv_err!(i32::try_read_ptp(cur));
        let color = map_conv_err!(i32::try_read_ptp(cur));
        let sharpness = map_conv_err!(i32::try_read_ptp(cur));
        let noise_reduction = map_conv_err!(u32::try_read_ptp(cur));
        let lens_modulation_optimizer = map_conv_err!(u32::try_read_ptp(cur));
        let color_space = map_conv_err!(u32::try_read_ptp(cur));
        let monochromatic_color_temperature = map_conv_err!(i32::try_read_ptp(cur));
        let smooth_skin_effect = map_conv_err!(u32::try_read_ptp(cur));
        let color_chrome_fx_blue = map_conv_err!(u32::try_read_ptp(cur));
        let monochromatic_color_tint = map_conv_err!(i32::try_read_ptp(cur));
        let clarity = map_conv_err!(i32::try_read_ptp(cur));
        let teleconverter = map_conv_err!(u32::try_read_ptp(cur));

        // Process into struct
        let file_type = map_conv_err!(file_type.try_into());
        let size = map_conv_err!(size.try_into());
        let quality = map_conv_err!(quality.try_into());
        let exposure_offset = map_conv_err!(exposure_offset.try_into());
        let simulation: FujiFilmSimulation = map_conv_err!(simulation.try_into());
        let monochromatic_color_temperature =
            map_conv_err!(monochromatic_color_temperature.try_into());
        let monochromatic_color_tint = map_conv_err!(monochromatic_color_tint.try_into());
        let dynamic_range_priority = map_conv_err!(dynamic_range_priority.try_into());
        let dynamic_range = map_conv_err!(dynamic_range.try_into());
        let highlight = map_conv_err!(highlight.try_into());
        let shadow = map_conv_err!(shadow.try_into());
        let color = map_conv_err!(color.try_into());
        let sharpness = map_conv_err!(sharpness.try_into());
        let clarity = map_conv_err!(clarity.try_into());
        let noise_reduction = map_conv_err!(noise_reduction.try_into());
        let grain = map_conv_err!(grain.try_into());
        let color_chrome_effect = map_conv_err!(color_chrome_effect.try_into());
        let color_chrome_fx_blue = map_conv_err!(color_chrome_fx_blue.try_into());
        let smooth_skin_effect = map_conv_err!(smooth_skin_effect.try_into());
        let white_balance_as_shot = map_conv_err!(white_balance_as_shot.try_into());
        let white_balance = map_conv_err!(white_balance.try_into());
        let white_balance_shift_red = map_conv_err!(white_balance_shift_red.try_into());
        let white_balance_shift_blue = map_conv_err!(white_balance_shift_blue.try_into());
        let white_balance_temperature = match white_balance {
            FujiWhiteBalance::Temperature => {
                Some(map_conv_err!(white_balance_temperature.try_into()))
            }
            _ => None,
        };
        let lens_modulation_optimizer = map_conv_err!(lens_modulation_optimizer.try_into());
        let color_space = map_conv_err!(color_space.try_into());
        let teleconverter = map_conv_err!(teleconverter.try_into());

        Ok(Self {
            unknown_0,
            file_type,
            size,
            quality,
            exposure_offset,
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
            white_balance_as_shot,
            white_balance,
            white_balance_shift_red,
            white_balance_shift_blue,
            white_balance_temperature,
            lens_modulation_optimizer,
            color_space,
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

        let profile_code = format!("{:X}", Self::EXPECTED_PROFILE_CODE);
        let profile_code = ExactString::from(profile_code);
        profile_code.try_write_ptp(buf)?;

        let padding = [0u8; Self::PADDING];
        buf.write_all(&padding)?;

        self.unknown_0.try_write_ptp(buf)?;
        u32::from(self.file_type).try_write_ptp(buf)?;
        u32::from(self.size).try_write_ptp(buf)?;
        u32::from(self.quality).try_write_ptp(buf)?;
        i32::from(self.exposure_offset).try_write_ptp(buf)?;
        u32::from(self.dynamic_range).try_write_ptp(buf)?;
        u32::from(self.dynamic_range_priority).try_write_ptp(buf)?;
        u32::from(self.simulation).try_write_ptp(buf)?;
        u32::from(self.grain).try_write_ptp(buf)?;
        u32::from(self.color_chrome_effect).try_write_ptp(buf)?;
        if self.white_balance_as_shot == FujiWhiteBalanceAsShot::False
            && self.white_balance == FujiWhiteBalance::AsShot
        {
            warn!(
                "White Balance has been altered but no explicit White Balance mode has been set. Consider setting a White Balance mode."
            );
        }
        u32::from(self.white_balance_as_shot).try_write_ptp(buf)?;
        u32::from(self.white_balance).try_write_ptp(buf)?;
        i32::from(self.white_balance_shift_red).try_write_ptp(buf)?;
        i32::from(self.white_balance_shift_blue).try_write_ptp(buf)?;
        self.white_balance_temperature
            .map_or(0i32, i32::from)
            .try_write_ptp(buf)?;
        i32::from(self.highlight).try_write_ptp(buf)?;
        i32::from(self.shadow).try_write_ptp(buf)?;
        i32::from(self.color).try_write_ptp(buf)?;
        i32::from(self.sharpness).try_write_ptp(buf)?;
        u32::from(self.noise_reduction).try_write_ptp(buf)?;
        u32::from(self.lens_modulation_optimizer).try_write_ptp(buf)?;
        u32::from(self.color_space).try_write_ptp(buf)?;
        i32::from(self.monochromatic_color_temperature).try_write_ptp(buf)?;
        u32::from(self.smooth_skin_effect).try_write_ptp(buf)?;
        u32::from(self.color_chrome_fx_blue).try_write_ptp(buf)?;
        i32::from(self.monochromatic_color_tint).try_write_ptp(buf)?;
        i32::from(self.clarity).try_write_ptp(buf)?;
        u32::from(self.teleconverter).try_write_ptp(buf)?;

        Ok(())
    }
}

impl ConversionProfile for XTransVConversionProfile {
    fn set_from_simulation(&mut self, simulation: &dyn Simulation) -> anyhow::Result<()> {
        let simulation = simulation
            .as_any()
            .downcast_ref::<XTransVSimulation>()
            .unwrap();

        self.set_size(&simulation.size)?;
        self.set_quality(&simulation.quality)?;
        self.set_simulation(&simulation.simulation)?;
        self.set_monochromatic_color_temperature(&simulation.monochromatic_color_temperature)?;
        self.set_monochromatic_color_tint(&simulation.monochromatic_color_tint)?;
        self.set_highlight(&simulation.highlight)?;
        self.set_shadow(&simulation.shadow)?;
        self.set_color(&simulation.color)?;
        self.set_sharpness(&simulation.sharpness)?;
        self.set_clarity(&simulation.clarity)?;
        self.set_white_balance(&simulation.white_balance)?;
        self.set_white_balance_shift_red(&simulation.white_balance_shift_red)?;
        self.set_white_balance_shift_blue(&simulation.white_balance_shift_blue)?;
        self.set_white_balance_temperature(&simulation.white_balance_temperature)?;
        self.set_dynamic_range(&simulation.dynamic_range)?;
        self.set_dynamic_range_priority(&simulation.dynamic_range_priority)?;
        self.set_noise_reduction(&simulation.noise_reduction)?;
        self.set_grain(&simulation.grain)?;
        self.set_color_chrome_effect(&simulation.color_chrome_effect)?;
        self.set_color_chrome_fx_blue(&simulation.color_chrome_fx_blue)?;
        self.set_smooth_skin_effect(&simulation.smooth_skin_effect)?;
        self.set_lens_modulation_optimizer(&simulation.lens_modulation_optimizer)?;
        self.set_color_space(&simulation.color_space)?;

        Ok(())
    }

    fn set_file_type(&mut self, value: &FujiFileType) -> anyhow::Result<()> {
        self.file_type = *value;
        Ok(())
    }

    fn set_exposure_offset(&mut self, value: &FujiExposureOffset) -> anyhow::Result<()> {
        self.exposure_offset = *value;
        Ok(())
    }

    fn set_teleconverter(&mut self, value: &FujiTeleconverter) -> anyhow::Result<()> {
        self.teleconverter = *value;
        Ok(())
    }

    fn set_size(&mut self, value: &FujiImageSize) -> anyhow::Result<()> {
        self.size = *value;
        Ok(())
    }

    fn set_quality(&mut self, value: &FujiImageQuality) -> anyhow::Result<()> {
        if matches!(value, FujiImageQuality::FineRaw | FujiImageQuality::Fine) {
            self.quality = FujiImageQuality::Fine;
        }

        if matches!(
            value,
            FujiImageQuality::NormalRaw | FujiImageQuality::Normal
        ) {
            self.quality = FujiImageQuality::Normal;
        }

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
        if *value == FujiWhiteBalance::AsShot {
            self.white_balance_as_shot = FujiWhiteBalanceAsShot::True;
        } else {
            self.white_balance_as_shot = FujiWhiteBalanceAsShot::False;
        }

        self.white_balance = *value;

        Ok(())
    }

    fn set_white_balance_shift_red(&mut self, value: &FujiWhiteBalanceShift) -> anyhow::Result<()> {
        self.white_balance_as_shot = FujiWhiteBalanceAsShot::False;
        self.white_balance_shift_red = *value;
        Ok(())
    }

    fn set_white_balance_shift_blue(
        &mut self,
        value: &FujiWhiteBalanceShift,
    ) -> anyhow::Result<()> {
        self.white_balance_as_shot = FujiWhiteBalanceAsShot::False;
        self.white_balance_shift_blue = *value;
        Ok(())
    }

    fn set_white_balance_temperature(
        &mut self,
        value: &FujiWhiteBalanceTemperature,
    ) -> anyhow::Result<()> {
        self.white_balance_as_shot = FujiWhiteBalanceAsShot::False;
        self.white_balance_temperature = Some(*value);
        Ok(())
    }

    fn set_dynamic_range(&mut self, value: &FujiDynamicRange) -> anyhow::Result<()> {
        if *value == FujiDynamicRange::HDR800Plus {
            self.dynamic_range = FujiDynamicRange::HDR800;
            self.dynamic_range_priority = FujiDynamicRangePriority::Plus;
        } else {
            self.dynamic_range = *value;
        }

        Ok(())
    }

    fn set_dynamic_range_priority(
        &mut self,
        value: &FujiDynamicRangePriority,
    ) -> anyhow::Result<()> {
        self.dynamic_range_priority = *value;
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

impl<T> CameraRenders for T
where
    T: XTransV,
{
    fn render(
        &self,
        ptp: &mut crate::camera::ptp::Ptp,
        image: &[u8],
        conversion_profile_modifier: &mut dyn FnMut(
            &mut dyn ConversionProfile,
        ) -> anyhow::Result<()>,
        draft: bool,
    ) -> anyhow::Result<Vec<u8>> {
        let object_info = ObjectInfo {
            object_format: ObjectFormat::FujiRAF,
            compressed_size: u32::try_from(image.len())?,
            filename: String::from("FUP_FILE.dat"),
            ..Default::default()
        };

        ptp.send(
            CommandCode::FujiSendObjectInfo,
            &[0x0, 0x0, 0x0],
            Some(&object_info.try_into_ptp()?),
        )?;

        ptp.send(CommandCode::FujiSendObject, &[], Some(image))?;

        let mut profile: XTransVConversionProfile =
            ptp.get_prop(DevicePropCode::FujiRawConversionProfile)?;
        debug!("Fetched image conversion profile: {profile:?}");

        conversion_profile_modifier(&mut profile)?;
        debug!("Updated image conversion profile: {profile:?}");

        ptp.set_prop(DevicePropCode::FujiRawConversionProfile, &profile)?;
        ptp.set_prop(DevicePropCode::FujiRawConversionRun, &u16::from(!draft))?;

        let handle;
        loop {
            let raw = ptp.send(CommandCode::GetObjectHandles, &[u32::MAX, 0, 0], None)?;

            let response = <Vec<u32>>::try_from_ptp(&raw)?;
            if !response.is_empty() {
                handle = response[0];
                break;
            }

            sleep(Duration::from_millis(100));
        }

        let buf = ptp.send(CommandCode::GetObject, &[handle], None)?;
        ptp.send(CommandCode::DeleteObject, &[handle], None)?;

        Ok(buf)
    }
}
