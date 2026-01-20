use std::{
    io::{self, Cursor, Write},
    thread::sleep,
    time::Duration,
};

use log::{debug, warn};
use ptp_cursor::{ExactString, PtpDeserialize, PtpSerialize};
use serde::{Deserialize, Serialize};

use crate::{
    devices::x_trans_v::x_t5::{FujifilmXT5, simulation::XT5Simulation},
    features::{
        render::{CameraRenderManager, ConversionProfile},
        simulation::Simulation,
    },
    ptp::{CommandCode, DevicePropCode, ObjectFormat, ObjectInfo, Ptp, fuji},
};

impl XT5ConversionProfile {
    const EXPECTED_N_PROPS: i16 = 29;
    const EXPECTED_PROFILE_CODE: u32 = 0xff17_9502;
    const PADDING: usize = 0x1EE;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct XT5ConversionProfile {
    pub unknown_0: i32,
    pub file_type: fuji::FileType,
    pub size: fuji::ImageSize,
    pub quality: fuji::ImageQuality,
    pub exposure_offset: fuji::ExposureOffset,
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
    pub white_balance_as_shot: fuji::WhiteBalanceAsShot,
    pub white_balance: fuji::WhiteBalance,
    pub white_balance_shift_red: fuji::WhiteBalanceShift,
    pub white_balance_shift_blue: fuji::WhiteBalanceShift,
    pub white_balance_temperature: Option<fuji::WhiteBalanceTemperature>,
    pub lens_modulation_optimizer: fuji::LensModulationOptimizer,
    pub color_space: fuji::ColorSpace,
    pub teleconverter: fuji::Teleconverter,
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

impl PtpDeserialize for XT5ConversionProfile {
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
        let simulation = map_conv_err!(simulation.try_into());
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
            fuji::WhiteBalance::Temperature => {
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

impl PtpSerialize for XT5ConversionProfile {
    fn try_into_ptp(&self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.try_write_ptp(&mut buf)?;
        Ok(buf)
    }

    fn try_write_ptp(&self, buf: &mut Vec<u8>) -> io::Result<()> {
        Self::EXPECTED_N_PROPS.try_write_ptp(buf)?;

        let profile_code = format!("{:x}", Self::EXPECTED_PROFILE_CODE);
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
        if self.white_balance_as_shot == fuji::WhiteBalanceAsShot::False
            && self.white_balance == fuji::WhiteBalance::AsShot
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

impl ConversionProfile for XT5ConversionProfile {
    fn set_from_simulation(&mut self, simulation: &dyn Simulation) -> anyhow::Result<()> {
        let simulation = simulation.as_any().downcast_ref::<XT5Simulation>().unwrap();

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

    fn set_file_type(&mut self, value: &fuji::FileType) -> anyhow::Result<()> {
        self.file_type = *value;
        Ok(())
    }

    fn set_exposure_offset(&mut self, value: &fuji::ExposureOffset) -> anyhow::Result<()> {
        self.exposure_offset = *value;
        Ok(())
    }

    fn set_teleconverter(&mut self, value: &fuji::Teleconverter) -> anyhow::Result<()> {
        self.teleconverter = *value;
        Ok(())
    }

    fn set_size(&mut self, value: &fuji::ImageSize) -> anyhow::Result<()> {
        self.size = *value;
        Ok(())
    }

    fn set_quality(&mut self, value: &fuji::ImageQuality) -> anyhow::Result<()> {
        if matches!(
            value,
            fuji::ImageQuality::FineRaw | fuji::ImageQuality::Fine
        ) {
            self.quality = fuji::ImageQuality::Fine;
        }

        if matches!(
            value,
            fuji::ImageQuality::NormalRaw | fuji::ImageQuality::Normal
        ) {
            self.quality = fuji::ImageQuality::Normal;
        }

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
        if *value == fuji::WhiteBalance::AsShot {
            self.white_balance_as_shot = fuji::WhiteBalanceAsShot::True;
        } else {
            self.white_balance_as_shot = fuji::WhiteBalanceAsShot::False;
        }

        self.white_balance = *value;

        Ok(())
    }

    fn set_white_balance_shift_red(
        &mut self,
        value: &fuji::WhiteBalanceShift,
    ) -> anyhow::Result<()> {
        self.white_balance_as_shot = fuji::WhiteBalanceAsShot::False;
        self.white_balance_shift_red = *value;
        Ok(())
    }

    fn set_white_balance_shift_blue(
        &mut self,
        value: &fuji::WhiteBalanceShift,
    ) -> anyhow::Result<()> {
        self.white_balance_as_shot = fuji::WhiteBalanceAsShot::False;
        self.white_balance_shift_blue = *value;
        Ok(())
    }

    fn set_white_balance_temperature(
        &mut self,
        value: &fuji::WhiteBalanceTemperature,
    ) -> anyhow::Result<()> {
        self.white_balance_as_shot = fuji::WhiteBalanceAsShot::False;
        self.white_balance_temperature = Some(*value);
        Ok(())
    }

    fn set_dynamic_range(&mut self, value: &fuji::DynamicRange) -> anyhow::Result<()> {
        if *value == fuji::DynamicRange::HDR800Plus {
            self.dynamic_range = fuji::DynamicRange::HDR800;
            self.dynamic_range_priority = fuji::DynamicRangePriority::Plus;
        } else {
            self.dynamic_range = *value;
        }

        Ok(())
    }

    fn set_dynamic_range_priority(
        &mut self,
        value: &fuji::DynamicRangePriority,
    ) -> anyhow::Result<()> {
        self.dynamic_range_priority = *value;
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

impl CameraRenderManager for FujifilmXT5 {
    fn render(
        &self,
        ptp: &mut Ptp,
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

        debug!("Sending image to camera");
        ptp.send(
            CommandCode::FujiSendObjectInfo,
            &[0x0, 0x0, 0x0],
            Some(&object_info.try_into_ptp()?),
        )?;
        ptp.send(CommandCode::FujiSendObject, &[], Some(image))?;
        debug!("Sent image to camera");

        debug!("Fetching image conversion profile");
        let mut profile: XT5ConversionProfile =
            ptp.get_prop(DevicePropCode::FujiRawConversionProfile)?;
        debug!("Fetched image conversion profile");

        debug!("Updating image conversion profile");
        conversion_profile_modifier(&mut profile)?;
        ptp.set_prop(DevicePropCode::FujiRawConversionProfile, &profile)?;
        debug!("Updated image conversion profile");

        debug!("Starting image render");
        ptp.set_prop(DevicePropCode::FujiRawConversionRun, &u16::from(!draft))?;

        let handle;
        loop {
            debug!("Fetching rendered object handles");
            let response = ptp.send(CommandCode::GetObjectHandles, &[u32::MAX, 0, 0], None)?;
            let response = <Vec<u32>>::try_from_ptp(&response)?;
            if !response.is_empty() {
                handle = response[0];
                break;
            }

            sleep(Duration::from_millis(100));
        }

        debug!("Fetching rendered image");
        let buf = ptp.send(CommandCode::GetObject, &[handle], None)?;
        debug!("Fetched rendered image");

        debug!("Cleaning up rendered image on camera");
        let _ = ptp.send(CommandCode::DeleteObject, &[handle], None)?;
        debug!("Cleaned up rendered image on camera");

        Ok(buf)
    }
}
