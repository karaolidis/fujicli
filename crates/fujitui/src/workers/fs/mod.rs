pub mod library;

use std::{
    path::PathBuf,
    sync::Arc,
    thread::{self, JoinHandle},
    time::Duration,
};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use fujicore::UsbId;
use log::{debug, error, info, warn};
use thiserror::Error;

use crate::workers::{
    ReqId,
    fs::library::{EntryEdit, LibraryEntry, LibraryError, LibrarySnapshot, SimLibrary, Slug},
};

const TICK: Duration = Duration::from_millis(100);

#[derive(Debug, Error)]
pub enum FsError {
    #[error(transparent)]
    Library(#[from] LibraryError),
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum FsCommand {
    LoadLibrary {
        req: ReqId,
    },
    ReloadLibrary {
        req: ReqId,
    },
    AddSim {
        req: ReqId,
        init: EntryEdit,
        source_camera: UsbId,
    },
    UpdateSim {
        req: ReqId,
        slug: Slug,
        edit: EntryEdit,
    },
    RemoveSim {
        req: ReqId,
        slug: Slug,
    },
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum FsEvent {
    LibraryLoaded {
        req: ReqId,
        snapshot: Arc<LibrarySnapshot>,
        skipped: usize,
    },
    LibraryReloaded {
        req: ReqId,
        snapshot: Arc<LibrarySnapshot>,
        skipped: usize,
    },
    LibraryEntryAdded {
        req: ReqId,
        slug: Slug,
        entry: LibraryEntry,
        snapshot: Arc<LibrarySnapshot>,
    },
    LibraryEntryUpdated {
        req: ReqId,
        old_slug: Slug,
        new_slug: Slug,
        entry: LibraryEntry,
        snapshot: Arc<LibrarySnapshot>,
    },
    LibraryEntryRemoved {
        req: ReqId,
        slug: Slug,
        entry: LibraryEntry,
        snapshot: Arc<LibrarySnapshot>,
    },
    LibraryOpFailed {
        req: ReqId,
        error: Box<LibraryError>,
    },
    Error(Box<FsError>),
}

pub struct FsHandle {
    command_tx: Option<Sender<FsCommand>>,
    join: Option<JoinHandle<()>>,
}

impl FsHandle {
    pub fn spawn(library_dir: PathBuf, event_tx: Sender<FsEvent>) -> Self {
        let (command_tx, command_rx) = crossbeam_channel::unbounded();
        let join = thread::Builder::new()
            .name("fujitui-fs".to_owned())
            .spawn(move || run(library_dir, &command_rx, &event_tx))
            .expect("spawning fs worker");
        Self {
            command_tx: Some(command_tx),
            join: Some(join),
        }
    }

    #[allow(dead_code)]
    pub fn send(&self, cmd: FsCommand) {
        if let Some(tx) = self.command_tx.as_ref() {
            let _ = tx.send(cmd);
        }
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

fn run(library_dir: PathBuf, command_rx: &Receiver<FsCommand>, event_tx: &Sender<FsEvent>) {
    let mut library = match SimLibrary::open(library_dir) {
        Ok((lib, report)) => {
            for skipped in &report.skipped {
                warn!(
                    "skipped library file {}: {}",
                    skipped.path.display(),
                    skipped.reason
                );
            }
            info!(
                "library opened ({} entries, {} skipped)",
                lib.len(),
                report.skipped.len()
            );
            lib
        }
        Err(e) => {
            error!("failed to open library: {e}");
            let _ = event_tx.send(FsEvent::Error(Box::new(FsError::from(e))));
            return;
        }
    };

    loop {
        match command_rx.recv_timeout(TICK) {
            Ok(cmd) => handle(&mut library, cmd, event_tx),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                debug!("fs worker command channel closed");
                break;
            }
        }
    }
}

fn handle(library: &mut SimLibrary, cmd: FsCommand, event_tx: &Sender<FsEvent>) {
    match cmd {
        FsCommand::LoadLibrary { req } => handle_load(library, req, event_tx),
        FsCommand::ReloadLibrary { req } => handle_reload(library, req, event_tx),
        FsCommand::AddSim {
            req,
            init,
            source_camera,
        } => handle_add(library, req, init, source_camera, event_tx),
        FsCommand::UpdateSim { req, slug, edit } => {
            handle_update(library, req, slug, edit, event_tx);
        }
        FsCommand::RemoveSim { req, slug } => handle_remove(library, req, slug, event_tx),
    }
}

fn handle_load(library: &SimLibrary, req: ReqId, event_tx: &Sender<FsEvent>) {
    let _ = event_tx.send(FsEvent::LibraryLoaded {
        req,
        snapshot: library.snapshot(),
        skipped: 0,
    });
}

fn handle_reload(library: &mut SimLibrary, req: ReqId, event_tx: &Sender<FsEvent>) {
    match library.reload() {
        Ok(report) => {
            let _ = event_tx.send(FsEvent::LibraryReloaded {
                req,
                snapshot: library.snapshot(),
                skipped: report.skipped.len(),
            });
        }
        Err(e) => {
            error!("{req}: reload library failed: {e}");
            let _ = event_tx.send(FsEvent::LibraryOpFailed {
                req,
                error: Box::new(e),
            });
        }
    }
}

fn handle_add(
    library: &mut SimLibrary,
    req: ReqId,
    init: EntryEdit,
    source_camera: UsbId,
    event_tx: &Sender<FsEvent>,
) {
    match library.add(init, source_camera) {
        Ok(slug) => {
            let entry = library.get(&slug).expect("just added").clone();
            let _ = event_tx.send(FsEvent::LibraryEntryAdded {
                req,
                slug,
                entry,
                snapshot: library.snapshot(),
            });
        }
        Err(e) => {
            error!("{req}: add sim failed: {e}");
            let _ = event_tx.send(FsEvent::LibraryOpFailed {
                req,
                error: Box::new(e),
            });
        }
    }
}

fn handle_update(
    library: &mut SimLibrary,
    req: ReqId,
    slug: Slug,
    edit: EntryEdit,
    event_tx: &Sender<FsEvent>,
) {
    match library.update(&slug, edit) {
        Ok(new_slug) => {
            let entry = library.get(&new_slug).expect("just updated").clone();
            let _ = event_tx.send(FsEvent::LibraryEntryUpdated {
                req,
                old_slug: slug,
                new_slug,
                entry,
                snapshot: library.snapshot(),
            });
        }
        Err(e) => {
            error!("{req}: update sim {slug} failed: {e}");
            let _ = event_tx.send(FsEvent::LibraryOpFailed {
                req,
                error: Box::new(e),
            });
        }
    }
}

fn handle_remove(library: &mut SimLibrary, req: ReqId, slug: Slug, event_tx: &Sender<FsEvent>) {
    match library.remove(&slug) {
        Ok(entry) => {
            let _ = event_tx.send(FsEvent::LibraryEntryRemoved {
                req,
                slug,
                entry,
                snapshot: library.snapshot(),
            });
        }
        Err(e) => {
            error!("{req}: remove sim {slug} failed: {e}");
            let _ = event_tx.send(FsEvent::LibraryOpFailed {
                req,
                error: Box::new(e),
            });
        }
    }
}
