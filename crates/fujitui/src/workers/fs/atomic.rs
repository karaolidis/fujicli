use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

use serde::Serialize;
use tempfile::NamedTempFile;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AtomicError {
    #[error("i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to serialize at {path}: {source}")]
    Serialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

pub fn write_json_atomic<T: Serialize>(target: &Path, value: &T) -> Result<(), AtomicError> {
    let dir = parent(target)?;
    let mut tmp = NamedTempFile::new_in(dir).map_err(|source| AtomicError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    serde_json::to_writer_pretty(&mut tmp, value).map_err(|source| AtomicError::Serialize {
        path: target.to_path_buf(),
        source,
    })?;
    persist(tmp, target)?;
    sync_dir(dir)?;
    Ok(())
}

pub fn write_bytes_atomic(target: &Path, bytes: &[u8]) -> Result<(), AtomicError> {
    use std::io::Write;

    let dir = parent(target)?;
    let mut tmp = NamedTempFile::new_in(dir).map_err(|source| AtomicError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    tmp.as_file_mut()
        .write_all(bytes)
        .map_err(|source| AtomicError::Io {
            path: target.to_path_buf(),
            source,
        })?;
    persist(tmp, target)?;
    sync_dir(dir)?;
    Ok(())
}

fn persist(tmp: NamedTempFile, target: &Path) -> Result<(), AtomicError> {
    tmp.as_file().sync_all().map_err(|source| AtomicError::Io {
        path: target.to_path_buf(),
        source,
    })?;
    tmp.persist(target).map_err(|e| AtomicError::Io {
        path: target.to_path_buf(),
        source: e.error,
    })?;
    Ok(())
}

fn sync_dir(dir: &Path) -> Result<(), AtomicError> {
    File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(|source| AtomicError::Io {
            path: dir.to_path_buf(),
            source,
        })
}

fn parent(target: &Path) -> Result<&Path, AtomicError> {
    target.parent().ok_or_else(|| AtomicError::Io {
        path: target.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::InvalidInput,
            "target has no parent directory",
        ),
    })
}
