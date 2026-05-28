use thiserror::Error;

use crate::input::OptionError;

#[derive(Debug, Error)]
pub enum SimulationError {
    #[error("{simulation}: required setting `{field}` is missing")]
    MissingField {
        simulation: &'static str,
        field: &'static str,
    },

    #[error("{simulation}: setting `{field}` is not part of this simulation")]
    ForeignField {
        simulation: &'static str,
        field: &'static str,
    },

    #[error("simulation type mismatch: expected {expected}")]
    TypeMismatch { expected: &'static str },

    #[error("invalid simulation: {0}")]
    RuleViolation(&'static str),

    #[error(transparent)]
    Option(#[from] OptionError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
