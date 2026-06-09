use std::{
    collections::BTreeMap,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use fujicore::{UsbId, generated::simulations::SimulationBase};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;

use crate::workers::fs::{
    atomic::{self, AtomicError},
    slug::{Slug, SlugError},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SimulationLibraryEntry {
    pub name: String,
    pub source_camera: UsbId,
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub modified: OffsetDateTime,
    pub simulation: SimulationBase,
}

#[derive(Debug, Clone)]
pub struct SimulationLibraryEdit {
    pub name: String,
    pub simulation: SimulationBase,
}

#[derive(Debug, Error)]
pub enum SimulationLibraryError {
    #[error("i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("invalid simulation library file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error(transparent)]
    Atomic(#[from] AtomicError),

    #[error("simulation library file {path} has a non-UTF-8 or missing filename")]
    NonUtf8Filename { path: PathBuf },

    #[error(
        "simulation library file {path}: name {name:?} slugifies to {actual:?} but file expects {expected:?}"
    )]
    SlugMismatch {
        path: PathBuf,
        name: String,
        actual: Slug,
        expected: String,
    },

    #[error("slug {slug} already exists ({existing_name:?})")]
    SlugConflict { slug: Slug, existing_name: String },

    #[error("no simulation library entry with slug {slug}")]
    NotFound { slug: Slug },

    #[error(transparent)]
    InvalidName(#[from] SlugError),
}

#[derive(Debug)]
pub struct SimulationLibraryLoadReport {
    pub loaded: usize,
    pub skipped: Vec<SimulationLibrarySkippedEntry>,
}

#[derive(Debug)]
pub struct SimulationLibrarySkippedEntry {
    pub path: PathBuf,
    pub reason: SimulationLibraryError,
}

pub struct SimulationLibrary {
    dir: PathBuf,
    entries: BTreeMap<Slug, SimulationLibraryEntry>,
    skipped: usize,
}

#[derive(Debug, Default)]
pub struct SimulationLibrarySnapshot {
    pub entries: BTreeMap<Slug, SimulationLibraryEntry>,
}

impl SimulationLibrarySnapshot {
    pub fn empty() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl SimulationLibrary {
    pub fn open(
        dir: PathBuf,
    ) -> Result<(Self, SimulationLibraryLoadReport), SimulationLibraryError> {
        fs::create_dir_all(&dir).map_err(|source| SimulationLibraryError::Io {
            path: dir.clone(),
            source,
        })?;
        let (entries, report) = Self::scan(&dir)?;
        info!(
            "opened simulation library at {} ({} loaded, {} skipped)",
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

    pub fn reload(&mut self) -> Result<SimulationLibraryLoadReport, SimulationLibraryError> {
        let (entries, report) = Self::scan(&self.dir)?;
        self.entries = entries;
        self.skipped = report.skipped.len();
        info!(
            "reloaded simulation library ({} loaded, {} skipped)",
            report.loaded,
            report.skipped.len(),
        );
        Ok(report)
    }

    #[allow(dead_code)]
    pub fn iter(&self) -> impl Iterator<Item = (&Slug, &SimulationLibraryEntry)> {
        self.entries.iter()
    }

    pub fn get(&self, slug: &Slug) -> Option<&SimulationLibraryEntry> {
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

    pub fn snapshot(&self) -> Arc<SimulationLibrarySnapshot> {
        Arc::new(SimulationLibrarySnapshot {
            entries: self.entries.clone(),
        })
    }

    pub fn add(
        &mut self,
        init: SimulationLibraryEdit,
        source_camera: UsbId,
    ) -> Result<Slug, SimulationLibraryError> {
        let slug = Slug::try_from(init.name.as_str())?;
        if let Some(existing) = self.entries.get(&slug) {
            return Err(SimulationLibraryError::SlugConflict {
                slug,
                existing_name: existing.name.clone(),
            });
        }

        let now = OffsetDateTime::now_utc();
        let entry = SimulationLibraryEntry {
            name: init.name,
            source_camera,
            created: now,
            modified: now,
            simulation: init.simulation,
        };

        atomic::write_json_atomic(&Self::file_path(&self.dir, &slug), &entry)?;
        info!("added simulation library entry {slug}");
        self.entries.insert(slug.clone(), entry);
        Ok(slug)
    }

    pub fn update(
        &mut self,
        slug: &Slug,
        edit: SimulationLibraryEdit,
    ) -> Result<Slug, SimulationLibraryError> {
        let existing = self
            .entries
            .get(slug)
            .ok_or_else(|| SimulationLibraryError::NotFound { slug: slug.clone() })?;

        let new_slug = Slug::try_from(edit.name.as_str())?;
        if new_slug != *slug
            && let Some(other) = self.entries.get(&new_slug)
        {
            return Err(SimulationLibraryError::SlugConflict {
                slug: new_slug,
                existing_name: other.name.clone(),
            });
        }

        let entry = SimulationLibraryEntry {
            name: edit.name,
            source_camera: existing.source_camera,
            created: existing.created,
            modified: OffsetDateTime::now_utc(),
            simulation: edit.simulation,
        };

        atomic::write_json_atomic(&Self::file_path(&self.dir, &new_slug), &entry)?;

        if new_slug == *slug {
            info!("updated simulation library entry {slug}");
        } else {
            let old_path = Self::file_path(&self.dir, slug);
            if let Err(source) = fs::remove_file(&old_path) {
                let _ = fs::remove_file(Self::file_path(&self.dir, &new_slug));
                return Err(SimulationLibraryError::Io {
                    path: old_path,
                    source,
                });
            }
            self.entries.remove(slug);
            info!("renamed simulation library entry {slug} -> {new_slug}");
        }

        self.entries.insert(new_slug.clone(), entry);
        Ok(new_slug)
    }

    pub fn remove(
        &mut self,
        slug: &Slug,
    ) -> Result<SimulationLibraryEntry, SimulationLibraryError> {
        if !self.entries.contains_key(slug) {
            return Err(SimulationLibraryError::NotFound { slug: slug.clone() });
        }
        let path = Self::file_path(&self.dir, slug);
        fs::remove_file(&path).map_err(|source| SimulationLibraryError::Io { path, source })?;
        let entry = self
            .entries
            .remove(slug)
            .expect("entry presence checked above");
        info!("removed simulation library entry {slug}");
        Ok(entry)
    }
}

impl SimulationLibrary {
    fn file_path(dir: &Path, slug: &Slug) -> PathBuf {
        dir.join(format!("{}.json", slug.as_str()))
    }

    fn scan(
        dir: &Path,
    ) -> Result<
        (
            BTreeMap<Slug, SimulationLibraryEntry>,
            SimulationLibraryLoadReport,
        ),
        SimulationLibraryError,
    > {
        let read = fs::read_dir(dir).map_err(|source| SimulationLibraryError::Io {
            path: dir.to_path_buf(),
            source,
        })?;

        let mut entries = BTreeMap::new();
        let mut skipped = Vec::new();

        for dirent in read {
            let dirent = match dirent {
                Ok(d) => d,
                Err(source) => {
                    skipped.push(SimulationLibrarySkippedEntry {
                        path: dir.to_path_buf(),
                        reason: SimulationLibraryError::Io {
                            path: dir.to_path_buf(),
                            source,
                        },
                    });
                    continue;
                }
            };

            let path = dirent.path();
            match Self::load_one(&path) {
                Ok((slug, entry)) => {
                    entries.insert(slug, entry);
                }
                Err(reason) => {
                    warn!(
                        "skipping simulation library file {}: {reason}",
                        path.display()
                    );
                    skipped.push(SimulationLibrarySkippedEntry { path, reason });
                }
            }
        }

        debug!(
            "scan of {} yielded {} entries",
            dir.display(),
            entries.len()
        );
        let loaded = entries.len();
        Ok((entries, SimulationLibraryLoadReport { loaded, skipped }))
    }

    fn load_one(path: &Path) -> Result<(Slug, SimulationLibraryEntry), SimulationLibraryError> {
        let file = File::open(path).map_err(|source| SimulationLibraryError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let entry: SimulationLibraryEntry =
            serde_json::from_reader(file).map_err(|source| SimulationLibraryError::Parse {
                path: path.to_path_buf(),
                source,
            })?;

        let name_slug = Slug::try_from(entry.name.as_str())?;
        let file_stem = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
            SimulationLibraryError::NonUtf8Filename {
                path: path.to_path_buf(),
            }
        })?;
        if name_slug.as_str() != file_stem {
            return Err(SimulationLibraryError::SlugMismatch {
                path: path.to_path_buf(),
                name: entry.name,
                actual: name_slug,
                expected: file_stem.to_owned(),
            });
        }
        Ok((name_slug, entry))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use fujicore::generated::{options::FilmSimulation, simulations::SimulationBase};
    use tempfile::TempDir;
    use time::OffsetDateTime;

    use super::*;

    fn cam() -> UsbId {
        UsbId {
            vendor: 0x04CB,
            product: 0x02FC,
        }
    }

    fn sim_velvia() -> SimulationBase {
        SimulationBase {
            film_simulation: Some(FilmSimulation::Velvia),
            ..Default::default()
        }
    }

    fn sim_acros() -> SimulationBase {
        SimulationBase {
            film_simulation: Some(FilmSimulation::Acros),
            ..Default::default()
        }
    }

    fn dir() -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_path_buf();
        (tmp, path)
    }

    #[test]
    fn open_creates_missing_directory() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("simulation");
        assert!(!nested.exists());
        let (lib, report) = SimulationLibrary::open(nested.clone()).unwrap();
        assert!(nested.is_dir());
        assert_eq!(lib.len(), 0);
        assert_eq!(report.loaded, 0);
        assert!(report.skipped.is_empty());
    }

    #[test]
    fn add_writes_file_and_roundtrips() {
        let (_tmp, path) = dir();
        let (mut lib, _) = SimulationLibrary::open(path.clone()).unwrap();
        let slug = lib
            .add(
                SimulationLibraryEdit {
                    name: "Velvia Warm".to_owned(),
                    simulation: sim_velvia(),
                },
                cam(),
            )
            .unwrap();
        assert_eq!(slug.as_str(), "velvia-warm");
        assert!(path.join("velvia-warm.json").exists());

        let (lib2, _) = SimulationLibrary::open(path).unwrap();
        let entry = lib2.get(&slug).unwrap();
        assert_eq!(entry.name, "Velvia Warm");
        assert_eq!(entry.source_camera, cam());
        assert_eq!(
            entry.simulation.film_simulation,
            Some(FilmSimulation::Velvia)
        );
    }

    #[test]
    fn add_conflict_fails() {
        let (_tmp, path) = dir();
        let (mut lib, _) = SimulationLibrary::open(path).unwrap();
        lib.add(
            SimulationLibraryEdit {
                name: "Velvia".to_owned(),
                simulation: sim_velvia(),
            },
            cam(),
        )
        .unwrap();
        let err = lib
            .add(
                SimulationLibraryEdit {
                    name: "VELVIA".to_owned(),
                    simulation: sim_acros(),
                },
                cam(),
            )
            .unwrap_err();
        assert!(matches!(err, SimulationLibraryError::SlugConflict { .. }));
    }

    #[test]
    fn update_same_name_bumps_modified_preserves_created() {
        let (_tmp, path) = dir();
        let (mut lib, _) = SimulationLibrary::open(path).unwrap();
        let slug = lib
            .add(
                SimulationLibraryEdit {
                    name: "Velvia".to_owned(),
                    simulation: sim_velvia(),
                },
                cam(),
            )
            .unwrap();
        let created = lib.get(&slug).unwrap().created;
        std::thread::sleep(std::time::Duration::from_millis(10));

        let new_slug = lib
            .update(
                &slug,
                SimulationLibraryEdit {
                    name: "Velvia".to_owned(),
                    simulation: sim_acros(),
                },
            )
            .unwrap();
        assert_eq!(new_slug, slug);
        let entry = lib.get(&slug).unwrap();
        assert_eq!(entry.created, created);
        assert!(entry.modified > created);
        assert_eq!(
            entry.simulation.film_simulation,
            Some(FilmSimulation::Acros)
        );
    }

    #[test]
    fn update_rename_moves_file_and_returns_new_slug() {
        let (_tmp, path) = dir();
        let (mut lib, _) = SimulationLibrary::open(path.clone()).unwrap();
        let slug = lib
            .add(
                SimulationLibraryEdit {
                    name: "Velvia".to_owned(),
                    simulation: sim_velvia(),
                },
                cam(),
            )
            .unwrap();
        let new_slug = lib
            .update(
                &slug,
                SimulationLibraryEdit {
                    name: "Velvia Warm".to_owned(),
                    simulation: sim_velvia(),
                },
            )
            .unwrap();
        assert_eq!(new_slug.as_str(), "velvia-warm");
        assert!(!path.join("velvia.json").exists());
        assert!(path.join("velvia-warm.json").exists());
        assert!(lib.get(&slug).is_none());
        assert!(lib.get(&new_slug).is_some());
    }

    #[test]
    fn update_rename_conflict_fails() {
        let (_tmp, path) = dir();
        let (mut lib, _) = SimulationLibrary::open(path).unwrap();
        let velvia = lib
            .add(
                SimulationLibraryEdit {
                    name: "Velvia".to_owned(),
                    simulation: sim_velvia(),
                },
                cam(),
            )
            .unwrap();
        lib.add(
            SimulationLibraryEdit {
                name: "Acros".to_owned(),
                simulation: sim_acros(),
            },
            cam(),
        )
        .unwrap();
        let err = lib
            .update(
                &velvia,
                SimulationLibraryEdit {
                    name: "ACROS".to_owned(),
                    simulation: sim_velvia(),
                },
            )
            .unwrap_err();
        assert!(matches!(err, SimulationLibraryError::SlugConflict { .. }));
        assert_eq!(lib.get(&velvia).unwrap().name, "Velvia");
    }

    #[test]
    fn update_missing_fails() {
        let (_tmp, path) = dir();
        let (mut lib, _) = SimulationLibrary::open(path).unwrap();
        let phantom = Slug::try_from("not-there").unwrap();
        let err = lib
            .update(
                &phantom,
                SimulationLibraryEdit {
                    name: "X".to_owned(),
                    simulation: sim_velvia(),
                },
            )
            .unwrap_err();
        assert!(matches!(err, SimulationLibraryError::NotFound { .. }));
    }

    #[test]
    fn remove_deletes_file_and_returns_entry() {
        let (_tmp, path) = dir();
        let (mut lib, _) = SimulationLibrary::open(path.clone()).unwrap();
        let slug = lib
            .add(
                SimulationLibraryEdit {
                    name: "Velvia".to_owned(),
                    simulation: sim_velvia(),
                },
                cam(),
            )
            .unwrap();
        let entry = lib.remove(&slug).unwrap();
        assert_eq!(entry.name, "Velvia");
        assert!(!path.join("velvia.json").exists());
        assert!(lib.get(&slug).is_none());
    }

    #[test]
    fn iter_is_sorted_by_slug() {
        let (_tmp, path) = dir();
        let (mut lib, _) = SimulationLibrary::open(path).unwrap();
        for name in ["Velvia", "Acros", "Provia"] {
            lib.add(
                SimulationLibraryEdit {
                    name: name.to_owned(),
                    simulation: sim_velvia(),
                },
                cam(),
            )
            .unwrap();
        }
        let order: Vec<_> = lib.iter().map(|(s, _)| s.as_str().to_owned()).collect();
        assert_eq!(order, vec!["acros", "provia", "velvia"]);
    }

    #[test]
    fn reload_picks_up_files_added_out_of_band() {
        let (_tmp, path) = dir();
        let (mut lib, _) = SimulationLibrary::open(path.clone()).unwrap();
        let slug = lib
            .add(
                SimulationLibraryEdit {
                    name: "Velvia".to_owned(),
                    simulation: sim_velvia(),
                },
                cam(),
            )
            .unwrap();

        let entry = SimulationLibraryEntry {
            name: "Acros".to_owned(),
            source_camera: cam(),
            created: OffsetDateTime::now_utc(),
            modified: OffsetDateTime::now_utc(),
            simulation: sim_acros(),
        };
        let acros_slug = Slug::try_from("Acros").unwrap();
        atomic::write_json_atomic(&SimulationLibrary::file_path(&path, &acros_slug), &entry)
            .unwrap();

        assert_eq!(lib.len(), 1);
        let report = lib.reload().unwrap();
        assert_eq!(report.loaded, 2);
        assert!(lib.get(&slug).is_some());
        assert!(lib.get(&acros_slug).is_some());
    }

    #[test]
    fn corrupt_file_is_reported_and_skipped() {
        let (_tmp, path) = dir();
        std::fs::write(path.join("garbage.json"), "{ not json").unwrap();
        let (lib, report) = SimulationLibrary::open(path).unwrap();
        assert_eq!(lib.len(), 0);
        assert_eq!(report.skipped.len(), 1);
        assert!(matches!(
            report.skipped[0].reason,
            SimulationLibraryError::Parse { .. }
        ));
    }

    #[test]
    fn slug_mismatch_is_reported_and_skipped() {
        let (_tmp, path) = dir();
        let entry = SimulationLibraryEntry {
            name: "Acros".to_owned(),
            source_camera: cam(),
            created: OffsetDateTime::now_utc(),
            modified: OffsetDateTime::now_utc(),
            simulation: sim_acros(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        std::fs::write(path.join("velvia.json"), json).unwrap();

        let (lib, report) = SimulationLibrary::open(path).unwrap();
        assert_eq!(lib.len(), 0);
        assert_eq!(report.skipped.len(), 1);
        assert!(matches!(
            report.skipped[0].reason,
            SimulationLibraryError::SlugMismatch { .. }
        ));
    }
}
