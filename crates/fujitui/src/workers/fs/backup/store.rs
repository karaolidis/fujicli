use std::{
    fs,
    path::{Path, PathBuf},
};

use fujicore::UsbId;
use log::info;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::workers::fs::{
    atomic,
    library::{Library, LibraryEntry, LibraryError, LibrarySnapshot},
    slug::Slug,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BackupLibraryEntry {
    pub name: String,
    pub source_camera: UsbId,
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub modified: OffsetDateTime,
}

impl LibraryEntry for BackupLibraryEntry {
    fn name(&self) -> &str {
        &self.name
    }

    fn load_one(dir: &Path, path: &Path) -> Result<Option<(Slug, Self)>, LibraryError> {
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            return Ok(None);
        }
        let (slug, entry) = BackupLibrary::read_meta(path)?;
        let blob = BackupLibrary::blob_path(dir, &slug);
        if !blob.exists() {
            return Err(LibraryError::MissingBlob {
                meta: path.to_path_buf(),
                blob,
            });
        }
        Ok(Some((slug, entry)))
    }
}

pub type BackupLibrary = Library<BackupLibraryEntry>;
pub type BackupLibraryError = LibraryError;
pub type BackupLibrarySnapshot = LibrarySnapshot<BackupLibraryEntry>;

impl Library<BackupLibraryEntry> {
    fn meta_path(dir: &Path, slug: &Slug) -> PathBuf {
        dir.join(format!("{}.json", slug.as_str()))
    }

    fn blob_path(dir: &Path, slug: &Slug) -> PathBuf {
        dir.join(format!("{}.bin", slug.as_str()))
    }

    pub fn add(
        &mut self,
        name: String,
        source_camera: UsbId,
        blob: &[u8],
    ) -> Result<Slug, BackupLibraryError> {
        let slug = Slug::try_from(name.as_str())?;
        if let Some(existing) = self.entries.get(&slug) {
            return Err(LibraryError::SlugConflict {
                slug,
                existing_name: existing.name.clone(),
            });
        }

        let now = OffsetDateTime::now_utc();
        let entry = BackupLibraryEntry {
            name,
            source_camera,
            created: now,
            modified: now,
        };

        atomic::write_bytes_atomic(&Self::blob_path(&self.dir, &slug), blob)?;
        if let Err(e) = atomic::write_json_atomic(&Self::meta_path(&self.dir, &slug), &entry) {
            let _ = fs::remove_file(Self::blob_path(&self.dir, &slug));
            return Err(e.into());
        }
        info!("added backup library entry {slug} ({} bytes)", blob.len());
        self.entries.insert(slug.clone(), entry);
        Ok(slug)
    }

    pub fn rename(&mut self, slug: &Slug, new_name: String) -> Result<Slug, BackupLibraryError> {
        let existing = self
            .entries
            .get(slug)
            .ok_or_else(|| LibraryError::NotFound { slug: slug.clone() })?;

        let new_slug = Slug::try_from(new_name.as_str())?;
        if new_slug != *slug
            && let Some(other) = self.entries.get(&new_slug)
        {
            return Err(LibraryError::SlugConflict {
                slug: new_slug,
                existing_name: other.name.clone(),
            });
        }

        let entry = BackupLibraryEntry {
            name: new_name,
            source_camera: existing.source_camera,
            created: existing.created,
            modified: OffsetDateTime::now_utc(),
        };

        atomic::write_json_atomic(&Self::meta_path(&self.dir, &new_slug), &entry)?;

        if new_slug == *slug {
            info!("updated backup library entry {slug}");
        } else {
            let old_blob = Self::blob_path(&self.dir, slug);
            let new_blob = Self::blob_path(&self.dir, &new_slug);
            if let Err(source) = fs::rename(&old_blob, &new_blob) {
                let _ = fs::remove_file(Self::meta_path(&self.dir, &new_slug));
                return Err(LibraryError::Io {
                    path: old_blob,
                    source,
                });
            }
            let old_meta = Self::meta_path(&self.dir, slug);
            if let Err(source) = fs::remove_file(&old_meta) {
                let _ = fs::rename(&new_blob, &old_blob);
                let _ = fs::remove_file(Self::meta_path(&self.dir, &new_slug));
                return Err(LibraryError::Io {
                    path: old_meta,
                    source,
                });
            }
            self.entries.remove(slug);
            info!("renamed backup library entry {slug} -> {new_slug}");
        }

        self.entries.insert(new_slug.clone(), entry);
        Ok(new_slug)
    }

    pub fn remove(&mut self, slug: &Slug) -> Result<BackupLibraryEntry, BackupLibraryError> {
        if !self.entries.contains_key(slug) {
            return Err(LibraryError::NotFound { slug: slug.clone() });
        }
        let blob = Self::blob_path(&self.dir, slug);
        let meta = Self::meta_path(&self.dir, slug);
        fs::remove_file(&blob).map_err(|source| LibraryError::Io { path: blob, source })?;
        fs::remove_file(&meta).map_err(|source| LibraryError::Io { path: meta, source })?;
        let entry = self
            .entries
            .remove(slug)
            .expect("entry presence checked above");
        info!("removed backup library entry {slug}");
        Ok(entry)
    }

    pub fn read_blob(&self, slug: &Slug) -> Result<Vec<u8>, BackupLibraryError> {
        if !self.entries.contains_key(slug) {
            return Err(LibraryError::NotFound { slug: slug.clone() });
        }
        let path = Self::blob_path(&self.dir, slug);
        fs::read(&path).map_err(|source| LibraryError::Io { path, source })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::*;

    fn cam() -> UsbId {
        UsbId {
            vendor: 0x04CB,
            product: 0x02FC,
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
        let nested = tmp.path().join("backups");
        let (lib, report) = BackupLibrary::open(nested.clone()).unwrap();
        assert!(nested.is_dir());
        assert_eq!(lib.len(), 0);
        assert!(report.skipped.is_empty());
    }

    #[test]
    fn add_writes_blob_and_metadata() {
        let (_tmp, path) = dir();
        let (mut lib, _) = BackupLibrary::open(path.clone()).unwrap();
        let blob = vec![1u8, 2, 3, 4, 5];
        let slug = lib.add("Pre-Trip".to_owned(), cam(), &blob).unwrap();
        assert_eq!(slug.as_str(), "pre-trip");
        assert!(path.join("pre-trip.json").exists());
        assert!(path.join("pre-trip.bin").exists());
    }

    #[test]
    fn read_blob_returns_added_bytes() {
        let (_tmp, path) = dir();
        let (mut lib, _) = BackupLibrary::open(path).unwrap();
        let blob = vec![9u8; 1024];
        let slug = lib.add("X".to_owned(), cam(), &blob).unwrap();
        assert_eq!(lib.read_blob(&slug).unwrap(), blob);
    }

    #[test]
    fn add_conflict_fails() {
        let (_tmp, path) = dir();
        let (mut lib, _) = BackupLibrary::open(path).unwrap();
        lib.add("Velvia".to_owned(), cam(), b"a").unwrap();
        let err = lib.add("VELVIA".to_owned(), cam(), b"b").unwrap_err();
        assert!(matches!(err, BackupLibraryError::SlugConflict { .. }));
    }

    #[test]
    fn rename_moves_both_files() {
        let (_tmp, path) = dir();
        let (mut lib, _) = BackupLibrary::open(path.clone()).unwrap();
        let slug = lib.add("Trip A".to_owned(), cam(), b"data").unwrap();
        let new_slug = lib.rename(&slug, "Trip B".to_owned()).unwrap();
        assert_eq!(new_slug.as_str(), "trip-b");
        assert!(!path.join("trip-a.json").exists());
        assert!(!path.join("trip-a.bin").exists());
        assert!(path.join("trip-b.json").exists());
        assert!(path.join("trip-b.bin").exists());
        assert_eq!(lib.read_blob(&new_slug).unwrap(), b"data");
    }

    #[test]
    fn rename_same_name_only_updates_metadata() {
        let (_tmp, path) = dir();
        let (mut lib, _) = BackupLibrary::open(path).unwrap();
        let slug = lib.add("Trip".to_owned(), cam(), b"data").unwrap();
        let created = lib.get(&slug).unwrap().created;
        std::thread::sleep(std::time::Duration::from_millis(10));
        let new_slug = lib.rename(&slug, "Trip".to_owned()).unwrap();
        assert_eq!(new_slug, slug);
        let entry = lib.get(&slug).unwrap();
        assert_eq!(entry.created, created);
        assert!(entry.modified > created);
    }

    #[test]
    fn rename_conflict_leaves_existing_intact() {
        let (_tmp, path) = dir();
        let (mut lib, _) = BackupLibrary::open(path).unwrap();
        let a = lib.add("Trip A".to_owned(), cam(), b"a").unwrap();
        lib.add("Trip B".to_owned(), cam(), b"b").unwrap();
        let err = lib.rename(&a, "Trip B".to_owned()).unwrap_err();
        assert!(matches!(err, BackupLibraryError::SlugConflict { .. }));
        assert_eq!(lib.get(&a).unwrap().name, "Trip A");
    }

    #[test]
    fn remove_deletes_both_files() {
        let (_tmp, path) = dir();
        let (mut lib, _) = BackupLibrary::open(path.clone()).unwrap();
        let slug = lib.add("Trip".to_owned(), cam(), b"data").unwrap();
        lib.remove(&slug).unwrap();
        assert!(!path.join("trip.json").exists());
        assert!(!path.join("trip.bin").exists());
        assert!(lib.get(&slug).is_none());
    }

    #[test]
    fn reload_picks_up_external_files() {
        let (_tmp, path) = dir();
        let (mut lib, _) = BackupLibrary::open(path.clone()).unwrap();
        assert_eq!(lib.len(), 0);

        let entry = BackupLibraryEntry {
            name: "External".to_owned(),
            source_camera: cam(),
            created: OffsetDateTime::now_utc(),
            modified: OffsetDateTime::now_utc(),
        };
        atomic::write_bytes_atomic(&path.join("external.bin"), b"abc").unwrap();
        atomic::write_json_atomic(&path.join("external.json"), &entry).unwrap();

        let report = lib.reload().unwrap();
        assert_eq!(report.loaded, 1);
        assert!(report.skipped.is_empty());
    }

    #[test]
    fn missing_blob_is_reported_and_skipped() {
        let (_tmp, path) = dir();
        let entry = BackupLibraryEntry {
            name: "Trip".to_owned(),
            source_camera: cam(),
            created: OffsetDateTime::now_utc(),
            modified: OffsetDateTime::now_utc(),
        };
        atomic::write_json_atomic(&path.join("trip.json"), &entry).unwrap();
        let (lib, report) = BackupLibrary::open(path).unwrap();
        assert_eq!(lib.len(), 0);
        assert!(matches!(
            report.skipped[0].reason,
            BackupLibraryError::MissingBlob { .. }
        ));
    }

    #[test]
    fn slug_mismatch_is_reported_and_skipped() {
        let (_tmp, path) = dir();
        let entry = BackupLibraryEntry {
            name: "Different".to_owned(),
            source_camera: cam(),
            created: OffsetDateTime::now_utc(),
            modified: OffsetDateTime::now_utc(),
        };
        atomic::write_bytes_atomic(&path.join("trip.bin"), b"abc").unwrap();
        atomic::write_json_atomic(&path.join("trip.json"), &entry).unwrap();
        let (lib, report) = BackupLibrary::open(path).unwrap();
        assert_eq!(lib.len(), 0);
        assert!(matches!(
            report.skipped[0].reason,
            BackupLibraryError::SlugMismatch { .. }
        ));
    }

    #[test]
    fn iter_is_sorted_by_slug() {
        let (_tmp, path) = dir();
        let (mut lib, _) = BackupLibrary::open(path).unwrap();
        for name in ["Velvia", "Acros", "Provia"] {
            lib.add(name.to_owned(), cam(), b"x").unwrap();
        }
        let order: Vec<_> = lib.iter().map(|(s, _)| s.as_str().to_owned()).collect();
        assert_eq!(order, vec!["acros", "provia", "velvia"]);
    }
}
