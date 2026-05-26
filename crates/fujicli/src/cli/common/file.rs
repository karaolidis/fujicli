use std::{
    convert::Infallible,
    fs::File,
    io,
    ops::Deref,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::Context;
use tempfile::NamedTempFile;

#[derive(Debug, Clone)]
pub enum Input {
    Path(PathBuf),
    Stdin,
}

impl FromStr for Input {
    type Err = Infallible;

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
            Self::Path(path) => {
                let file = File::open(path)
                    .with_context(|| format!("opening input file {}", path.display()))?;
                Ok(Box::new(file))
            }
        }
    }

    pub fn into_path(self) -> anyhow::Result<Box<dyn Deref<Target = Path>>> {
        match self {
            Self::Path(p) => Ok(Box::new(p)),
            Self::Stdin => {
                let mut tempfile =
                    NamedTempFile::new().context("creating temporary file for stdin input")?;
                io::copy(&mut io::stdin(), &mut tempfile)
                    .context("copying stdin to temporary file")?;
                Ok(Box::new(tempfile.into_temp_path()))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Output {
    Path(PathBuf),
    Stdout,
}

impl FromStr for Output {
    type Err = Infallible;

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
            Self::Path(path) => {
                let file = File::create(path)
                    .with_context(|| format!("creating output file {}", path.display()))?;
                Ok(Box::new(file))
            }
        }
    }
}
