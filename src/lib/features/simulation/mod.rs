pub mod manager;
pub mod parser;

pub use manager::CameraSimulationManager;
pub use parser::CameraSimulationParser;

use std::{any::Any, fmt};

use erased_serde::serialize_trait_object;
use serde::Serialize;

use crate::ptp::fuji;

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

    getter!(get_name, fuji::CustomSettingName);

    setter!(set_name, fuji::CustomSettingName);

    setter!(set_size, fuji::ImageSize);
    setter!(set_quality, fuji::ImageQuality);
    setter!(set_simulation, fuji::FilmSimulation);
    setter!(
        set_monochromatic_color_temperature,
        fuji::MonochromaticColorShift
    );
    setter!(set_monochromatic_color_tint, fuji::MonochromaticColorShift);
    setter!(set_highlight, fuji::HighlightTone);
    setter!(set_shadow, fuji::ShadowTone);
    setter!(set_color, fuji::Color);
    setter!(set_sharpness, fuji::Sharpness);
    setter!(set_clarity, fuji::Clarity);
    setter!(set_noise_reduction, fuji::NoiseReduction);
    setter!(set_grain, fuji::GrainEffect);
    setter!(set_color_chrome_effect, fuji::ColorChromeEffect);
    setter!(set_color_chrome_fx_blue, fuji::ColorChromeFXBlue);
    setter!(set_smooth_skin_effect, fuji::SmoothSkinEffect);
    setter!(set_white_balance, fuji::WhiteBalance);
    setter!(set_white_balance_shift_red, fuji::WhiteBalanceShift);
    setter!(set_white_balance_shift_blue, fuji::WhiteBalanceShift);
    setter!(set_white_balance_temperature, fuji::WhiteBalanceTemperature);
    setter!(set_dynamic_range, fuji::DynamicRange);
    setter!(set_dynamic_range_priority, fuji::DynamicRangePriority);
    setter!(set_lens_modulation_optimizer, fuji::LensModulationOptimizer);
    setter!(set_color_space, fuji::ColorSpace);
}

serialize_trait_object!(Simulation);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationListItem {
    pub slot: fuji::CustomSetting,
    pub name: fuji::CustomSettingName,
}

impl fmt::Display for SimulationListItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.slot, self.name)
    }
}
