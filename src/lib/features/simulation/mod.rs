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

    fn serialize(&self) -> anyhow::Result<Vec<u8>>;
    fn deserialize(data: &[u8]) -> anyhow::Result<Box<dyn Simulation>>
    where
        Self: Sized;

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

macro_rules! impl_simulation {
    (
        $sim:ty,
        [ $( $cap:ident ),* $(,)? ]
    ) => {
        impl crate::features::simulation::Simulation for $sim {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn serialize(&self) -> anyhow::Result<Vec<u8>> {
                Ok(serde_json::to_vec(self)?)
            }

            fn deserialize(
                data: &[u8]
            ) -> anyhow::Result<Box<dyn crate::features::simulation::Simulation>>
            where
                Self: Sized,
            {
                let s: $sim = serde_json::from_slice(data)?;
                Ok(Box::new(s))
            }

            fn get_name(&self) -> anyhow::Result<crate::ptp::fuji::CustomSettingName> {
                Ok(self.name.clone())
            }

            fn set_name(&mut self, value: &crate::ptp::fuji::CustomSettingName) -> anyhow::Result<()> {
                self.name = value.clone();
                Ok(())
            }

            $(
                crate::features::simulation::impl_simulation!(@cap self, $cap);
            )*
        }

        impl std::fmt::Display for $sim {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
                writeln!(f, "Name: {}", self.name)?;
                $(
                    crate::features::simulation::impl_simulation!(@write self, f, $cap);
                )*
                Ok(())
            }
        }
    };

    (@cap $self:ident, ImageSize) => {
        fn set_size(&mut $self, value: &crate::ptp::fuji::ImageSize) -> anyhow::Result<()> {
            $self.size = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, ImageSize) => {
        writeln!($f, "Size: {}", $self.size)?;
    };

    (@cap $self:ident, ImageQuality) => {
        fn set_quality(&mut $self, value: &crate::ptp::fuji::ImageQuality) -> anyhow::Result<()> {
            $self.quality = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, ImageQuality) => {
        writeln!($f, "Quality: {}", $self.quality)?;
    };

    (@cap $self:ident, FilmSimulation) => {
        fn set_simulation(&mut $self, value: &crate::ptp::fuji::FilmSimulation) -> anyhow::Result<()> {
            $self.simulation = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, FilmSimulation) => {
        writeln!($f, "Simulation: {}", $self.simulation)?;
    };

    (@cap $self:ident, MonochromaticColorShift) => {
        fn set_monochromatic_color_temperature(
            &mut $self,
            value: &crate::ptp::fuji::MonochromaticColorShift
        ) -> anyhow::Result<()> {
            $self.monochromatic_color_temperature = *value;
            Ok(())
        }

        fn set_monochromatic_color_tint(
            &mut $self,
            value: &crate::ptp::fuji::MonochromaticColorShift
        ) -> anyhow::Result<()> {
            $self.monochromatic_color_tint = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, MonochromaticColorShift) => {
        writeln!($f, "Monochromatic Color Temperature: {}", $self.monochromatic_color_temperature)?;
        writeln!($f, "Monochromatic Color Tint: {}", $self.monochromatic_color_tint)?;
    };

    (@cap $self:ident, Highlight) => {
        fn set_highlight(&mut $self, value: &crate::ptp::fuji::HighlightTone) -> anyhow::Result<()> {
            $self.highlight = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, Highlight) => {
        writeln!($f, "Highlight: {}", $self.highlight)?;
    };

    (@cap $self:ident, Shadow) => {
        fn set_shadow(&mut $self, value: &crate::ptp::fuji::ShadowTone) -> anyhow::Result<()> {
            $self.shadow = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, Shadow) => {
        writeln!($f, "Shadow: {}", $self.shadow)?;
    };

    (@cap $self:ident, Color) => {
        fn set_color(&mut $self, value: &crate::ptp::fuji::Color) -> anyhow::Result<()> {
            $self.color = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, Color) => {
        writeln!($f, "Color: {}", $self.color)?;
    };

    (@cap $self:ident, Sharpness) => {
        fn set_sharpness(&mut $self, value: &crate::ptp::fuji::Sharpness) -> anyhow::Result<()> {
            $self.sharpness = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, Sharpness) => {
        writeln!($f, "Sharpness: {}", $self.sharpness)?;
    };

    (@cap $self:ident, Clarity) => {
        fn set_clarity(&mut $self, value: &crate::ptp::fuji::Clarity) -> anyhow::Result<()> {
            $self.clarity = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, Clarity) => {
        writeln!($f, "Clarity: {}", $self.clarity)?;
    };

    (@cap $self:ident, NoiseReduction) => {
        fn set_noise_reduction(&mut $self, value: &crate::ptp::fuji::NoiseReduction) -> anyhow::Result<()> {
            $self.noise_reduction = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, NoiseReduction) => {
        writeln!($f, "Noise Reduction: {}", $self.noise_reduction)?;
    };

    (@cap $self:ident, GrainEffect) => {
        fn set_grain(&mut $self, value: &crate::ptp::fuji::GrainEffect) -> anyhow::Result<()> {
            $self.grain = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, GrainEffect) => {
        writeln!($f, "Grain: {}", $self.grain)?;
    };

    (@cap $self:ident, ColorChromeEffect) => {
        fn set_color_chrome_effect(
            &mut $self,
            value: &crate::ptp::fuji::ColorChromeEffect
        ) -> anyhow::Result<()> {
            $self.color_chrome_effect = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, ColorChromeEffect) => {
        writeln!($f, "Color Chrome Effect: {}", $self.color_chrome_effect)?;
    };

    (@cap $self:ident, ColorChromeFXBlue) => {
        fn set_color_chrome_fx_blue(
            &mut $self,
            value: &crate::ptp::fuji::ColorChromeFXBlue
        ) -> anyhow::Result<()> {
            $self.color_chrome_fx_blue = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, ColorChromeFXBlue) => {
        writeln!($f, "Color Chrome FX Blue: {}", $self.color_chrome_fx_blue)?;
    };

    (@cap $self:ident, SmoothSkinEffect) => {
        fn set_smooth_skin_effect(
            &mut $self,
            value: &crate::ptp::fuji::SmoothSkinEffect
        ) -> anyhow::Result<()> {
            $self.smooth_skin_effect = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, SmoothSkinEffect) => {
        writeln!($f, "Smooth Skin Effect: {}", $self.smooth_skin_effect)?;
    };

    (@cap $self:ident, WhiteBalance) => {
        fn set_white_balance(&mut $self, value: &crate::ptp::fuji::WhiteBalance) -> anyhow::Result<()> {
            $self.white_balance = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, WhiteBalance) => {
        writeln!($f, "White Balance: {}", $self.white_balance)?;
    };

    (@cap $self:ident, WhiteBalanceShift) => {
        fn set_white_balance_shift_red(
            &mut $self,
            value: &crate::ptp::fuji::WhiteBalanceShift
        ) -> anyhow::Result<()> {
            $self.white_balance_shift_red = *value;
            Ok(())
        }

        fn set_white_balance_shift_blue(
            &mut $self,
            value: &crate::ptp::fuji::WhiteBalanceShift
        ) -> anyhow::Result<()> {
            $self.white_balance_shift_blue = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, WhiteBalanceShift) => {
        writeln!(
            $f,
            "White Balance Shift (R/B): {} / {}",
            $self.white_balance_shift_red,
            $self.white_balance_shift_blue
        )?;
    };

    (@cap $self:ident, WhiteBalanceTemperature) => {
        fn set_white_balance_temperature(
            &mut $self,
            value: &crate::ptp::fuji::WhiteBalanceTemperature
        ) -> anyhow::Result<()> {
            $self.white_balance_temperature = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, WhiteBalanceTemperature) => {
        writeln!($f, "White Balance Temperature: {}K", $self.white_balance_temperature)?;
    };

    (@cap $self:ident, DynamicRange) => {
        fn set_dynamic_range(&mut $self, value: &crate::ptp::fuji::DynamicRange) -> anyhow::Result<()> {
            $self.dynamic_range = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, DynamicRange) => {
        writeln!($f, "Dynamic Range: {}", $self.dynamic_range)?;
    };

    (@cap $self:ident, DynamicRangePriority) => {
        fn set_dynamic_range_priority(
            &mut $self,
            value: &crate::ptp::fuji::DynamicRangePriority
        ) -> anyhow::Result<()> {
            $self.dynamic_range_priority = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, DynamicRangePriority) => {
        writeln!($f, "Dynamic Range Priority: {}", $self.dynamic_range_priority)?;
    };

    (@cap $self:ident, LensModulationOptimizer) => {
        fn set_lens_modulation_optimizer(
            &mut $self,
            value: &crate::ptp::fuji::LensModulationOptimizer
        ) -> anyhow::Result<()> {
            $self.lens_modulation_optimizer = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, LensModulationOptimizer) => {
        writeln!($f, "Lens Modulation Optimizer: {}", $self.lens_modulation_optimizer)?;
    };

    (@cap $self:ident, ColorSpace) => {
        fn set_color_space(&mut $self, value: &crate::ptp::fuji::ColorSpace) -> anyhow::Result<()> {
            $self.color_space = *value;
            Ok(())
        }
    };

    (@write $self:ident, $f:ident, ColorSpace) => {
        writeln!($f, "Color Space: {}", $self.color_space)?;
    };
}

pub(crate) use impl_simulation;

macro_rules! define_simulation {
    (
        $sim:ident,
        [ $( $cap:ident ),* $(,)? ]
    ) => {
        crate::features::simulation::define_simulation!(@collect $sim, [ $( $cap ),* ], [ $( $cap ),* ], ());
    };

    (@collect $sim:ident, [], [ $( $cap:ident ),* ], ($($fields:tt)*)) => {
        #[derive(Clone, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $sim {
            pub name: fuji::CustomSettingName,
            $($fields)*
        }

        crate::features::simulation::impl_simulation!(
            $sim,
            [ $( $cap ),* ]
        );
    };

    (@collect $sim:ident, [ImageSize $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub size: fuji::ImageSize,
        ));
    };

    (@collect $sim:ident, [ImageQuality $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub quality: fuji::ImageQuality,
        ));
    };

    (@collect $sim:ident, [FilmSimulation $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub simulation: fuji::FilmSimulation,
        ));
    };

    (@collect $sim:ident, [MonochromaticColorShift $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub monochromatic_color_temperature: fuji::MonochromaticColorShift,
            pub monochromatic_color_tint: fuji::MonochromaticColorShift,
        ));
    };

    (@collect $sim:ident, [Highlight $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub highlight: fuji::HighlightTone,
        ));
    };

    (@collect $sim:ident, [Shadow $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub shadow: fuji::ShadowTone,
        ));
    };

    (@collect $sim:ident, [Color $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub color: fuji::Color,
        ));
    };

    (@collect $sim:ident, [Sharpness $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub sharpness: fuji::Sharpness,
        ));
    };

    (@collect $sim:ident, [Clarity $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub clarity: fuji::Clarity,
        ));
    };

    (@collect $sim:ident, [NoiseReduction $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub noise_reduction: fuji::NoiseReduction,
        ));
    };

    (@collect $sim:ident, [GrainEffect $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub grain: fuji::GrainEffect,
        ));
    };

    (@collect $sim:ident, [ColorChromeEffect $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub color_chrome_effect: fuji::ColorChromeEffect,
        ));
    };

    (@collect $sim:ident, [ColorChromeFXBlue $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub color_chrome_fx_blue: fuji::ColorChromeFXBlue,
        ));
    };

    (@collect $sim:ident, [SmoothSkinEffect $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub smooth_skin_effect: fuji::SmoothSkinEffect,
        ));
    };

    (@collect $sim:ident, [WhiteBalance $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub white_balance: fuji::WhiteBalance,
        ));
    };

    (@collect $sim:ident, [WhiteBalanceShift $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub white_balance_shift_red: fuji::WhiteBalanceShift,
            pub white_balance_shift_blue: fuji::WhiteBalanceShift,
        ));
    };

    (@collect $sim:ident, [WhiteBalanceTemperature $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub white_balance_temperature: fuji::WhiteBalanceTemperature,
        ));
    };

    (@collect $sim:ident, [DynamicRange $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub dynamic_range: fuji::DynamicRange,
        ));
    };

    (@collect $sim:ident, [DynamicRangePriority $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub dynamic_range_priority: fuji::DynamicRangePriority,
        ));
    };

    (@collect $sim:ident, [LensModulationOptimizer $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub lens_modulation_optimizer: fuji::LensModulationOptimizer,
        ));
    };

    (@collect $sim:ident, [ColorSpace $(, $rest:ident )*], $orig:tt, ($($fields:tt)*)) => {
        define_simulation!(@collect $sim, [ $($rest),* ], $orig, (
            $($fields)*
            pub color_space: fuji::ColorSpace,
        ));
    };
}

pub(crate) use define_simulation;
