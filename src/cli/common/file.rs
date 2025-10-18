use std::{fs::File, io, path::PathBuf, str::FromStr};

#[derive(Debug, Clone)]
pub enum Input {
    Path(PathBuf),
    Stdin,
}

impl FromStr for Input {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "-" {
            Ok(Self::Stdin)
        } else {
            Ok(Self::Path(PathBuf::from(s)))
        }
    }
}

impl Input {
    pub fn get_reader(&self) -> anyhow::Result<Box<dyn io::Read>> {
        match self {
            Self::Stdin => Ok(Box::new(io::stdin())),
            Self::Path(path) => Ok(Box::new(File::open(path)?)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Output {
    Path(PathBuf),
    Stdout,
}

impl FromStr for Output {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "-" {
            Ok(Self::Stdout)
        } else {
            Ok(Self::Path(PathBuf::from(s)))
        }
    }
}

impl Output {
    pub fn get_writer(&self) -> anyhow::Result<Box<dyn io::Write>> {
        match self {
            Self::Stdout => Ok(Box::new(io::stdout())),
            Self::Path(path) => Ok(Box::new(File::create(path)?)),
        }
    }
}
