use thiserror::Error;

use crate::{
    features::simulation::SimulationError,
    generated::{options::OptionCategory, simulations::SimulationBase},
    input::OptionError,
};

#[derive(Debug, Clone, Copy)]
pub struct OptionDescriptor<B: 'static> {
    pub name: &'static str,
    pub category: Option<OptionCategory>,
    pub display: fn(&B) -> Option<String>,
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
pub struct EnumOps<B: 'static> {
    pub variants: &'static [&'static str],
    pub cycle: fn(&mut B, Direction, &Validator<'_, B>) -> Result<(), BumpError>,
    pub set_by_index: fn(&mut B, usize, &Validator<'_, B>) -> SetOutcome,
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
    pub validate: fn(SimulationBase) -> Result<SimulationBase, SimulationError>,
    pub validate_partial: fn(SimulationBase) -> Result<SimulationBase, SimulationError>,
}

impl SimulationDescriptors {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::options::{FilmSimulation, MonochromaticColorTemperature};

    const OPT_FILM_SIM: OptionDescriptor<SimulationBase> = OptionDescriptor {
        name: "Film Simulation",
        category: None,
        display: |b| b.film_simulation.as_ref().map(ToString::to_string),
        ops: OptionOps::Enum(EnumOps {
            variants: &["Provia"],
            cycle: |_, _, _| Err(BumpError::Exhausted),
            set_by_index: |_, _, _| SetOutcome::Rejected,
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
    fn shadow_seeding_sets_every_default_unconditionally() {
        let desc = mock_descriptors(|_| panic!("shadow must not call validate_partial"));

        let shadow = desc.new_shadow_default_simulation();

        assert_eq!(shadow.film_simulation, Some(FilmSimulation::Provia));
        assert_eq!(
            shadow.monochromatic_color_temperature,
            Some(MonochromaticColorTemperature::default()),
        );
    }
}
