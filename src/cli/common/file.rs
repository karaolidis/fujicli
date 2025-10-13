use std::{error::Error, fs::File, io, path::PathBuf, str::FromStr};

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

impl Input {
    pub fn get_reader(&self) -> Result<Box<dyn io::Read>, Box<dyn Error + Send + Sync>> {
        match self {
            Input::Stdin => Ok(Box::new(io::stdin())),
            Input::Path(path) => Ok(Box::new(File::open(path)?)),
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

impl Output {
    pub fn get_writer(&self) -> Result<Box<dyn io::Write>, Box<dyn Error + Send + Sync>> {
        match self {
            Output::Stdout => Ok(Box::new(io::stdout())),
            Output::Path(path) => Ok(Box::new(File::create(path)?)),
        }
    }
}
