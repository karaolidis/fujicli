use std::{
    collections::BTreeMap,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use fujicore::generated::simulations::SimulationBase;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;
use time::OffsetDateTime;

use super::slug::Slug;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceCamera {
    pub vendor: u16,
    pub product: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LibraryEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub source_camera: SourceCamera,
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub modified: OffsetDateTime,
    pub simulation: SimulationBase,
}

#[derive(Debug, Clone)]
pub struct EntryEdit {
    pub name: String,
    pub description: Option<String>,
    pub simulation: SimulationBase,
}

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

    #[error(
        "library file {path}: name {name:?} slugifies to {actual:?} but file expects {expected:?}"
    )]
    SlugMismatch {
        path: PathBuf,
        name: String,
        actual: Slug,
        expected: String,
    },

    #[error("slug {slug} already exists ({existing_name:?})")]
    SlugConflict { slug: Slug, existing_name: String },

    #[error("no library entry with slug {slug}")]
    NotFound { slug: Slug },

    #[error("name cannot be slugified (empty or non-slug-compatible characters only)")]
    InvalidName,
}

#[derive(Debug)]
pub struct LoadReport {
    pub loaded: usize,
    pub skipped: Vec<SkippedEntry>,
}

#[derive(Debug)]
pub struct SkippedEntry {
    pub path: PathBuf,
    pub reason: LibraryError,
}

pub struct SimLibrary {
    dir: PathBuf,
    entries: BTreeMap<Slug, LibraryEntry>,
}

impl SimLibrary {
    pub fn open(dir: PathBuf) -> Result<(Self, LoadReport), LibraryError> {
        fs::create_dir_all(&dir).map_err(|source| LibraryError::Io {
            path: dir.clone(),
            source,
        })?;
        let (entries, report) = scan(&dir)?;
        info!(
            "opened simulation library at {} ({} loaded, {} skipped)",
            dir.display(),
            report.loaded,
            report.skipped.len(),
        );
        Ok((Self { dir, entries }, report))
    }

    pub fn reload(&mut self) -> Result<LoadReport, LibraryError> {
        let (entries, report) = scan(&self.dir)?;
        self.entries = entries;
        info!(
            "reloaded simulation library ({} loaded, {} skipped)",
            report.loaded,
            report.skipped.len(),
        );
        Ok(report)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Slug, &LibraryEntry)> {
        self.entries.iter()
    }

    pub fn get(&self, slug: &Slug) -> Option<&LibraryEntry> {
        self.entries.get(slug)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn add(
        &mut self,
        init: EntryEdit,
        source_camera: SourceCamera,
    ) -> Result<Slug, LibraryError> {
        let slug = Slug::from_name(&init.name)?;
        if let Some(existing) = self.entries.get(&slug) {
            return Err(LibraryError::SlugConflict {
                slug,
                existing_name: existing.name.clone(),
            });
        }

        let now = OffsetDateTime::now_utc();
        let entry = LibraryEntry {
            name: init.name,
            description: init.description,
            source_camera,
            created: now,
            modified: now,
            simulation: init.simulation,
        };

        write_atomic(&self.dir, &slug, &entry)?;
        info!("added library entry {slug}");
        self.entries.insert(slug.clone(), entry);
        Ok(slug)
    }

    pub fn update(&mut self, slug: &Slug, edit: EntryEdit) -> Result<Slug, LibraryError> {
        let existing = self
            .entries
            .get(slug)
            .ok_or_else(|| LibraryError::NotFound { slug: slug.clone() })?;

        let new_slug = Slug::from_name(&edit.name)?;
        if new_slug != *slug
            && let Some(other) = self.entries.get(&new_slug)
        {
            return Err(LibraryError::SlugConflict {
                slug: new_slug,
                existing_name: other.name.clone(),
            });
        }

        let entry = LibraryEntry {
            name: edit.name,
            description: edit.description,
            source_camera: existing.source_camera,
            created: existing.created,
            modified: OffsetDateTime::now_utc(),
            simulation: edit.simulation,
        };

        write_atomic(&self.dir, &new_slug, &entry)?;

        if new_slug == *slug {
            info!("updated library entry {slug}");
        } else {
            let old_path = file_path(&self.dir, slug);
            if let Err(e) = fs::remove_file(&old_path) {
                warn!(
                    "rename: failed to delete old file {}: {e}",
                    old_path.display()
                );
            }
            self.entries.remove(slug);
            info!("renamed library entry {slug} -> {new_slug}");
        }

        self.entries.insert(new_slug.clone(), entry);
        Ok(new_slug)
    }

    pub fn remove(&mut self, slug: &Slug) -> Result<LibraryEntry, LibraryError> {
        let entry = self
            .entries
            .remove(slug)
            .ok_or_else(|| LibraryError::NotFound { slug: slug.clone() })?;
        let path = file_path(&self.dir, slug);
        fs::remove_file(&path).map_err(|source| LibraryError::Io { path, source })?;
        info!("removed library entry {slug}");
        Ok(entry)
    }
}

fn file_path(dir: &Path, slug: &Slug) -> PathBuf {
    dir.join(format!("{}.json", slug.as_str()))
}

fn write_atomic(dir: &Path, slug: &Slug, entry: &LibraryEntry) -> Result<(), LibraryError> {
    let target = file_path(dir, slug);
    let mut tmp = NamedTempFile::new_in(dir).map_err(|source| LibraryError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    serde_json::to_writer_pretty(&mut tmp, entry).map_err(|source| LibraryError::Parse {
        path: target.clone(),
        source,
    })?;
    tmp.persist(&target).map_err(|e| LibraryError::Io {
        path: target,
        source: e.error,
    })?;
    Ok(())
}

fn scan(dir: &Path) -> Result<(BTreeMap<Slug, LibraryEntry>, LoadReport), LibraryError> {
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
                skipped.push(SkippedEntry {
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
        match load_one(&path) {
            Ok((slug, entry)) => {
                entries.insert(slug, entry);
            }
            Err(reason) => {
                warn!("skipping library file {}: {reason}", path.display());
                skipped.push(SkippedEntry { path, reason });
            }
        }
    }

    debug!(
        "scan of {} yielded {} entries",
        dir.display(),
        entries.len()
    );
    let loaded = entries.len();
    Ok((entries, LoadReport { loaded, skipped }))
}

fn load_one(path: &Path) -> Result<(Slug, LibraryEntry), LibraryError> {
    let file = File::open(path).map_err(|source| LibraryError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let entry: LibraryEntry =
        serde_json::from_reader(file).map_err(|source| LibraryError::Parse {
            path: path.to_path_buf(),
            source,
        })?;

    let name_slug = Slug::from_name(&entry.name)?;
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_owned();
    if name_slug.as_str() != file_stem {
        return Err(LibraryError::SlugMismatch {
            path: path.to_path_buf(),
            name: entry.name,
            actual: name_slug,
            expected: file_stem,
        });
    }
    Ok((name_slug, entry))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use fujicore::generated::{options::FilmSimulation, simulations::SimulationBase};
    use tempfile::TempDir;
    use time::OffsetDateTime;

    use super::*;

    fn cam() -> SourceCamera {
        SourceCamera {
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
        let nested = tmp.path().join("library");
        assert!(!nested.exists());
        let (lib, report) = SimLibrary::open(nested.clone()).unwrap();
        assert!(nested.is_dir());
        assert_eq!(lib.len(), 0);
        assert_eq!(report.loaded, 0);
        assert!(report.skipped.is_empty());
    }

    #[test]
    fn add_writes_file_and_roundtrips() {
        let (_tmp, path) = dir();
        let (mut lib, _) = SimLibrary::open(path.clone()).unwrap();
        let slug = lib
            .add(
                EntryEdit {
                    name: "Velvia Warm".to_owned(),
                    description: Some("warm push".to_owned()),
                    simulation: sim_velvia(),
                },
                cam(),
            )
            .unwrap();
        assert_eq!(slug.as_str(), "velvia-warm");
        assert!(path.join("velvia-warm.json").exists());

        let (lib2, _) = SimLibrary::open(path).unwrap();
        let entry = lib2.get(&slug).unwrap();
        assert_eq!(entry.name, "Velvia Warm");
        assert_eq!(entry.description.as_deref(), Some("warm push"));
        assert_eq!(entry.source_camera, cam());
        assert_eq!(
            entry.simulation.film_simulation,
            Some(FilmSimulation::Velvia)
        );
    }

    #[test]
    fn add_conflict_fails() {
        let (_tmp, path) = dir();
        let (mut lib, _) = SimLibrary::open(path).unwrap();
        lib.add(
            EntryEdit {
                name: "Velvia".to_owned(),
                description: None,
                simulation: sim_velvia(),
            },
            cam(),
        )
        .unwrap();
        let err = lib
            .add(
                EntryEdit {
                    name: "VELVIA".to_owned(),
                    description: None,
                    simulation: sim_acros(),
                },
                cam(),
            )
            .unwrap_err();
        assert!(matches!(err, LibraryError::SlugConflict { .. }));
    }

    #[test]
    fn update_same_name_bumps_modified_preserves_created() {
        let (_tmp, path) = dir();
        let (mut lib, _) = SimLibrary::open(path).unwrap();
        let slug = lib
            .add(
                EntryEdit {
                    name: "Velvia".to_owned(),
                    description: None,
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
                EntryEdit {
                    name: "Velvia".to_owned(),
                    description: Some("edited".to_owned()),
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
        let (mut lib, _) = SimLibrary::open(path.clone()).unwrap();
        let slug = lib
            .add(
                EntryEdit {
                    name: "Velvia".to_owned(),
                    description: None,
                    simulation: sim_velvia(),
                },
                cam(),
            )
            .unwrap();
        let new_slug = lib
            .update(
                &slug,
                EntryEdit {
                    name: "Velvia Warm".to_owned(),
                    description: None,
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
        let (mut lib, _) = SimLibrary::open(path).unwrap();
        let velvia = lib
            .add(
                EntryEdit {
                    name: "Velvia".to_owned(),
                    description: None,
                    simulation: sim_velvia(),
                },
                cam(),
            )
            .unwrap();
        lib.add(
            EntryEdit {
                name: "Acros".to_owned(),
                description: None,
                simulation: sim_acros(),
            },
            cam(),
        )
        .unwrap();
        let err = lib
            .update(
                &velvia,
                EntryEdit {
                    name: "ACROS".to_owned(),
                    description: None,
                    simulation: sim_velvia(),
                },
            )
            .unwrap_err();
        assert!(matches!(err, LibraryError::SlugConflict { .. }));
        assert_eq!(lib.get(&velvia).unwrap().name, "Velvia");
    }

    #[test]
    fn update_missing_fails() {
        let (_tmp, path) = dir();
        let (mut lib, _) = SimLibrary::open(path).unwrap();
        let phantom = Slug::from_name("not-there").unwrap();
        let err = lib
            .update(
                &phantom,
                EntryEdit {
                    name: "X".to_owned(),
                    description: None,
                    simulation: sim_velvia(),
                },
            )
            .unwrap_err();
        assert!(matches!(err, LibraryError::NotFound { .. }));
    }

    #[test]
    fn remove_deletes_file_and_returns_entry() {
        let (_tmp, path) = dir();
        let (mut lib, _) = SimLibrary::open(path.clone()).unwrap();
        let slug = lib
            .add(
                EntryEdit {
                    name: "Velvia".to_owned(),
                    description: None,
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
        let (mut lib, _) = SimLibrary::open(path).unwrap();
        for name in ["Velvia", "Acros", "Provia"] {
            lib.add(
                EntryEdit {
                    name: name.to_owned(),
                    description: None,
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
        let (mut lib, _) = SimLibrary::open(path.clone()).unwrap();
        let slug = lib
            .add(
                EntryEdit {
                    name: "Velvia".to_owned(),
                    description: None,
                    simulation: sim_velvia(),
                },
                cam(),
            )
            .unwrap();

        let entry = LibraryEntry {
            name: "Acros".to_owned(),
            description: None,
            source_camera: cam(),
            created: OffsetDateTime::now_utc(),
            modified: OffsetDateTime::now_utc(),
            simulation: sim_acros(),
        };
        let acros_slug = Slug::from_name("Acros").unwrap();
        write_atomic(&path, &acros_slug, &entry).unwrap();

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
        let (lib, report) = SimLibrary::open(path).unwrap();
        assert_eq!(lib.len(), 0);
        assert_eq!(report.skipped.len(), 1);
        assert!(matches!(
            report.skipped[0].reason,
            LibraryError::Parse { .. }
        ));
    }

    #[test]
    fn slug_mismatch_is_reported_and_skipped() {
        let (_tmp, path) = dir();
        let entry = LibraryEntry {
            name: "Acros".to_owned(),
            description: None,
            source_camera: cam(),
            created: OffsetDateTime::now_utc(),
            modified: OffsetDateTime::now_utc(),
            simulation: sim_acros(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        std::fs::write(path.join("velvia.json"), json).unwrap();

        let (lib, report) = SimLibrary::open(path).unwrap();
        assert_eq!(lib.len(), 0);
        assert_eq!(report.skipped.len(), 1);
        assert!(matches!(
            report.skipped[0].reason,
            LibraryError::SlugMismatch { .. }
        ));
    }
}
