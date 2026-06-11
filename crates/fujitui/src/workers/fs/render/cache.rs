use std::{collections::BTreeMap, fmt::Write as _, fs, path::PathBuf};

use fujicore::generated::renders::RenderBase;
use image::{DynamicImage, ImageDecoder, ImageReader};
use log::warn;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::OffsetDateTime;

use crate::workers::fs::atomic::{AtomicError, write_bytes_atomic, write_json_atomic};

const INDEX_FILE: &str = "index.json";

#[derive(Debug, Error)]
pub enum RenderCacheError {
    #[error("cache i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Atomic(#[from] AtomicError),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RenderCacheKey(String);

impl RenderCacheKey {
    #[must_use]
    pub fn compute(raf_sha256: &[u8; 32], base: &RenderBase, draft: bool) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(raf_sha256);
        let encoded = serde_json::to_vec(base).expect("RenderBase always serializes");
        hasher.update(&encoded);
        hasher.update([u8::from(draft)]);
        Self(hex(&hasher.finalize()))
    }
}

pub fn decode_image(bytes: &[u8]) -> image::ImageResult<DynamicImage> {
    let mut decoder = ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()?
        .into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

#[derive(Serialize, Deserialize)]
struct Entry {
    size: u64,
    #[serde(with = "time::serde::rfc3339")]
    last_accessed: OffsetDateTime,
}

pub struct RenderCache {
    dir: PathBuf,
    entries: BTreeMap<RenderCacheKey, Entry>,
    max_size: u64,
    size: u64,
}

impl RenderCache {
    pub fn open(dir: PathBuf, max_bytes: u64) -> Result<Self, RenderCacheError> {
        fs::create_dir_all(&dir).map_err(|source| RenderCacheError::Io {
            path: dir.clone(),
            source,
        })?;

        let mut entries = BTreeMap::new();
        let index_path = dir.join(INDEX_FILE);
        if index_path.exists() {
            match fs::read(&index_path)
                .map(|raw| serde_json::from_slice::<BTreeMap<RenderCacheKey, Entry>>(&raw))
            {
                Ok(Ok(loaded)) => {
                    for (hash, entry) in loaded {
                        if dir.join(&hash.0).is_file() {
                            entries.insert(hash, entry);
                        }
                    }
                }
                Ok(Err(e)) => warn!("render cache index is corrupt ({e}); starting fresh"),
                Err(e) => warn!("render cache index unreadable ({e}); starting fresh"),
            }
        }

        let total_bytes = entries.values().map(|e| e.size).sum();
        let cache = Self {
            dir,
            max_size: max_bytes,
            entries,
            size: total_bytes,
        };
        cache.remove_orphans();
        Ok(cache)
    }

    pub fn get(&mut self, key: &RenderCacheKey) -> Option<Vec<u8>> {
        if !self.entries.contains_key(key) {
            return None;
        }
        let Ok(bytes) = fs::read(self.file_path(key)) else {
            self.drop_entry(key);
            return None;
        };
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_accessed = OffsetDateTime::now_utc();
        }
        Some(bytes)
    }

    pub fn put(&mut self, key: &RenderCacheKey, bytes: &[u8]) -> Result<(), RenderCacheError> {
        write_bytes_atomic(&self.file_path(key), bytes)?;
        let size = bytes.len() as u64;
        if let Some(old) = self.entries.insert(
            key.clone(),
            Entry {
                size,
                last_accessed: OffsetDateTime::now_utc(),
            },
        ) {
            self.size -= old.size;
        }
        self.size += size;
        self.evict();
        self.persist_index()
    }

    fn evict(&mut self) {
        if self.size <= self.max_size {
            return;
        }
        let mut order: Vec<(RenderCacheKey, OffsetDateTime)> = self
            .entries
            .iter()
            .map(|(k, e)| (k.clone(), e.last_accessed))
            .collect();
        order.sort_by_key(|(_, t)| *t);
        for (key, _) in order {
            if self.size <= self.max_size {
                break;
            }
            self.drop_entry(&key);
        }
    }

    fn drop_entry(&mut self, key: &RenderCacheKey) {
        if let Some(entry) = self.entries.remove(key) {
            self.size -= entry.size;
        }
        let _ = fs::remove_file(self.file_path(key));
    }

    fn remove_orphans(&self) {
        let Ok(read_dir) = fs::read_dir(&self.dir) else {
            return;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if name != INDEX_FILE && !self.entries.contains_key(&RenderCacheKey(name.to_owned())) {
                let _ = fs::remove_file(path);
            }
        }
    }

    fn persist_index(&self) -> Result<(), RenderCacheError> {
        write_json_atomic(&self.dir.join(INDEX_FILE), &self.entries)?;
        Ok(())
    }

    fn file_path(&self, key: &RenderCacheKey) -> PathBuf {
        self.dir.join(&key.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache(max_bytes: u64) -> (RenderCache, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let cache = RenderCache::open(dir.path().join("renders"), max_bytes).unwrap();
        (cache, dir)
    }

    fn key(stem: &str) -> RenderCacheKey {
        RenderCacheKey(stem.to_owned())
    }

    #[test]
    fn compute_is_deterministic_and_input_sensitive() {
        let raf = Sha256::digest(b"raf-bytes").into();
        let base = RenderBase::default();
        let a = RenderCacheKey::compute(&raf, &base, true);
        assert_eq!(a, RenderCacheKey::compute(&raf, &base, true));
        assert_ne!(a, RenderCacheKey::compute(&raf, &base, false));
        assert_ne!(
            a,
            RenderCacheKey::compute(&Sha256::digest(b"other").into(), &base, true)
        );
        let other_base = RenderBase {
            film_simulation: Some(fujicore::generated::options::FilmSimulation::Velvia),
            ..Default::default()
        };
        assert_ne!(a, RenderCacheKey::compute(&raf, &other_base, true));
    }

    #[test]
    fn put_then_get_round_trips() {
        let (mut cache, _dir) = cache(1 << 20);
        let k = key("aa");
        assert!(cache.get(&k).is_none());
        cache.put(&k, b"jpeg-bytes").unwrap();
        assert_eq!(cache.get(&k).as_deref(), Some(b"jpeg-bytes".as_slice()));
    }

    #[test]
    fn eviction_drops_least_recently_used() {
        let (mut cache, _dir) = cache(20);
        cache.put(&key("old"), &[0u8; 8]).unwrap();
        cache.put(&key("new"), &[0u8; 8]).unwrap();
        cache.entries.get_mut(&key("old")).unwrap().last_accessed =
            OffsetDateTime::from_unix_timestamp(0).unwrap();
        cache.put(&key("third"), &[0u8; 8]).unwrap();
        assert!(cache.get(&key("old")).is_none());
        assert!(cache.get(&key("new")).is_some());
        assert!(cache.get(&key("third")).is_some());
        assert!(cache.size <= 20);
    }

    #[test]
    fn reopen_restores_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("renders");
        {
            let mut cache = RenderCache::open(path.clone(), 1 << 20).unwrap();
            cache.put(&key("persist"), b"bytes").unwrap();
        }
        let mut reopened = RenderCache::open(path, 1 << 20).unwrap();
        assert_eq!(
            reopened.get(&key("persist")).as_deref(),
            Some(b"bytes".as_slice())
        );
    }

    #[test]
    fn missing_file_is_a_miss_and_drops_the_entry() {
        let (mut cache, _dir) = cache(1 << 20);
        let k = key("gone");
        cache.put(&k, b"bytes").unwrap();
        fs::remove_file(cache.file_path(&k)).unwrap();
        assert!(cache.get(&k).is_none());
        assert!(!cache.entries.contains_key(&k));
    }

    #[test]
    fn reopen_removes_orphan_files_not_in_index() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("renders");
        let cache = RenderCache::open(path.clone(), 1 << 20).unwrap();
        drop(cache);
        let orphan = path.join("orphan.jpg");
        fs::write(&orphan, b"stale").unwrap();
        let _cache = RenderCache::open(path, 1 << 20).unwrap();
        assert!(!orphan.exists());
    }
}
