pub mod manager;

pub use manager::{CameraRenderManager, INCOMING_OBJECT_HANDLE, OUTGOING_OBJECT_HANDLE};

use erased_serde::serialize_trait_object;

use crate::{features::simulation::Simulation, ptp::fuji};

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

    setter!(set_file_type, fuji::FileType);
    setter!(set_exposure_offset, fuji::ExposureOffset);
    setter!(set_teleconverter, fuji::Teleconverter);

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

serialize_trait_object!(ConversionProfile);
