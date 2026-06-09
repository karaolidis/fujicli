use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use clap::{ArgAction, Parser};
use crossbeam_channel::{Receiver, Sender, after, select};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use directories::ProjectDirs;
use fujicore::CoreError;
use log::{debug, info, warn};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
};

use crate::{
    ui::{
        Action, Tab, actions,
        modals::{
            ModalEffect, ModalHandler, ModalOutcome, device::DevicePickerModal, fatal::FatalModal,
        },
        tabs::{AppCtx, Tabs},
        widgets::{Header, Loading, Status, StatusQueue},
    },
    workers::{
        ReqId, ReqIdGen,
        device::{
            DeviceCommand, DeviceEvent, DeviceHandle,
            usb::{self, DeviceCandidate},
        },
        fs::{
            FsCommand, FsError, FsEvent, FsHandle,
            backup::{BackupLibraryEntry, BackupLibraryError, BackupLibrarySnapshot},
            simulation::{
                SimulationLibraryEntry, SimulationLibraryError, SimulationLibrarySnapshot,
            },
            slug::Slug,
        },
        input,
    },
};

const TICK: Duration = Duration::from_millis(100);
const INPUT_CHANNEL_BOUND: usize = 16;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, author)]
pub struct Cli {
    /// Log extra debugging information (multiple instances increase verbosity)
    #[arg(long, short = 'v', action = ArgAction::Count)]
    pub verbose: u8,
}

pub struct App {
    pub ctx: AppCtx,

    pub tabs: Tabs,
    pub active_tab: Tab,
    pub modal: Option<Box<dyn ModalHandler>>,
    pub status: StatusQueue,

    pub started: Instant,

    quitting: bool,

    input_rx: Receiver<KeyEvent>,
    device_rx: Receiver<DeviceEvent>,
    device_tx: Sender<DeviceEvent>,
    fs_rx: Receiver<FsEvent>,
    #[allow(dead_code)]
    fs_tx: Sender<FsEvent>,
}

pub fn run(dirs: &ProjectDirs) -> anyhow::Result<()> {
    let candidates = usb::enumerate()?;
    info!("found {} supported camera(s)", candidates.len());

    let (input_tx, input_rx) = crossbeam_channel::bounded(INPUT_CHANNEL_BOUND);
    input::spawn(input_tx);

    let simulation_dir = dirs.data_dir().join("simulations");
    let backup_dir = dirs.data_dir().join("backups");
    info!("simulation library directory: {}", simulation_dir.display());
    info!("backup library directory: {}", backup_dir.display());

    let mut app = App::new(candidates, input_rx, simulation_dir, backup_dir);
    ratatui::run(|terminal| app.run(terminal))
}

impl App {
    fn new(
        candidates: Vec<DeviceCandidate>,
        input_rx: Receiver<KeyEvent>,
        simulation_dir: std::path::PathBuf,
        backup_dir: std::path::PathBuf,
    ) -> Self {
        let (device_tx, device_rx) = crossbeam_channel::unbounded();
        let (fs_tx, fs_rx) = crossbeam_channel::unbounded();

        let (modal, device): (Option<Box<dyn ModalHandler>>, _) = match candidates.len() {
            0 => {
                warn!("no supported camera connected");
                (Some(Box::new(FatalModal::no_device())), None)
            }
            1 => {
                let candidate = candidates.into_iter().next().expect("len > 0");
                info!(
                    "single camera detected: {} ({}, bus {}.{})",
                    candidate.name, candidate.usb_id, candidate.bus, candidate.address
                );
                let handle = DeviceHandle::spawn(candidate.device, device_tx.clone());
                (None, Some(handle))
            }
            n => {
                info!("{n} cameras detected");
                (Some(Box::new(DevicePickerModal::new(candidates))), None)
            }
        };

        let fs = FsHandle::spawn(simulation_dir, backup_dir, fs_tx.clone());
        let req = ReqIdGen::new();

        let sim_load_req = req.next();
        debug!("{sim_load_req}: loading simulation library");
        fs.send(FsCommand::LoadSimulationLibrary { req: sim_load_req });

        let backup_load_req = req.next();
        debug!("{backup_load_req}: loading backup library");
        fs.send(FsCommand::LoadBackupLibrary {
            req: backup_load_req,
        });

        Self {
            ctx: AppCtx {
                device,
                fs,
                req,
                device_snapshot: None,
                simulation_library_snapshot: SimulationLibrarySnapshot::empty(),
                backup_library_snapshot: BackupLibrarySnapshot::empty(),
            },
            tabs: Tabs::default(),
            active_tab: Tab::Simulation,
            modal,
            status: StatusQueue::default(),
            started: Instant::now(),
            quitting: false,
            input_rx,
            device_rx,
            device_tx,
            fs_rx,
            fs_tx,
        }
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        info!("entering event loop");
        while !self.quitting {
            terminal.draw(|frame| self.draw(frame))?;

            select! {
                recv(self.input_rx)  -> msg => self.handle_key(msg?),
                recv(self.device_rx) -> msg => self.handle_device_event(msg?),
                recv(self.fs_rx)     -> msg => self.handle_fs_event(msg?),
                recv(after(TICK))    -> _   => {}
            }
        }
        info!("exiting event loop");
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            info!("quit requested");
            self.quitting = true;
            return;
        }

        if let Some(modal) = self.modal.as_mut() {
            match modal.on_key(key) {
                ModalOutcome::Continue => {}
                ModalOutcome::Effect(effect) => {
                    self.modal = None;
                    self.apply_effect(effect);
                }
            }
            return;
        }

        if self.tabs.is_capturing_input(self.active_tab) {
            self.tabs.handle_key(&self.ctx, self.active_tab, key);
            return;
        }

        if let Some(action) = actions::map(key) {
            match action {
                Action::Quit => {
                    info!("quit requested");
                    self.quitting = true;
                }
                Action::NextTab => self.cycle_tab(true),
                Action::PrevTab => self.cycle_tab(false),
                Action::GotoTab(index) => {
                    if let Some(&target) = self.available_tabs().get(index) {
                        self.set_tab(target);
                    }
                }
            }
            return;
        }

        self.tabs.handle_key(&self.ctx, self.active_tab, key);
    }

    fn set_tab(&mut self, target: Tab) {
        self.active_tab = target;
        self.tabs.handle_activate(&self.ctx, target);
    }

    pub(crate) fn available_tabs(&self) -> Vec<Tab> {
        self.ctx
            .device_snapshot
            .as_ref()
            .map_or_else(Vec::new, |snap| Tab::available(snap.capabilities))
    }

    fn cycle_tab(&mut self, forward: bool) {
        let available = self.available_tabs();
        let len = available.len();
        if len == 0 {
            return;
        }
        let current = available
            .iter()
            .position(|t| *t == self.active_tab)
            .unwrap_or(0);
        let next = if forward {
            (current + 1) % len
        } else {
            (current + len - 1) % len
        };
        self.set_tab(available[next]);
    }

    fn ensure_active_tab_available(&mut self) {
        let available = self.available_tabs();
        if available.is_empty() || available.contains(&self.active_tab) {
            return;
        }
        if let Some(&first) = available.first() {
            self.set_tab(first);
        }
    }

    fn handle_device_event(&mut self, event: DeviceEvent) {
        match event {
            DeviceEvent::Connected(snap) => {
                info!(
                    "device connected: {} ({}, bus {}, battery {}%)",
                    snap.name, snap.usb_id, snap.bus_address, snap.battery
                );
                self.ctx.device_snapshot = Some(snap);
                self.ensure_active_tab_available();
                self.tabs.handle_device_connected(&self.ctx);
            }
            DeviceEvent::InfoUpdated(snap) => {
                debug!("device info: battery {}%", snap.battery);
                self.ctx.device_snapshot = Some(snap);
            }
            DeviceEvent::Disconnected => {
                info!("device disconnected");
                self.ctx.device_snapshot = None;
                self.ctx.device = None;
                self.modal = Some(Box::new(FatalModal::disconnect()));
                self.tabs.handle_device_disconnected(&self.ctx);
            }
            DeviceEvent::Error(error) => {
                warn!("device error: {error}");
                self.status.push_error(error.to_string());
            }
            DeviceEvent::SlotsEnumerated { req, slots } => {
                debug!("{req}: slots enumerated ({} slots)", slots.len());
                self.tabs.handle_slots_enumerated(&self.ctx, req, &slots);
            }
            DeviceEvent::SlotsEnumerationFailed { req, error } => {
                warn!("{req}: slot enumeration failed: {error}");
                self.tabs.handle_slots_enumeration_failed(&self.ctx, req);
                self.status
                    .push_error(format!("Slot enumeration failed: {error}"));
            }
            DeviceEvent::SlotFetched { req, slot, base } => {
                debug!("{req}: slot {slot} fetched");
                self.tabs.handle_slot_fetched(&self.ctx, slot, &base);
            }
            DeviceEvent::SlotFetchFailed { req, slot, error } => {
                warn!("{req}: fetch of {slot} failed: {error}");
                let error: Arc<CoreError> = error.into();
                self.tabs.handle_slot_fetch_failed(&self.ctx, slot, &error);
                self.status
                    .push_error(format!("Fetch of {slot} failed: {error}"));
            }
            DeviceEvent::SlotChanged { req, slot } => {
                info!("{req}: slot {slot} pushed");
                self.tabs.handle_slot_changed(&self.ctx, slot);
            }
            DeviceEvent::SlotPushFailed { req, slot, error } => {
                warn!("{req}: push to {slot} failed: {error}");
                let error: Arc<CoreError> = error.into();
                self.tabs.handle_slot_push_failed(&self.ctx, slot, &error);
                self.status
                    .push_error(format!("Push to {slot} failed: {error}"));
            }
            DeviceEvent::BackupExported { req, blob } => {
                info!("{req}: backup exported ({} bytes)", blob.len());
                self.tabs.handle_backup_exported(&self.ctx, req, &blob);
            }
            DeviceEvent::BackupExportFailed { req, error } => {
                warn!("{req}: backup export failed: {error}");
                let error: Arc<CoreError> = error.into();
                self.tabs
                    .handle_backup_export_failed(&self.ctx, req, &error);
                self.status
                    .push_error(format!("Backup export failed: {error}"));
            }
            DeviceEvent::BackupImported { req } => {
                info!("{req}: backup imported");
                self.tabs.handle_backup_imported(&self.ctx, req);
                self.status.push_info("Backup imported");
            }
            DeviceEvent::BackupImportFailed { req, error } => {
                warn!("{req}: backup import failed: {error}");
                let error: Arc<CoreError> = error.into();
                self.tabs
                    .handle_backup_import_failed(&self.ctx, req, &error);
                self.status
                    .push_error(format!("Backup import failed: {error}"));
            }
        }
    }

    fn handle_fs_event(&mut self, event: FsEvent) {
        match event {
            FsEvent::SimulationLibraryLoaded {
                req,
                snapshot,
                skipped,
            } => self.handle_simulation_library_loaded(req, snapshot, skipped),
            FsEvent::SimulationLibraryReloaded {
                req,
                snapshot,
                skipped,
            } => self.handle_simulation_library_reloaded(req, snapshot, skipped),
            FsEvent::SimulationEntryAdded {
                req,
                slug,
                entry,
                snapshot,
            } => self.handle_simulation_entry_added(req, slug, entry, snapshot),
            FsEvent::SimulationEntryUpdated {
                req,
                old_slug,
                new_slug,
                entry,
                snapshot,
            } => self.handle_simulation_entry_updated(req, old_slug, new_slug, entry, snapshot),
            FsEvent::SimulationEntryRemoved {
                req,
                slug,
                entry,
                snapshot,
            } => self.handle_simulation_entry_removed(req, slug, entry, snapshot),
            FsEvent::SimulationLibraryOpFailed { req, error } => {
                self.handle_simulation_library_op_failed(req, error);
            }
            FsEvent::BackupLibraryLoaded {
                req,
                snapshot,
                skipped,
            } => self.handle_backup_library_loaded(req, snapshot, skipped),
            FsEvent::BackupLibraryReloaded {
                req,
                snapshot,
                skipped,
            } => self.handle_backup_library_reloaded(req, snapshot, skipped),
            FsEvent::BackupEntryAdded {
                req,
                slug,
                entry,
                snapshot,
            } => self.handle_backup_entry_added(req, slug, entry, snapshot),
            FsEvent::BackupEntryUpdated {
                req,
                old_slug,
                new_slug,
                entry,
                snapshot,
            } => self.handle_backup_entry_updated(req, old_slug, new_slug, entry, snapshot),
            FsEvent::BackupEntryRemoved {
                req,
                slug,
                entry,
                snapshot,
            } => self.handle_backup_entry_removed(req, slug, entry, snapshot),
            FsEvent::BackupBlobRead { req, slug, blob } => {
                self.handle_backup_blob_read(req, slug, blob);
            }
            FsEvent::BackupLibraryOpFailed { req, error } => {
                self.handle_backup_library_op_failed(req, error);
            }
            FsEvent::Error(error) => self.handle_fs_error(error),
        }
    }

    fn handle_simulation_library_loaded(
        &mut self,
        req: ReqId,
        snapshot: Arc<SimulationLibrarySnapshot>,
        skipped: usize,
    ) {
        info!(
            "{req}: simulation library loaded ({} entries, {skipped} skipped)",
            snapshot.entries.len()
        );
        self.set_simulation_library(snapshot);
    }

    fn handle_simulation_library_reloaded(
        &mut self,
        req: ReqId,
        snapshot: Arc<SimulationLibrarySnapshot>,
        skipped: usize,
    ) {
        info!(
            "{req}: simulation library reloaded ({} entries, {skipped} skipped)",
            snapshot.entries.len()
        );
        self.set_simulation_library(snapshot);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_simulation_entry_added(
        &mut self,
        req: ReqId,
        slug: Slug,
        _entry: SimulationLibraryEntry,
        snapshot: Arc<SimulationLibrarySnapshot>,
    ) {
        info!("{req}: simulation entry added: {slug}");
        self.set_simulation_library(snapshot);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_simulation_entry_updated(
        &mut self,
        req: ReqId,
        old_slug: Slug,
        new_slug: Slug,
        _entry: SimulationLibraryEntry,
        snapshot: Arc<SimulationLibrarySnapshot>,
    ) {
        if old_slug == new_slug {
            info!("{req}: simulation entry updated: {new_slug}");
        } else {
            info!("{req}: simulation entry renamed: {old_slug} -> {new_slug}");
        }
        self.set_simulation_library(snapshot);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_simulation_entry_removed(
        &mut self,
        req: ReqId,
        slug: Slug,
        _entry: SimulationLibraryEntry,
        snapshot: Arc<SimulationLibrarySnapshot>,
    ) {
        info!("{req}: simulation entry removed: {slug}");
        self.set_simulation_library(snapshot);
    }

    fn set_simulation_library(&mut self, snapshot: Arc<SimulationLibrarySnapshot>) {
        self.ctx.simulation_library_snapshot = snapshot;
        self.tabs.handle_simulation_library_changed(&self.ctx);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_simulation_library_op_failed(
        &mut self,
        req: ReqId,
        error: Box<SimulationLibraryError>,
    ) {
        warn!("{req}: simulation library operation failed: {error}");
        self.status
            .push_error(format!("Simulation library operation failed: {error}"));
    }

    fn handle_backup_library_loaded(
        &mut self,
        req: ReqId,
        snapshot: Arc<BackupLibrarySnapshot>,
        skipped: usize,
    ) {
        info!(
            "{req}: backup library loaded ({} entries, {skipped} skipped)",
            snapshot.entries.len()
        );
        self.set_backup_library(snapshot);
    }

    fn handle_backup_library_reloaded(
        &mut self,
        req: ReqId,
        snapshot: Arc<BackupLibrarySnapshot>,
        skipped: usize,
    ) {
        info!(
            "{req}: backup library reloaded ({} entries, {skipped} skipped)",
            snapshot.entries.len()
        );
        self.set_backup_library(snapshot);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_backup_entry_added(
        &mut self,
        req: ReqId,
        slug: Slug,
        _entry: BackupLibraryEntry,
        snapshot: Arc<BackupLibrarySnapshot>,
    ) {
        info!("{req}: backup entry added: {slug}");
        self.tabs.handle_backup_entry_added(&self.ctx, &slug);
        self.set_backup_library(snapshot);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_backup_entry_updated(
        &mut self,
        req: ReqId,
        old_slug: Slug,
        new_slug: Slug,
        _entry: BackupLibraryEntry,
        snapshot: Arc<BackupLibrarySnapshot>,
    ) {
        if old_slug == new_slug {
            info!("{req}: backup entry updated: {new_slug}");
        } else {
            info!("{req}: backup entry renamed: {old_slug} -> {new_slug}");
        }
        self.tabs.handle_backup_entry_updated(&self.ctx, &new_slug);
        self.set_backup_library(snapshot);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_backup_entry_removed(
        &mut self,
        req: ReqId,
        slug: Slug,
        _entry: BackupLibraryEntry,
        snapshot: Arc<BackupLibrarySnapshot>,
    ) {
        info!("{req}: backup entry removed: {slug}");
        self.set_backup_library(snapshot);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_backup_blob_read(&mut self, req: ReqId, slug: Slug, blob: Vec<u8>) {
        info!(
            "{req}: backup blob read for {slug} ({} bytes); pushing to device",
            blob.len()
        );
        if let Some(device) = self.ctx.device.as_ref() {
            device.send(DeviceCommand::ImportBackup { req, blob });
        } else {
            warn!("{req}: backup blob read but no device connected");
            self.tabs.handle_backup_library_op_failed(&self.ctx, req);
            self.status
                .push_error("No camera connected to import backup");
        }
    }

    fn set_backup_library(&mut self, snapshot: Arc<BackupLibrarySnapshot>) {
        self.ctx.backup_library_snapshot = snapshot;
        self.tabs.handle_backup_library_changed(&self.ctx);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_backup_library_op_failed(&mut self, req: ReqId, error: Box<BackupLibraryError>) {
        warn!("{req}: backup library operation failed: {error}");
        self.tabs.handle_backup_library_op_failed(&self.ctx, req);
        self.status
            .push_error(format!("Backup library operation failed: {error}"));
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_fs_error(&mut self, error: Box<FsError>) {
        warn!("fs error: {error}");
        self.status.push_error(format!("Filesystem error: {error}"));
    }

    fn apply_effect(&mut self, effect: ModalEffect) {
        match effect {
            ModalEffect::Quit => self.handle_effect_quit(),
            ModalEffect::SelectDevice(candidate) => self.handle_effect_select_device(candidate),
        }
    }

    fn handle_effect_quit(&mut self) {
        info!("quit requested");
        self.quitting = true;
    }

    fn handle_effect_select_device(&mut self, candidate: DeviceCandidate) {
        info!(
            "device selected: {} ({}, bus {}.{})",
            candidate.name, candidate.usb_id, candidate.bus, candidate.address
        );
        let handle = DeviceHandle::spawn(candidate.device, self.device_tx.clone());
        self.ctx.device = Some(handle);
    }

    fn draw(&mut self, frame: &mut Frame) {
        let [header_area, body_area, status_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());

        if self.ctx.device_snapshot.is_none() {
            Loading::draw(frame, header_area.union(body_area));
        } else {
            Header::draw(self, frame, header_area);
            self.tabs.draw(&self.ctx, self.active_tab, frame, body_area);
        }
        Status::draw(self, frame, status_area);

        if let Some(modal) = &self.modal {
            modal.render(frame, frame.area());
        }
    }
}
