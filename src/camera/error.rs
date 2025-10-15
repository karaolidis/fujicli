use std::{error::Error, fmt};

#[allow(dead_code)]
#[derive(Debug)]
pub struct UnsupportedFeatureError;

impl fmt::Display for UnsupportedFeatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "feature is not supported for this device")
    }
}

impl Error for UnsupportedFeatureError {}
