use strum::IntoEnumIterator;
use thiserror::Error;

use crate::{
    features::simulation::SimulationError,
    generated::{
        options::{CustomSetting, OptionCategory},
        simulations::SimulationBase,
    },
    input::OptionError,
};

#[derive(Debug, Clone, Copy)]
pub struct OptionDescriptor<B: 'static> {
    pub name: &'static str,
    pub category: Option<OptionCategory>,
    pub display: fn(&B) -> Option<String>,
    pub copy_from: fn(dst: &mut B, src: &B),
    pub eq: fn(a: &B, b: &B) -> bool,
    pub ops: OptionOps<B>,
}

impl<B: 'static> OptionDescriptor<B> {
    pub fn set_default(&self, base: &mut B) {
        match &self.ops {
            OptionOps::Enum(ops) => (ops.set_default)(base),
            OptionOps::Integer(ops) => (ops.set_default)(base),
            OptionOps::Float(ops) => (ops.set_default)(base),
            OptionOps::String(ops) => (ops.set_default)(base),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OptionOps<B: 'static> {
    Enum(EnumOps<B>),
    Integer(IntegerOps<B>),
    Float(FloatOps<B>),
    String(StringOps<B>),
}

#[derive(Debug, Clone, Copy)]
pub struct VariantInfo {
    pub id: &'static str,
    pub name: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct EnumOps<B: 'static> {
    pub variants: &'static [VariantInfo],
    pub cycle: fn(&mut B, Direction, &Validator<'_, B>) -> Result<(), BumpError>,
    pub set_by_id: fn(&mut B, &str, &Validator<'_, B>) -> SetOutcome,
    pub set_default: fn(&mut B),
}

#[derive(Debug, Clone, Copy)]
pub struct IntegerOps<B: 'static> {
    pub min: i32,
    pub max: i32,
    pub step: i32,
    pub jump: i32,
    pub step_fn: fn(&mut B, Direction, Magnitude, &Validator<'_, B>) -> Result<(), BumpError>,
    pub jump_fn: fn(&mut B, Extreme, &Validator<'_, B>) -> Result<(), BumpError>,
    pub set_default: fn(&mut B),
}

#[derive(Debug, Clone, Copy)]
pub struct FloatOps<B: 'static> {
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub jump: f32,
    pub step_fn: fn(&mut B, Direction, Magnitude, &Validator<'_, B>) -> Result<(), BumpError>,
    pub jump_fn: fn(&mut B, Extreme, &Validator<'_, B>) -> Result<(), BumpError>,
    pub set_default: fn(&mut B),
}

#[derive(Debug, Clone, Copy)]
pub struct StringOps<B: 'static> {
    pub max_len: Option<usize>,
    pub set_by_text: fn(&mut B, &str, &Validator<'_, B>) -> SetOutcome,
    pub set_default: fn(&mut B),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Prev,
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Magnitude {
    Single,
    Big,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extreme {
    Min,
    Max,
}

#[derive(Debug)]
pub enum SetOutcome {
    Set,
    Rejected,
    InvalidInput(OptionError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BumpError {
    #[error("validator rejected every candidate in the requested direction")]
    Exhausted,
    #[error("field has no current value to bump from")]
    Unset,
}

pub type Validator<'a, B> = dyn Fn(B) -> Option<B> + 'a;

#[derive(Debug, Clone, Copy)]
pub struct SimulationDescriptors {
    pub fields: &'static [&'static OptionDescriptor<SimulationBase>],
    pub slots: usize,
    pub validate: fn(SimulationBase) -> Result<SimulationBase, SimulationError>,
    pub validate_partial: fn(SimulationBase) -> Result<SimulationBase, SimulationError>,
}

impl SimulationDescriptors {
    #[must_use]
    pub fn slots(&self) -> Vec<CustomSetting> {
        CustomSetting::iter().take(self.slots).collect()
    }

    pub fn partial_validator(&self) -> impl Fn(SimulationBase) -> Option<SimulationBase> + '_ {
        move |b| (self.validate_partial)(b).ok()
    }

    #[must_use]
    pub fn visible_fields(
        &self,
        canonical: &SimulationBase,
    ) -> Vec<&'static OptionDescriptor<SimulationBase>> {
        let mut categories: Vec<Option<OptionCategory>> = Vec::new();
        for field in self.fields {
            if !categories.contains(&field.category) {
                categories.push(field.category);
            }
        }
        let mut out = Vec::with_capacity(self.fields.len());
        for category in categories {
            for field in self.fields {
                if field.category == category && (field.display)(canonical).is_some() {
                    out.push(*field);
                }
            }
        }
        out
    }

    #[must_use]
    pub fn new_canonical_default_simulation(&self) -> SimulationBase {
        let mut canonical = SimulationBase::default();
        for desc in self.fields {
            let mut candidate = canonical.clone();
            desc.set_default(&mut candidate);
            if let Ok(settled) = (self.validate_partial)(candidate) {
                canonical = settled;
            }
        }
        canonical
    }

    #[must_use]
    pub fn new_shadow_default_simulation(&self) -> SimulationBase {
        let mut shadow = SimulationBase::default();
        for desc in self.fields {
            desc.set_default(&mut shadow);
        }
        shadow
    }

    #[must_use]
    pub fn new_shadow_from(&self, base: &SimulationBase) -> SimulationBase {
        let mut shadow = base.clone();
        for desc in self.fields {
            if (desc.display)(&shadow).is_some() {
                continue;
            }
            let mut probe = shadow.clone();
            desc.set_default(&mut probe);
            let blocked = (self.validate_partial)(probe.clone())
                .map_or(true, |repaired| (desc.display)(&repaired).is_none());
            if blocked {
                (desc.copy_from)(&mut shadow, &probe);
            }
        }
        shadow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::options::{FilmSimulation, MonochromaticColorTemperature};

    const OPT_FILM_SIM: OptionDescriptor<SimulationBase> = OptionDescriptor {
        name: "Film Simulation",
        category: None,
        display: |b| b.film_simulation.as_ref().map(ToString::to_string),
        copy_from: |dst, src| dst.film_simulation = src.film_simulation,
        eq: |a, b| a.film_simulation == b.film_simulation,
        ops: OptionOps::Enum(EnumOps {
            variants: &[VariantInfo {
                id: "provia",
                name: "Provia",
            }],
            cycle: |_, _, _| Err(BumpError::Exhausted),
            set_by_id: |_, _, _| SetOutcome::Rejected,
            set_default: |b| b.film_simulation = Some(FilmSimulation::default()),
        }),
    };

    const OPT_MONO_TEMP: OptionDescriptor<SimulationBase> = OptionDescriptor {
        name: "Monochromatic Color Temperature",
        category: None,
        display: |b| {
            b.monochromatic_color_temperature
                .as_ref()
                .map(ToString::to_string)
        },
        copy_from: |dst, src| {
            dst.monochromatic_color_temperature = src.monochromatic_color_temperature;
        },
        eq: |a, b| a.monochromatic_color_temperature == b.monochromatic_color_temperature,
        ops: OptionOps::Integer(IntegerOps {
            min: -9,
            max: 9,
            step: 1,
            jump: 10,
            step_fn: |_, _, _, _| Err(BumpError::Exhausted),
            jump_fn: |_, _, _| Err(BumpError::Exhausted),
            set_default: |b| {
                b.monochromatic_color_temperature = Some(MonochromaticColorTemperature::default());
            },
        }),
    };

    const fn mock_descriptors(
        validate_partial: fn(SimulationBase) -> Result<SimulationBase, SimulationError>,
    ) -> SimulationDescriptors {
        SimulationDescriptors {
            fields: &[&OPT_FILM_SIM, &OPT_MONO_TEMP],
            slots: 0,
            validate: |_| panic!("validate is unused by seeding"),
            validate_partial,
        }
    }

    #[test]
    fn canonical_seeding_skips_defaults_the_validator_rejects() {
        let desc = mock_descriptors(|b| {
            let film_is_mono = b
                .film_simulation
                .is_some_and(|v| matches!(v, FilmSimulation::Monochrome));
            if b.monochromatic_color_temperature.is_some() && !film_is_mono {
                Err(SimulationError::RuleViolation("conflict"))
            } else {
                Ok(b)
            }
        });

        let canonical = desc.new_canonical_default_simulation();

        assert_eq!(canonical.film_simulation, Some(FilmSimulation::Provia));
        assert_eq!(canonical.monochromatic_color_temperature, None);
    }

    #[test]
    fn new_shadow_from_seeds_only_rule_blocked_fields() {
        let desc = mock_descriptors(|b| {
            let film_is_mono = b
                .film_simulation
                .is_some_and(|v| matches!(v, FilmSimulation::Monochrome));
            if b.monochromatic_color_temperature.is_some() && !film_is_mono {
                Err(SimulationError::RuleViolation("conflict"))
            } else {
                Ok(b)
            }
        });

        let base = SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            monochromatic_color_temperature: None,
            ..Default::default()
        };

        let shadow = desc.new_shadow_from(&base);

        assert_eq!(shadow.film_simulation, Some(FilmSimulation::Provia));
        assert_eq!(
            shadow.monochromatic_color_temperature,
            Some(MonochromaticColorTemperature::default()),
        );
    }

    #[test]
    fn shadow_from_leaves_rule_allowed_fields_unset() {
        let desc = mock_descriptors(Ok);

        let base = SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            monochromatic_color_temperature: None,
            ..Default::default()
        };
        let shadow = desc.new_shadow_from(&base);

        assert_eq!(shadow.monochromatic_color_temperature, None);
    }

    #[test]
    fn shadow_seeding_sets_every_default_unconditionally() {
        let desc = mock_descriptors(|_| panic!("shadow must not call validate_partial"));

        let shadow = desc.new_shadow_default_simulation();

        assert_eq!(shadow.film_simulation, Some(FilmSimulation::Provia));
        assert_eq!(
            shadow.monochromatic_color_temperature,
            Some(MonochromaticColorTemperature::default()),
        );
    }

    #[test]
    fn canonical_default_simulation_validates_for_every_supported_camera() {
        for cam in crate::generated::cameras::SUPPORTED {
            let Some(descriptors) = cam.simulation else {
                continue;
            };
            let canonical = descriptors.new_canonical_default_simulation();
            let result = (descriptors.validate)(canonical);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn codegen_copy_from_and_eq_round_trip_for_every_supported_camera() {
        for cam in crate::generated::cameras::SUPPORTED {
            let Some(descriptors) = cam.simulation else {
                continue;
            };
            let source = descriptors.new_shadow_default_simulation();
            for desc in descriptors.fields {
                if (desc.display)(&source).is_none() {
                    continue;
                }
                let mut target = SimulationBase::default();
                (desc.copy_from)(&mut target, &source);
                assert!((desc.eq)(&target, &source));
                assert_eq!((desc.display)(&target), (desc.display)(&source));
            }
        }
    }

    fn enum_ops(desc: &OptionDescriptor<SimulationBase>) -> &EnumOps<SimulationBase> {
        match &desc.ops {
            OptionOps::Enum(ops) => ops,
            _ => panic!("expected enum ops"),
        }
    }

    #[test]
    fn set_by_id_reports_rejected_when_validator_strips_the_requested_field() {
        let desc = crate::generated::descriptors::OPT_FILM_SIMULATION;
        let stripping_validator = |mut b: SimulationBase| -> Option<SimulationBase> {
            b.film_simulation = None;
            Some(b)
        };
        let mut base = SimulationBase::default();
        let outcome = (enum_ops(&desc).set_by_id)(&mut base, "velvia", &stripping_validator);
        assert!(matches!(outcome, SetOutcome::Rejected));
        assert_eq!(base.film_simulation, None);
    }

    #[test]
    fn set_by_id_accepts_when_validator_preserves_the_requested_field() {
        let desc = crate::generated::descriptors::OPT_FILM_SIMULATION;
        let identity = |b: SimulationBase| Some(b);
        let mut base = SimulationBase::default();
        let outcome = (enum_ops(&desc).set_by_id)(&mut base, "velvia", &identity);
        assert!(matches!(outcome, SetOutcome::Set));
        assert_eq!(base.film_simulation, Some(FilmSimulation::Velvia));
    }

    #[test]
    fn set_by_id_rejects_unknown_id() {
        let desc = crate::generated::descriptors::OPT_FILM_SIMULATION;
        let identity = |b: SimulationBase| Some(b);
        let mut base = SimulationBase::default();
        let outcome = (enum_ops(&desc).set_by_id)(&mut base, "Velvia", &identity);
        assert!(matches!(outcome, SetOutcome::Rejected));
    }

    #[test]
    fn cycle_skips_variants_that_repair_strips() {
        let desc = crate::generated::descriptors::OPT_FILM_SIMULATION;
        let strip_all = |mut b: SimulationBase| -> Option<SimulationBase> {
            b.film_simulation = None;
            Some(b)
        };
        let mut base = SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        };
        let outcome = (enum_ops(&desc).cycle)(&mut base, Direction::Next, &strip_all);
        assert_eq!(outcome, Err(BumpError::Exhausted));
        assert_eq!(base.film_simulation, Some(FilmSimulation::Provia));
    }

    #[test]
    fn variants_carry_canonical_id_and_display_name() {
        let desc = crate::generated::descriptors::OPT_FILM_SIMULATION;
        let provia = enum_ops(&desc)
            .variants
            .iter()
            .find(|v| v.id == "provia")
            .expect("provia variant present");
        assert_eq!(provia.name, "Provia");
    }

    #[test]
    fn custom_setting_name_derefs_to_str() {
        use crate::generated::options::CustomSettingName;
        let name = CustomSettingName::default();
        let s: &str = &name;
        assert_eq!(s, "");
        assert!(name.is_empty());
    }

    fn integer_ops(desc: &OptionDescriptor<SimulationBase>) -> &IntegerOps<SimulationBase> {
        match &desc.ops {
            OptionOps::Integer(ops) => ops,
            _ => panic!("expected integer ops"),
        }
    }

    fn float_ops(desc: &OptionDescriptor<SimulationBase>) -> &FloatOps<SimulationBase> {
        match &desc.ops {
            OptionOps::Float(ops) => ops,
            _ => panic!("expected float ops"),
        }
    }

    #[test]
    fn integer_scaled_big_step_clamps_to_max_on_overshoot() {
        use crate::generated::options::Clarity;
        let desc = crate::generated::descriptors::OPT_CLARITY;
        let identity = |b: SimulationBase| Some(b);
        let mut base = SimulationBase {
            clarity: Some(Clarity::try_from(4).unwrap()),
            ..Default::default()
        };
        let outcome =
            (integer_ops(&desc).step_fn)(&mut base, Direction::Next, Magnitude::Big, &identity);
        assert_eq!(outcome, Ok(()));
        assert_eq!(base.clarity, Some(Clarity::try_from(5).unwrap()));
    }

    #[test]
    fn integer_scaled_big_step_clamps_to_min_on_undershoot() {
        use crate::generated::options::Clarity;
        let desc = crate::generated::descriptors::OPT_CLARITY;
        let identity = |b: SimulationBase| Some(b);
        let mut base = SimulationBase {
            clarity: Some(Clarity::try_from(-4).unwrap()),
            ..Default::default()
        };
        let outcome =
            (integer_ops(&desc).step_fn)(&mut base, Direction::Prev, Magnitude::Big, &identity);
        assert_eq!(outcome, Ok(()));
        assert_eq!(base.clarity, Some(Clarity::try_from(-5).unwrap()));
    }

    #[test]
    fn integer_scaled_step_returns_exhausted_when_already_at_boundary() {
        use crate::generated::options::Clarity;
        let desc = crate::generated::descriptors::OPT_CLARITY;
        let identity = |b: SimulationBase| Some(b);
        let mut base = SimulationBase {
            clarity: Some(Clarity::try_from(5).unwrap()),
            ..Default::default()
        };
        let big = (integer_ops(&desc).step_fn)(
            &mut base.clone(),
            Direction::Next,
            Magnitude::Big,
            &identity,
        );
        assert_eq!(big, Err(BumpError::Exhausted));
        let single =
            (integer_ops(&desc).step_fn)(&mut base, Direction::Next, Magnitude::Single, &identity);
        assert_eq!(single, Err(BumpError::Exhausted));
    }

    #[test]
    fn integer_scaled_inward_walk_skips_validator_rejected_boundary() {
        use crate::generated::options::Clarity;
        let desc = crate::generated::descriptors::OPT_CLARITY;
        let reject_max = |b: SimulationBase| -> Option<SimulationBase> {
            if b.clarity == Some(Clarity::try_from(5).unwrap()) {
                None
            } else {
                Some(b)
            }
        };
        let mut base = SimulationBase {
            clarity: Some(Clarity::try_from(0).unwrap()),
            ..Default::default()
        };
        let outcome =
            (integer_ops(&desc).step_fn)(&mut base, Direction::Next, Magnitude::Big, &reject_max);
        assert_eq!(outcome, Ok(()));
        assert_eq!(base.clarity, Some(Clarity::try_from(4).unwrap()));
    }

    #[test]
    fn integer_lookup_big_step_clamps_to_last_variant_on_overshoot() {
        use crate::generated::options::NoiseReduction;
        let desc = crate::generated::descriptors::OPT_NOISE_REDUCTION;
        let identity = |b: SimulationBase| Some(b);
        let mut base = SimulationBase {
            noise_reduction: Some(NoiseReduction::Plus3),
            ..Default::default()
        };
        let outcome =
            (integer_ops(&desc).step_fn)(&mut base, Direction::Next, Magnitude::Big, &identity);
        assert_eq!(outcome, Ok(()));
        assert_eq!(base.noise_reduction, Some(NoiseReduction::Plus4));
    }

    #[test]
    fn integer_lookup_big_step_clamps_to_first_variant_on_undershoot() {
        use crate::generated::options::NoiseReduction;
        let desc = crate::generated::descriptors::OPT_NOISE_REDUCTION;
        let identity = |b: SimulationBase| Some(b);
        let mut base = SimulationBase {
            noise_reduction: Some(NoiseReduction::Minus3),
            ..Default::default()
        };
        let outcome =
            (integer_ops(&desc).step_fn)(&mut base, Direction::Prev, Magnitude::Big, &identity);
        assert_eq!(outcome, Ok(()));
        assert_eq!(base.noise_reduction, Some(NoiseReduction::Minus4));
    }

    #[test]
    fn integer_lookup_step_returns_exhausted_when_already_at_terminal_variant() {
        use crate::generated::options::NoiseReduction;
        let desc = crate::generated::descriptors::OPT_NOISE_REDUCTION;
        let identity = |b: SimulationBase| Some(b);
        let mut base = SimulationBase {
            noise_reduction: Some(NoiseReduction::Plus4),
            ..Default::default()
        };
        let outcome =
            (integer_ops(&desc).step_fn)(&mut base, Direction::Next, Magnitude::Big, &identity);
        assert_eq!(outcome, Err(BumpError::Exhausted));
    }

    #[test]
    fn float_scaled_big_step_clamps_to_max_on_overshoot() {
        use crate::generated::options::HighlightTone;
        let desc = crate::generated::descriptors::OPT_HIGHLIGHT_TONE;
        let identity = |b: SimulationBase| Some(b);
        let mut base = SimulationBase {
            highlight_tone: Some(HighlightTone::try_from(3.5_f32).unwrap()),
            ..Default::default()
        };
        let outcome =
            (float_ops(&desc).step_fn)(&mut base, Direction::Next, Magnitude::Big, &identity);
        assert_eq!(outcome, Ok(()));
        assert_eq!(
            base.highlight_tone,
            Some(HighlightTone::try_from(4.0_f32).unwrap())
        );
    }

    #[test]
    fn float_scaled_big_step_clamps_to_min_on_undershoot() {
        use crate::generated::options::HighlightTone;
        let desc = crate::generated::descriptors::OPT_HIGHLIGHT_TONE;
        let identity = |b: SimulationBase| Some(b);
        let mut base = SimulationBase {
            highlight_tone: Some(HighlightTone::try_from(-1.5_f32).unwrap()),
            ..Default::default()
        };
        let outcome =
            (float_ops(&desc).step_fn)(&mut base, Direction::Prev, Magnitude::Big, &identity);
        assert_eq!(outcome, Ok(()));
        assert_eq!(
            base.highlight_tone,
            Some(HighlightTone::try_from(-2.0_f32).unwrap())
        );
    }
}
