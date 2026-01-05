use erased_serde::serialize_trait_object;

use crate::camera::{
    features::simulation::simulation::Simulation,
    ptp::hex::{
        FujiClarity, FujiColor, FujiColorChromeEffect, FujiColorChromeFXBlue, FujiColorSpace,
        FujiDynamicRange, FujiDynamicRangePriority, FujiExposureOffset, FujiFileType,
        FujiFilmSimulation, FujiGrainEffect, FujiHighISONR, FujiHighlightTone, FujiImageQuality,
        FujiImageSize, FujiLensModulationOptimizer, FujiMonochromaticColorShift, FujiShadowTone,
        FujiSharpness, FujiSmoothSkinEffect, FujiTeleconverter, FujiWhiteBalance,
        FujiWhiteBalanceShift, FujiWhiteBalanceTemperature,
    },
};

macro_rules! setter {
    ($name:ident, $type:ty) => {
        fn $name(&mut self, _value: &$type) -> anyhow::Result<()> {
            anyhow::bail!(
                "This conversion profile does not support setting {}",
                stringify!($ty)
            );
        }
    };
}

pub trait ConversionProfile: erased_serde::Serialize {
    fn set_from_simulation(&mut self, simulation: &dyn Simulation) -> anyhow::Result<()>;

    setter!(set_file_type, FujiFileType);
    setter!(set_exposure_offset, FujiExposureOffset);
    setter!(set_teleconverter, FujiTeleconverter);

    setter!(set_size, FujiImageSize);
    setter!(set_quality, FujiImageQuality);
    setter!(set_simulation, FujiFilmSimulation);
    setter!(
        set_monochromatic_color_temperature,
        FujiMonochromaticColorShift
    );
    setter!(set_monochromatic_color_tint, FujiMonochromaticColorShift);
    setter!(set_highlight, FujiHighlightTone);
    setter!(set_shadow, FujiShadowTone);
    setter!(set_color, FujiColor);
    setter!(set_sharpness, FujiSharpness);
    setter!(set_clarity, FujiClarity);
    setter!(set_noise_reduction, FujiHighISONR);
    setter!(set_grain, FujiGrainEffect);
    setter!(set_color_chrome_effect, FujiColorChromeEffect);
    setter!(set_color_chrome_fx_blue, FujiColorChromeFXBlue);
    setter!(set_smooth_skin_effect, FujiSmoothSkinEffect);
    setter!(set_white_balance, FujiWhiteBalance);
    setter!(set_white_balance_shift_red, FujiWhiteBalanceShift);
    setter!(set_white_balance_shift_blue, FujiWhiteBalanceShift);
    setter!(set_white_balance_temperature, FujiWhiteBalanceTemperature);
    setter!(set_dynamic_range, FujiDynamicRange);
    setter!(set_dynamic_range_priority, FujiDynamicRangePriority);
    setter!(set_lens_modulation_optimizer, FujiLensModulationOptimizer);
    setter!(set_color_space, FujiColorSpace);
}

serialize_trait_object!(ConversionProfile);
