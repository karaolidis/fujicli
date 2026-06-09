pub mod atomic;
pub mod backup;
pub mod simulation;
pub mod slug;

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use fujicore::UsbId;
use log::{debug, error, info, warn};
use thiserror::Error;

use crate::workers::{
    ReqId,
    fs::{
        backup::{BackupLibrary, BackupLibraryEntry, BackupLibraryError, BackupLibrarySnapshot},
        simulation::{
            SimulationLibrary, SimulationLibraryEdit, SimulationLibraryEntry,
            SimulationLibraryError, SimulationLibrarySnapshot,
        },
        slug::Slug,
    },
};

const TICK: Duration = Duration::from_millis(100);

#[derive(Debug, Error)]
pub enum FsError {
    #[error(transparent)]
    SimulationLibrary(#[from] SimulationLibraryError),

    #[error(transparent)]
    BackupLibrary(#[from] BackupLibraryError),
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
        entry: SimulationLibraryEntry,
        snapshot: Arc<SimulationLibrarySnapshot>,
    },
    SimulationEntryUpdated {
        req: ReqId,
        old_slug: Slug,
        new_slug: Slug,
        entry: SimulationLibraryEntry,
        snapshot: Arc<SimulationLibrarySnapshot>,
    },
    SimulationEntryRemoved {
        req: ReqId,
        slug: Slug,
        entry: SimulationLibraryEntry,
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
        entry: BackupLibraryEntry,
        snapshot: Arc<BackupLibrarySnapshot>,
    },
    BackupEntryUpdated {
        req: ReqId,
        old_slug: Slug,
        new_slug: Slug,
        entry: BackupLibraryEntry,
        snapshot: Arc<BackupLibrarySnapshot>,
    },
    BackupEntryRemoved {
        req: ReqId,
        slug: Slug,
        entry: BackupLibraryEntry,
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

    Error(Box<FsError>),
}

pub struct FsHandle {
    command_tx: Option<Sender<FsCommand>>,
    inflight: Arc<AtomicUsize>,
    join: Option<JoinHandle<()>>,
}

impl FsHandle {
    pub fn spawn(simulation_dir: PathBuf, backup_dir: PathBuf, event_tx: Sender<FsEvent>) -> Self {
        let (command_tx, command_rx) = crossbeam_channel::unbounded();
        let inflight = Arc::new(AtomicUsize::new(0));
        let worker_inflight = Arc::clone(&inflight);
        let join = thread::Builder::new()
            .name("fujitui-fs".to_owned())
            .spawn(move || {
                FsWorker::run(
                    simulation_dir,
                    backup_dir,
                    &command_rx,
                    &event_tx,
                    &worker_inflight,
                );
            })
            .expect("spawning fs worker");
        Self {
            command_tx: Some(command_tx),
            inflight,
            join: Some(join),
        }
    }

    #[allow(dead_code)]
    pub fn send(&self, cmd: FsCommand) {
        if let Some(tx) = self.command_tx.as_ref() {
            self.inflight.fetch_add(1, Ordering::Relaxed);
            if tx.send(cmd).is_err() {
                self.inflight.fetch_sub(1, Ordering::Relaxed);
            }
        }
    }

    pub fn is_busy(&self) -> bool {
        self.inflight.load(Ordering::Relaxed) > 0
    }
}

impl Drop for FsHandle {
    fn drop(&mut self) {
        drop(self.command_tx.take());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

struct FsWorker {
    simulation: SimulationLibrary,
    backup: BackupLibrary,
}

impl FsWorker {
    fn run(
        simulation_dir: PathBuf,
        backup_dir: PathBuf,
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
        let mut worker = Self { simulation, backup };
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
                let entry = self.simulation.get(&slug).expect("just added").clone();
                let _ = event_tx.send(FsEvent::SimulationEntryAdded {
                    req,
                    slug,
                    entry,
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
                let entry = self
                    .simulation
                    .get(&new_slug)
                    .expect("just updated")
                    .clone();
                let _ = event_tx.send(FsEvent::SimulationEntryUpdated {
                    req,
                    old_slug: slug,
                    new_slug,
                    entry,
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
            Ok(entry) => {
                let _ = event_tx.send(FsEvent::SimulationEntryRemoved {
                    req,
                    slug,
                    entry,
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
                let entry = self.backup.get(&slug).expect("just added").clone();
                let _ = event_tx.send(FsEvent::BackupEntryAdded {
                    req,
                    slug,
                    entry,
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
                let entry = self.backup.get(&new_slug).expect("just renamed").clone();
                let _ = event_tx.send(FsEvent::BackupEntryUpdated {
                    req,
                    old_slug: slug,
                    new_slug,
                    entry,
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
            Ok(entry) => {
                let _ = event_tx.send(FsEvent::BackupEntryRemoved {
                    req,
                    slug,
                    entry,
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
