use thiserror::Error;

#[derive(Debug, Error)]
pub enum OptionError {
    #[error("{type_name} value {value} is out of range [{min}, {max}]")]
    OutOfRange {
        type_name: &'static str,
        value: String,
        min: String,
        max: String,
    },

    #[error("{type_name} value {value} is not aligned to step {step}")]
    StepMisaligned {
        type_name: &'static str,
        value: String,
        step: String,
    },

    #[error("invalid {type_name} value '{input}': {reason}")]
    InvalidValue {
        type_name: &'static str,
        input: String,
        reason: String,
    },

    #[error("unknown {type_name} '{input}'. Did you mean '{suggestion}'?")]
    UnknownWithSuggestion {
        type_name: &'static str,
        input: String,
        suggestion: String,
    },

    #[error("unknown {type_name} '{input}'")]
    Unknown {
        type_name: &'static str,
        input: String,
    },

    #[error("{type_name} value '{input}' is shorter than minimum length {min}")]
    TooShort {
        type_name: &'static str,
        input: String,
        min: u32,
    },

    #[error("{type_name} value '{input}' exceeds maximum length {max}")]
    TooLong {
        type_name: &'static str,
        input: String,
        max: u32,
    },

    #[error("{type_name} value {raw} does not fit in {repr}")]
    WireOverflow {
        type_name: &'static str,
        raw: String,
        repr: &'static str,
    },
}
