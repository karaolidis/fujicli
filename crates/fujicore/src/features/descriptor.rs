use thiserror::Error;

use crate::{generated::options::OptionCategory, input::OptionError};

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

pub trait DescriptorTable {
    type Base: Clone + PartialEq + 'static;

    fn fields(&self) -> &'static [&'static OptionDescriptor<Self::Base>];

    fn visible_fields(&self, canonical: &Self::Base) -> Vec<&'static OptionDescriptor<Self::Base>>;

    fn validate_partial(&self, base: Self::Base) -> Option<Self::Base>;
}
