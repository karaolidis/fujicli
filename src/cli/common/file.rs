use std::{error::Error, path::PathBuf, str::FromStr};

#[derive(Debug, Clone)]
pub enum Input {
    Path(PathBuf),
    Stdin,
}

impl FromStr for Input {
    type Err = Box<dyn Error + Send + Sync>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "-" {
            Ok(Input::Stdin)
        } else {
            Ok(Input::Path(PathBuf::from(s)))
        }
    }
}

#[derive(Debug, Clone)]
pub enum Output {
    Path(PathBuf),
    Stdout,
}

impl FromStr for Output {
    type Err = Box<dyn Error + Send + Sync>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "-" {
            Ok(Output::Stdout)
        } else {
            Ok(Output::Path(PathBuf::from(s)))
        }
    }
}
