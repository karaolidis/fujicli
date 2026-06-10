use std::{
    collections::BTreeMap,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use log::{debug, info, warn};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

use crate::workers::fs::{
    atomic::AtomicError,
    slug::{Slug, SlugError},
};

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("invalid library file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error(transparent)]
    Atomic(#[from] AtomicError),

    #[error("library file {path} has a non-UTF-8 or missing filename")]
    NonUtf8Filename { path: PathBuf },

    #[error(
        "library file {path}: name {name:?} slugifies to {actual:?} but file expects {expected:?}"
    )]
    SlugMismatch {
        path: PathBuf,
        name: String,
        actual: Slug,
        expected: String,
    },

    #[error("metadata {meta} has no matching blob at {blob}")]
    MissingBlob { meta: PathBuf, blob: PathBuf },

    #[error("slug {slug} already exists ({existing_name:?})")]
    SlugConflict { slug: Slug, existing_name: String },

    #[error("no library entry with slug {slug}")]
    NotFound { slug: Slug },

    #[error(transparent)]
    InvalidName(#[from] SlugError),
}

#[derive(Debug)]
pub struct LibraryLoadReport {
    pub loaded: usize,
    pub skipped: Vec<LibrarySkippedEntry>,
}

#[derive(Debug)]
pub struct LibrarySkippedEntry {
    pub path: PathBuf,
    pub reason: LibraryError,
}

pub struct LibrarySnapshot<E> {
    pub entries: BTreeMap<Slug, E>,
}

impl<E: std::fmt::Debug> std::fmt::Debug for LibrarySnapshot<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibrarySnapshot")
            .field("entries", &self.entries)
            .finish()
    }
}

impl<E> Default for LibrarySnapshot<E> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

impl<E> LibrarySnapshot<E> {
    pub fn empty() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

pub trait LibraryEntry: Clone + Serialize + DeserializeOwned {
    fn name(&self) -> &str;

    fn load_one(dir: &Path, path: &Path) -> Result<Option<(Slug, Self)>, LibraryError>;
}

pub struct Library<E> {
    pub(in crate::workers::fs) dir: PathBuf,
    pub(in crate::workers::fs) entries: BTreeMap<Slug, E>,
    skipped: usize,
}

impl<E: LibraryEntry> Library<E> {
    pub fn open(dir: PathBuf) -> Result<(Self, LibraryLoadReport), LibraryError> {
        fs::create_dir_all(&dir).map_err(|source| LibraryError::Io {
            path: dir.clone(),
            source,
        })?;
        let (entries, report) = Self::scan(&dir)?;
        info!(
            "opened library at {} ({} loaded, {} skipped)",
            dir.display(),
            report.loaded,
            report.skipped.len(),
        );
        let skipped = report.skipped.len();
        Ok((
            Self {
                dir,
                entries,
                skipped,
            },
            report,
        ))
    }

    pub fn reload(&mut self) -> Result<LibraryLoadReport, LibraryError> {
        let (entries, report) = Self::scan(&self.dir)?;
        self.entries = entries;
        self.skipped = report.skipped.len();
        info!(
            "reloaded library ({} loaded, {} skipped)",
            report.loaded,
            report.skipped.len(),
        );
        Ok(report)
    }

    fn scan(dir: &Path) -> Result<(BTreeMap<Slug, E>, LibraryLoadReport), LibraryError> {
        let read = fs::read_dir(dir).map_err(|source| LibraryError::Io {
            path: dir.to_path_buf(),
            source,
        })?;

        let mut entries = BTreeMap::new();
        let mut skipped = Vec::new();

        for dirent in read {
            let dirent = match dirent {
                Ok(d) => d,
                Err(source) => {
                    skipped.push(LibrarySkippedEntry {
                        path: dir.to_path_buf(),
                        reason: LibraryError::Io {
                            path: dir.to_path_buf(),
                            source,
                        },
                    });
                    continue;
                }
            };

            let path = dirent.path();
            match E::load_one(dir, &path) {
                Ok(Some((slug, entry))) => {
                    entries.insert(slug, entry);
                }
                Ok(None) => {}
                Err(reason) => {
                    warn!("skipping library file {}: {reason}", path.display());
                    skipped.push(LibrarySkippedEntry { path, reason });
                }
            }
        }

        debug!(
            "scan of {} yielded {} entries",
            dir.display(),
            entries.len()
        );
        let loaded = entries.len();
        Ok((entries, LibraryLoadReport { loaded, skipped }))
    }

    pub(in crate::workers::fs) fn read_meta(path: &Path) -> Result<(Slug, E), LibraryError> {
        let file = File::open(path).map_err(|source| LibraryError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let entry: E = serde_json::from_reader(file).map_err(|source| LibraryError::Parse {
            path: path.to_path_buf(),
            source,
        })?;

        let name_slug = Slug::try_from(entry.name())?;
        let file_stem = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
            LibraryError::NonUtf8Filename {
                path: path.to_path_buf(),
            }
        })?;
        if name_slug.as_str() != file_stem {
            return Err(LibraryError::SlugMismatch {
                path: path.to_path_buf(),
                name: entry.name().to_owned(),
                actual: name_slug,
                expected: file_stem.to_owned(),
            });
        }
        Ok((name_slug, entry))
    }

    #[allow(dead_code)]
    pub fn iter(&self) -> impl Iterator<Item = (&Slug, &E)> {
        self.entries.iter()
    }

    #[allow(dead_code)]
    pub fn get(&self, slug: &Slug) -> Option<&E> {
        self.entries.get(slug)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub const fn skipped(&self) -> usize {
        self.skipped
    }

    pub fn snapshot(&self) -> Arc<LibrarySnapshot<E>> {
        Arc::new(LibrarySnapshot {
            entries: self.entries.clone(),
        })
    }
}
