use std::{any::Any, fmt};

use erased_serde::serialize_trait_object;
use serde::Serialize;

use crate::ptp::hex::{
    FujiClarity, FujiColor, FujiColorChromeEffect, FujiColorChromeFXBlue, FujiColorSpace,
    FujiCustomSetting, FujiCustomSettingName, FujiDynamicRange, FujiDynamicRangePriority,
    FujiFilmSimulation, FujiGrainEffect, FujiHighISONR, FujiHighlightTone, FujiImageQuality,
    FujiImageSize, FujiLensModulationOptimizer, FujiMonochromaticColorShift, FujiShadowTone,
    FujiSharpness, FujiSmoothSkinEffect, FujiWhiteBalance, FujiWhiteBalanceShift,
    FujiWhiteBalanceTemperature,
};

macro_rules! getter {
    ($name:ident, $type:ty) => {
        fn $name(&self) -> anyhow::Result<$type> {
            anyhow::bail!(
                "This simulation profile does not support getting {}",
                stringify!($ty)
            );
        }
    };
}

macro_rules! setter {
    ($name:ident, $type:ty) => {
        fn $name(&mut self, _value: &$type) -> anyhow::Result<()> {
            anyhow::bail!(
                "This simulation profile does not support setting {}",
                stringify!($ty)
            );
        }
    };
}

pub trait Simulation: fmt::Display + erased_serde::Serialize {
    fn as_any(&self) -> &dyn Any;

    getter!(get_name, FujiCustomSettingName);

    setter!(set_name, FujiCustomSettingName);

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

serialize_trait_object!(Simulation);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationListItem {
    pub slot: FujiCustomSetting,
    pub name: FujiCustomSettingName,
}

impl fmt::Display for SimulationListItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.slot, self.name)
    }
}
