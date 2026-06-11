pub mod atomic;
pub mod backup;
mod library;
pub mod render;
pub mod simulation;
pub mod slug;

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use fujicore::UsbId;
use image::DynamicImage;
use log::{debug, error, info, warn};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::workers::{
    ReqId, WorkerHandle,
    fs::{
        backup::{BackupLibrary, BackupLibraryError, BackupLibrarySnapshot},
        library::LibraryError,
        render::{RenderCache, RenderCacheKey, decode_image},
        simulation::{
            SimulationLibrary, SimulationLibraryEdit, SimulationLibraryError,
            SimulationLibrarySnapshot,
        },
        slug::Slug,
    },
};

const TICK: Duration = Duration::from_millis(100);
const RENDER_CACHE_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum FsError {
    #[error(transparent)]
    Library(#[from] LibraryError),
}

#[derive(Debug)]
pub enum FsCommand {
    LoadSimulationLibrary {
        req: ReqId,
    },
    #[allow(dead_code)]
    ReloadSimulationLibrary {
        req: ReqId,
    },
    #[allow(dead_code)]
    AddSimulation {
        req: ReqId,
        init: SimulationLibraryEdit,
        source_camera: UsbId,
    },
    #[allow(dead_code)]
    UpdateSimulation {
        req: ReqId,
        slug: Slug,
        edit: SimulationLibraryEdit,
    },
    #[allow(dead_code)]
    RemoveSimulation {
        req: ReqId,
        slug: Slug,
    },
    LoadBackupLibrary {
        req: ReqId,
    },
    #[allow(dead_code)]
    ReloadBackupLibrary {
        req: ReqId,
    },
    AddBackup {
        req: ReqId,
        name: String,
        source_camera: UsbId,
        blob: Vec<u8>,
    },
    RenameBackup {
        req: ReqId,
        slug: Slug,
        new_name: String,
    },
    RemoveBackup {
        req: ReqId,
        slug: Slug,
    },
    ReadBackupBlob {
        req: ReqId,
        slug: Slug,
    },
    ReadImage {
        req: ReqId,
        path: PathBuf,
    },
    LoadCachedRender {
        req: ReqId,
        key: RenderCacheKey,
    },
    StoreCachedRender {
        req: ReqId,
        key: RenderCacheKey,
        bytes: Vec<u8>,
    },
}

#[derive(Debug)]
pub enum FsEvent {
    SimulationLibraryLoaded {
        req: ReqId,
        snapshot: Arc<SimulationLibrarySnapshot>,
        skipped: usize,
    },
    SimulationLibraryReloaded {
        req: ReqId,
        snapshot: Arc<SimulationLibrarySnapshot>,
        skipped: usize,
    },
    SimulationEntryAdded {
        req: ReqId,
        slug: Slug,
        snapshot: Arc<SimulationLibrarySnapshot>,
    },
    SimulationEntryUpdated {
        req: ReqId,
        old_slug: Slug,
        new_slug: Slug,
        snapshot: Arc<SimulationLibrarySnapshot>,
    },
    SimulationEntryRemoved {
        req: ReqId,
        slug: Slug,
        snapshot: Arc<SimulationLibrarySnapshot>,
    },
    SimulationLibraryOpFailed {
        req: ReqId,
        error: Box<SimulationLibraryError>,
    },

    BackupLibraryLoaded {
        req: ReqId,
        snapshot: Arc<BackupLibrarySnapshot>,
        skipped: usize,
    },
    BackupLibraryReloaded {
        req: ReqId,
        snapshot: Arc<BackupLibrarySnapshot>,
        skipped: usize,
    },
    BackupEntryAdded {
        req: ReqId,
        slug: Slug,
        snapshot: Arc<BackupLibrarySnapshot>,
    },
    BackupEntryUpdated {
        req: ReqId,
        old_slug: Slug,
        new_slug: Slug,
        snapshot: Arc<BackupLibrarySnapshot>,
    },
    BackupEntryRemoved {
        req: ReqId,
        slug: Slug,
        snapshot: Arc<BackupLibrarySnapshot>,
    },
    BackupBlobRead {
        req: ReqId,
        slug: Slug,
        blob: Vec<u8>,
    },
    BackupLibraryOpFailed {
        req: ReqId,
        error: Box<BackupLibraryError>,
    },

    ImageRead {
        req: ReqId,
        path: PathBuf,
        image: Arc<[u8]>,
        sha256: [u8; 32],
    },
    ImageReadFailed {
        req: ReqId,
        path: PathBuf,
        error: Box<std::io::Error>,
    },

    RenderImageReady {
        req: ReqId,
        image: DynamicImage,
    },
    CachedRenderMiss {
        req: ReqId,
    },
    RenderImageFailed {
        req: ReqId,
    },

    Error(Box<FsError>),
}

pub type FsHandle = WorkerHandle<FsCommand>;

impl WorkerHandle<FsCommand> {
    pub fn spawn(
        simulation_dir: PathBuf,
        backup_dir: PathBuf,
        cache_dir: PathBuf,
        event_tx: Sender<FsEvent>,
    ) -> Self {
        let (command_tx, command_rx) = crossbeam_channel::unbounded();
        let inflight = Arc::new(AtomicUsize::new(0));
        let worker_inflight = Arc::clone(&inflight);
        let join = thread::Builder::new()
            .name("fujitui-fs".to_owned())
            .spawn(move || {
                FsWorker::run(
                    simulation_dir,
                    backup_dir,
                    cache_dir,
                    &command_rx,
                    &event_tx,
                    &worker_inflight,
                );
            })
            .expect("spawning fs worker");
        Self::new(command_tx, inflight, join)
    }
}

struct FsWorker {
    simulation: SimulationLibrary,
    backup: BackupLibrary,
    render_cache: Option<RenderCache>,
}

impl FsWorker {
    fn run(
        simulation_dir: PathBuf,
        backup_dir: PathBuf,
        cache_dir: PathBuf,
        command_rx: &Receiver<FsCommand>,
        event_tx: &Sender<FsEvent>,
        inflight: &AtomicUsize,
    ) {
        let simulation = match SimulationLibrary::open(simulation_dir) {
            Ok((lib, report)) => {
                for skipped in &report.skipped {
                    warn!(
                        "skipped simulation library file {}: {}",
                        skipped.path.display(),
                        skipped.reason
                    );
                }
                info!(
                    "simulation library opened ({} entries, {} skipped)",
                    lib.len(),
                    report.skipped.len()
                );
                lib
            }
            Err(e) => {
                error!("failed to open simulation library: {e}");
                let _ = event_tx.send(FsEvent::Error(Box::new(FsError::from(e))));
                return;
            }
        };
        let backup = match BackupLibrary::open(backup_dir) {
            Ok((lib, report)) => {
                for skipped in &report.skipped {
                    warn!(
                        "skipped backup library file {}: {}",
                        skipped.path.display(),
                        skipped.reason
                    );
                }
                info!(
                    "backup library opened ({} entries, {} skipped)",
                    lib.len(),
                    report.skipped.len()
                );
                lib
            }
            Err(e) => {
                error!("failed to open backup library: {e}");
                let _ = event_tx.send(FsEvent::Error(Box::new(FsError::from(e))));
                return;
            }
        };
        let render_cache = match RenderCache::open(cache_dir, RENDER_CACHE_MAX_BYTES) {
            Ok(cache) => Some(cache),
            Err(e) => {
                warn!("render cache disabled: {e}");
                None
            }
        };
        let mut worker = Self {
            simulation,
            backup,
            render_cache,
        };
        worker.event_loop(command_rx, event_tx, inflight);
    }

    fn event_loop(
        &mut self,
        command_rx: &Receiver<FsCommand>,
        event_tx: &Sender<FsEvent>,
        inflight: &AtomicUsize,
    ) {
        loop {
            match command_rx.recv_timeout(TICK) {
                Ok(cmd) => {
                    self.handle(cmd, event_tx);
                    inflight.fetch_sub(1, Ordering::Relaxed);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    debug!("fs worker command channel closed");
                    break;
                }
            }
        }
    }

    fn handle(&mut self, cmd: FsCommand, event_tx: &Sender<FsEvent>) {
        match cmd {
            FsCommand::LoadSimulationLibrary { req } => self.handle_simulation_load(req, event_tx),
            FsCommand::ReloadSimulationLibrary { req } => {
                self.handle_simulation_reload(req, event_tx);
            }
            FsCommand::AddSimulation {
                req,
                init,
                source_camera,
            } => self.handle_simulation_add(req, init, source_camera, event_tx),
            FsCommand::UpdateSimulation { req, slug, edit } => {
                self.handle_simulation_update(req, slug, edit, event_tx);
            }
            FsCommand::RemoveSimulation { req, slug } => {
                self.handle_simulation_remove(req, slug, event_tx);
            }

            FsCommand::LoadBackupLibrary { req } => self.handle_backup_load(req, event_tx),
            FsCommand::ReloadBackupLibrary { req } => self.handle_backup_reload(req, event_tx),
            FsCommand::AddBackup {
                req,
                name,
                source_camera,
                blob,
            } => self.handle_backup_add(req, name, source_camera, &blob, event_tx),
            FsCommand::RenameBackup {
                req,
                slug,
                new_name,
            } => self.handle_backup_rename(req, slug, new_name, event_tx),
            FsCommand::RemoveBackup { req, slug } => self.handle_backup_remove(req, slug, event_tx),
            FsCommand::ReadBackupBlob { req, slug } => self.handle_backup_read(req, slug, event_tx),
            FsCommand::ReadImage { req, path } => Self::handle_read_image(req, path, event_tx),
            FsCommand::LoadCachedRender { req, key } => {
                self.handle_load_cached_render(req, &key, event_tx);
            }
            FsCommand::StoreCachedRender { req, key, bytes } => {
                self.handle_store_cached_render(req, &key, &bytes, event_tx);
            }
        }
    }

    fn handle_load_cached_render(
        &mut self,
        req: ReqId,
        key: &RenderCacheKey,
        event_tx: &Sender<FsEvent>,
    ) {
        let hit = self.render_cache.as_mut().and_then(|cache| cache.get(key));
        let Some(bytes) = hit else {
            debug!("{req}: render cache miss");
            let _ = event_tx.send(FsEvent::CachedRenderMiss { req });
            return;
        };
        match decode_image(&bytes) {
            Ok(image) => {
                debug!("{req}: render cache hit");
                let _ = event_tx.send(FsEvent::RenderImageReady { req, image });
            }
            Err(e) => {
                error!("{req}: cached render failed to decode ({e}); re-rendering");
                let _ = event_tx.send(FsEvent::CachedRenderMiss { req });
            }
        }
    }

    fn handle_store_cached_render(
        &mut self,
        req: ReqId,
        key: &RenderCacheKey,
        bytes: &[u8],
        event_tx: &Sender<FsEvent>,
    ) {
        let image = match decode_image(bytes) {
            Ok(image) => image,
            Err(e) => {
                error!("{req}: failed to decode rendered image: {e}");
                let _ = event_tx.send(FsEvent::RenderImageFailed { req });
                return;
            }
        };
        if let Some(cache) = self.render_cache.as_mut()
            && let Err(e) = cache.put(key, bytes)
        {
            warn!("{req}: failed to cache render: {e}");
        }
        let _ = event_tx.send(FsEvent::RenderImageReady { req, image });
    }

    fn handle_read_image(req: ReqId, path: PathBuf, event_tx: &Sender<FsEvent>) {
        match std::fs::read(&path) {
            Ok(bytes) => {
                info!(
                    "{req}: read image {} ({} bytes)",
                    path.display(),
                    bytes.len()
                );
                let sha256 = Sha256::digest(&bytes).into();
                let _ = event_tx.send(FsEvent::ImageRead {
                    req,
                    path,
                    image: Arc::from(bytes),
                    sha256,
                });
            }
            Err(e) => {
                error!("{req}: read image {} failed: {e}", path.display());
                let _ = event_tx.send(FsEvent::ImageReadFailed {
                    req,
                    path,
                    error: Box::new(e),
                });
            }
        }
    }

    fn handle_simulation_load(&self, req: ReqId, event_tx: &Sender<FsEvent>) {
        let _ = event_tx.send(FsEvent::SimulationLibraryLoaded {
            req,
            snapshot: self.simulation.snapshot(),
            skipped: self.simulation.skipped(),
        });
    }

    fn handle_simulation_reload(&mut self, req: ReqId, event_tx: &Sender<FsEvent>) {
        match self.simulation.reload() {
            Ok(report) => {
                let _ = event_tx.send(FsEvent::SimulationLibraryReloaded {
                    req,
                    snapshot: self.simulation.snapshot(),
                    skipped: report.skipped.len(),
                });
            }
            Err(e) => {
                error!("{req}: reload simulation library failed: {e}");
                let _ = event_tx.send(FsEvent::SimulationLibraryOpFailed {
                    req,
                    error: Box::new(e),
                });
            }
        }
    }

    fn handle_simulation_add(
        &mut self,
        req: ReqId,
        init: SimulationLibraryEdit,
        source_camera: UsbId,
        event_tx: &Sender<FsEvent>,
    ) {
        match self.simulation.add(init, source_camera) {
            Ok(slug) => {
                let _ = event_tx.send(FsEvent::SimulationEntryAdded {
                    req,
                    slug,
                    snapshot: self.simulation.snapshot(),
                });
            }
            Err(e) => {
                error!("{req}: add simulation failed: {e}");
                let _ = event_tx.send(FsEvent::SimulationLibraryOpFailed {
                    req,
                    error: Box::new(e),
                });
            }
        }
    }

    fn handle_simulation_update(
        &mut self,
        req: ReqId,
        slug: Slug,
        edit: SimulationLibraryEdit,
        event_tx: &Sender<FsEvent>,
    ) {
        match self.simulation.update(&slug, edit) {
            Ok(new_slug) => {
                let _ = event_tx.send(FsEvent::SimulationEntryUpdated {
                    req,
                    old_slug: slug,
                    new_slug,
                    snapshot: self.simulation.snapshot(),
                });
            }
            Err(e) => {
                error!("{req}: update simulation {slug} failed: {e}");
                let _ = event_tx.send(FsEvent::SimulationLibraryOpFailed {
                    req,
                    error: Box::new(e),
                });
            }
        }
    }

    fn handle_simulation_remove(&mut self, req: ReqId, slug: Slug, event_tx: &Sender<FsEvent>) {
        match self.simulation.remove(&slug) {
            Ok(_entry) => {
                let _ = event_tx.send(FsEvent::SimulationEntryRemoved {
                    req,
                    slug,
                    snapshot: self.simulation.snapshot(),
                });
            }
            Err(e) => {
                error!("{req}: remove simulation {slug} failed: {e}");
                let _ = event_tx.send(FsEvent::SimulationLibraryOpFailed {
                    req,
                    error: Box::new(e),
                });
            }
        }
    }

    fn handle_backup_load(&self, req: ReqId, event_tx: &Sender<FsEvent>) {
        let _ = event_tx.send(FsEvent::BackupLibraryLoaded {
            req,
            snapshot: self.backup.snapshot(),
            skipped: self.backup.skipped(),
        });
    }

    fn handle_backup_reload(&mut self, req: ReqId, event_tx: &Sender<FsEvent>) {
        match self.backup.reload() {
            Ok(report) => {
                let _ = event_tx.send(FsEvent::BackupLibraryReloaded {
                    req,
                    snapshot: self.backup.snapshot(),
                    skipped: report.skipped.len(),
                });
            }
            Err(e) => {
                error!("{req}: reload backup library failed: {e}");
                let _ = event_tx.send(FsEvent::BackupLibraryOpFailed {
                    req,
                    error: Box::new(e),
                });
            }
        }
    }

    fn handle_backup_add(
        &mut self,
        req: ReqId,
        name: String,
        source_camera: UsbId,
        blob: &[u8],
        event_tx: &Sender<FsEvent>,
    ) {
        match self.backup.add(name, source_camera, blob) {
            Ok(slug) => {
                let _ = event_tx.send(FsEvent::BackupEntryAdded {
                    req,
                    slug,
                    snapshot: self.backup.snapshot(),
                });
            }
            Err(e) => {
                error!("{req}: add backup failed: {e}");
                let _ = event_tx.send(FsEvent::BackupLibraryOpFailed {
                    req,
                    error: Box::new(e),
                });
            }
        }
    }

    fn handle_backup_rename(
        &mut self,
        req: ReqId,
        slug: Slug,
        new_name: String,
        event_tx: &Sender<FsEvent>,
    ) {
        match self.backup.rename(&slug, new_name) {
            Ok(new_slug) => {
                let _ = event_tx.send(FsEvent::BackupEntryUpdated {
                    req,
                    old_slug: slug,
                    new_slug,
                    snapshot: self.backup.snapshot(),
                });
            }
            Err(e) => {
                error!("{req}: rename backup {slug} failed: {e}");
                let _ = event_tx.send(FsEvent::BackupLibraryOpFailed {
                    req,
                    error: Box::new(e),
                });
            }
        }
    }

    fn handle_backup_remove(&mut self, req: ReqId, slug: Slug, event_tx: &Sender<FsEvent>) {
        match self.backup.remove(&slug) {
            Ok(_entry) => {
                let _ = event_tx.send(FsEvent::BackupEntryRemoved {
                    req,
                    slug,
                    snapshot: self.backup.snapshot(),
                });
            }
            Err(e) => {
                error!("{req}: remove backup {slug} failed: {e}");
                let _ = event_tx.send(FsEvent::BackupLibraryOpFailed {
                    req,
                    error: Box::new(e),
                });
            }
        }
    }

    fn handle_backup_read(&self, req: ReqId, slug: Slug, event_tx: &Sender<FsEvent>) {
        match self.backup.read_blob(&slug) {
            Ok(blob) => {
                let _ = event_tx.send(FsEvent::BackupBlobRead { req, slug, blob });
            }
            Err(e) => {
                error!("{req}: read backup blob {slug} failed: {e}");
                let _ = event_tx.send(FsEvent::BackupLibraryOpFailed {
                    req,
                    error: Box::new(e),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crossbeam_channel::unbounded;

    use super::*;
    use crate::workers::ReqIdGen;

    fn spawn_worker() -> (FsHandle, Receiver<FsEvent>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let (tx, rx) = unbounded();
        let fs = FsHandle::spawn(
            dir.path().join("simulations"),
            dir.path().join("backups"),
            dir.path().join("renders"),
            tx,
        );
        (fs, rx, dir)
    }

    #[test]
    fn read_image_returns_file_bytes() {
        let (fs, rx, dir) = spawn_worker();
        let path = dir.path().join("DSCF0001.RAF");
        let payload = b"II*\x00 fake raf payload";
        std::fs::write(&path, payload).expect("write fixture");

        let req = ReqIdGen::new().next();
        fs.send(FsCommand::ReadImage {
            req,
            path: path.clone(),
        });

        match rx.recv_timeout(Duration::from_secs(5)).expect("event") {
            FsEvent::ImageRead {
                req: got,
                path: got_path,
                image,
                sha256: got_sha,
            } => {
                assert_eq!(got, req);
                assert_eq!(got_path, path);
                assert_eq!(&*image, &payload[..]);
                let expected_sha: [u8; 32] = Sha256::digest(payload).into();
                assert_eq!(got_sha, expected_sha);
            }
            other => panic!("expected ImageRead, got {other:?}"),
        }
    }

    #[test]
    fn read_image_missing_file_reports_failure() {
        let (fs, rx, dir) = spawn_worker();
        let req = ReqIdGen::new().next();
        fs.send(FsCommand::ReadImage {
            req,
            path: dir.path().join("missing.RAF"),
        });

        match rx.recv_timeout(Duration::from_secs(5)).expect("event") {
            FsEvent::ImageReadFailed { req: got, .. } => assert_eq!(got, req),
            other => panic!("expected ImageReadFailed, got {other:?}"),
        }
    }
}
