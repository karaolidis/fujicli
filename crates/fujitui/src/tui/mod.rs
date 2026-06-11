use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use clap::{ArgAction, Parser};
use crossbeam_channel::{Receiver, Sender, after, select};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use directories::ProjectDirs;
use fujicore::CoreError;
use log::{debug, error, info, warn};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
};
use ratatui_image::{
    picker::Picker,
    thread::{ResizeRequest, ResizeResponse},
};

use crate::{
    ui::{
        Action, Tab, actions,
        modals::{
            ModalEffect, ModalHandler, ModalOutcome, device::DevicePickerModal, fatal::FatalModal,
            help::HelpModal,
        },
        tabs::{AppCtx, Tabs},
        widgets::{Header, Loading, SPINNER_INTERVAL, Status, StatusQueue},
    },
    workers::{
        ReqId, ReqIdGen,
        device::{
            DeviceCommand, DeviceEvent, DeviceHandle,
            usb::{self, DeviceCandidate},
        },
        fs::{
            FsCommand, FsError, FsEvent, FsHandle,
            backup::{BackupLibraryError, BackupLibrarySnapshot},
            simulation::{SimulationLibraryError, SimulationLibrarySnapshot},
            slug::Slug,
        },
        image::ImageWorker,
        input::InputWorker,
    },
};

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
    resize_rx: Receiver<ResizeResponse>,
}

pub fn run(dirs: &ProjectDirs) -> anyhow::Result<()> {
    let candidates = usb::enumerate()?;
    info!("found {} supported camera(s)", candidates.len());

    let picker = Picker::from_query_stdio().unwrap_or_else(|e| {
        warn!("graphics protocol query failed ({e}); falling back to halfblocks");
        Picker::halfblocks()
    });

    let (input_tx, input_rx) = crossbeam_channel::bounded(INPUT_CHANNEL_BOUND);
    InputWorker::spawn(input_tx);

    let (resize_tx, resize_rx) = ImageWorker::spawn();

    let simulation_dir = dirs.data_dir().join("simulations");
    let backup_dir = dirs.data_dir().join("backups");
    info!("simulation library directory: {}", simulation_dir.display());
    info!("backup library directory: {}", backup_dir.display());

    let mut app = App::new(
        candidates,
        input_rx,
        picker,
        resize_tx,
        resize_rx,
        simulation_dir,
        backup_dir,
    );
    ratatui::run(|terminal| app.run(terminal))
}

impl App {
    #[allow(clippy::too_many_arguments)]
    fn new(
        candidates: Vec<DeviceCandidate>,
        input_rx: Receiver<KeyEvent>,
        image_picker: Picker,
        resize_tx: std::sync::mpsc::Sender<ResizeRequest>,
        resize_rx: Receiver<ResizeResponse>,
        simulation_dir: std::path::PathBuf,
        backup_dir: std::path::PathBuf,
    ) -> Self {
        let (device_tx, device_rx) = crossbeam_channel::unbounded();
        let (fs_tx, fs_rx) = crossbeam_channel::unbounded();

        let (modal, device): (Option<Box<dyn ModalHandler>>, _) = match candidates.len() {
            0 => {
                error!("no supported camera connected");
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

        let fs = FsHandle::spawn(simulation_dir, backup_dir, fs_tx);
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
                image_picker,
                resize_tx,
                overlay: false,
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
            resize_rx,
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
                recv(self.resize_rx) -> msg => {
                    if let Ok(response) = msg {
                        self.tabs.rendering.apply_resized(response);
                    }
                }
                recv(after(self.next_frame_in())) -> _ => {}
            }

            for update in self.tabs.take_status() {
                self.status.push(update);
            }
        }
        info!("exiting event loop");
        Ok(())
    }

    fn next_frame_in(&self) -> Duration {
        let interval = SPINNER_INTERVAL.as_millis();
        let phase = self.started.elapsed().as_millis() % interval;
        Duration::from_millis(u64::try_from(interval - phase).unwrap_or(0))
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
                ModalOutcome::Dismiss => self.modal = None,
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
                Action::Help => {
                    self.modal = Some(Box::new(HelpModal::new(
                        self.active_tab.label(),
                        self.tabs.keybinds(self.active_tab),
                    )));
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
                self.tabs
                    .broadcast(|tab| tab.on_device_connected(&self.ctx));
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
                self.tabs
                    .broadcast(|tab| tab.on_device_disconnected(&self.ctx));
            }
            DeviceEvent::Error(error) => {
                error!("device error: {error}");
                self.status.push_error(error.to_string());
            }
            DeviceEvent::SlotFetched { req, slot, base } => {
                debug!("{req}: slot {slot} fetched");
                self.tabs
                    .broadcast(|tab| tab.on_simulation_slot_fetched(&self.ctx, slot, &base));
            }
            DeviceEvent::SlotFetchFailed { req, slot, error } => {
                error!("{req}: fetch of {slot} failed: {error}");
                let error: Arc<CoreError> = error.into();
                self.tabs
                    .broadcast(|tab| tab.on_simulation_slot_fetch_failed(&self.ctx, slot, &error));
                self.status
                    .push_error(format!("Fetch of {slot} failed: {error}"));
            }
            DeviceEvent::SlotChanged { req, slot } => {
                info!("{req}: slot {slot} pushed");
                self.tabs
                    .broadcast(|tab| tab.on_simulation_slot_changed(&self.ctx, slot));
            }
            DeviceEvent::SlotPushFailed { req, slot, error } => {
                error!("{req}: push to {slot} failed: {error}");
                let error: Arc<CoreError> = error.into();
                self.tabs
                    .broadcast(|tab| tab.on_simulation_slot_push_failed(&self.ctx, slot, &error));
                self.status
                    .push_error(format!("Push to {slot} failed: {error}"));
            }
            DeviceEvent::ImageLoaded { req, profile } => {
                info!("{req}: image loaded; conversion profile read");
                self.tabs
                    .broadcast(|tab| tab.on_image_loaded(&self.ctx, req, &profile));
            }
            DeviceEvent::ImageLoadFailed { req, error } => {
                error!("{req}: image load failed: {error}");
                self.tabs
                    .broadcast(|tab| tab.on_image_load_failed(&self.ctx, req));
                self.status
                    .push_error(format!("Failed to load image: {error}"));
            }
            DeviceEvent::RenderStarted { req } => {
                debug!("{req}: render started");
            }
            DeviceEvent::RenderDone { req, image } => {
                info!("{req}: render done");
                self.tabs.rendering.on_render_done(&self.ctx, req, image);
            }
            DeviceEvent::RenderFailed { req, error } => {
                error!("{req}: render failed: {error}");
                self.tabs.rendering.on_render_failed(req);
                self.status.push_error(format!("Render failed: {error}"));
            }
            DeviceEvent::BackupExported { req, blob } => {
                info!("{req}: backup exported ({} bytes)", blob.len());
                self.tabs
                    .broadcast(|tab| tab.on_backup_exported(&self.ctx, req, &blob));
            }
            DeviceEvent::BackupExportFailed { req, error } => {
                error!("{req}: backup export failed: {error}");
                let error: Arc<CoreError> = error.into();
                self.tabs
                    .broadcast(|tab| tab.on_backup_export_failed(&self.ctx, req, &error));
                self.status
                    .push_error(format!("Backup export failed: {error}"));
            }
            DeviceEvent::BackupImported { req } => {
                info!("{req}: backup imported");
                self.tabs
                    .broadcast(|tab| tab.on_backup_imported(&self.ctx, req));
                self.status.push_info("Backup imported");
            }
            DeviceEvent::BackupImportFailed { req, error } => {
                error!("{req}: backup import failed: {error}");
                let error: Arc<CoreError> = error.into();
                self.tabs
                    .broadcast(|tab| tab.on_backup_import_failed(&self.ctx, req, &error));
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
                snapshot,
            } => self.handle_simulation_library_entry_added(req, slug, snapshot),
            FsEvent::SimulationEntryUpdated {
                req,
                old_slug,
                new_slug,
                snapshot,
            } => self.handle_simulation_library_entry_updated(req, old_slug, new_slug, snapshot),
            FsEvent::SimulationEntryRemoved {
                req,
                slug,
                snapshot,
            } => self.handle_simulation_library_entry_removed(req, slug, snapshot),
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
                snapshot,
            } => self.handle_backup_library_entry_added(req, slug, snapshot),
            FsEvent::BackupEntryUpdated {
                req,
                old_slug,
                new_slug,
                snapshot,
            } => self.handle_backup_library_entry_updated(req, old_slug, new_slug, snapshot),
            FsEvent::BackupEntryRemoved {
                req,
                slug,
                snapshot,
            } => self.handle_backup_library_entry_removed(req, slug, snapshot),
            FsEvent::BackupBlobRead { req, slug, blob } => {
                self.handle_backup_blob_read(req, slug, blob);
            }
            FsEvent::BackupLibraryOpFailed { req, error } => {
                self.handle_backup_library_op_failed(req, error);
            }
            FsEvent::ImageRead { req, path, image } => {
                info!(
                    "{req}: image read: {} ({} bytes)",
                    path.display(),
                    image.len()
                );
                self.tabs
                    .broadcast(|tab| tab.on_image_read(&self.ctx, req, &path, &image));
            }
            FsEvent::ImageReadFailed { req, path, error } => {
                error!("{req}: image read failed ({}): {error}", path.display());
                self.tabs
                    .broadcast(|tab| tab.on_image_read_failed(&self.ctx, req));
                self.status
                    .push_error(format!("Failed to read image: {error}"));
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
    fn handle_simulation_library_entry_added(
        &mut self,
        req: ReqId,
        slug: Slug,
        snapshot: Arc<SimulationLibrarySnapshot>,
    ) {
        info!("{req}: simulation entry added: {slug}");
        self.tabs
            .broadcast(|tab| tab.on_simulation_library_entry_added(&self.ctx, req, &slug));
        self.set_simulation_library(snapshot);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_simulation_library_entry_updated(
        &mut self,
        req: ReqId,
        old_slug: Slug,
        new_slug: Slug,
        snapshot: Arc<SimulationLibrarySnapshot>,
    ) {
        if old_slug == new_slug {
            info!("{req}: simulation entry saved: {new_slug}");
            self.tabs
                .broadcast(|tab| tab.on_simulation_library_entry_saved(&self.ctx, req, &new_slug));
            self.set_simulation_library(snapshot);
        } else {
            info!("{req}: simulation entry renamed: {old_slug} -> {new_slug}");
            self.set_simulation_library(snapshot);
            self.tabs.broadcast(|tab| {
                tab.on_simulation_library_entry_renamed(&self.ctx, &old_slug, &new_slug);
            });
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_simulation_library_entry_removed(
        &mut self,
        req: ReqId,
        slug: Slug,
        snapshot: Arc<SimulationLibrarySnapshot>,
    ) {
        info!("{req}: simulation entry removed: {slug}");
        self.set_simulation_library(snapshot);
    }

    fn set_simulation_library(&mut self, snapshot: Arc<SimulationLibrarySnapshot>) {
        self.ctx.simulation_library_snapshot = snapshot;
        self.tabs
            .broadcast(|tab| tab.on_simulation_library_changed(&self.ctx));
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_simulation_library_op_failed(
        &mut self,
        req: ReqId,
        error: Box<SimulationLibraryError>,
    ) {
        error!("{req}: simulation library operation failed: {error}");
        self.tabs
            .broadcast(|tab| tab.on_simulation_library_op_failed(&self.ctx, req));
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
    fn handle_backup_library_entry_added(
        &mut self,
        req: ReqId,
        slug: Slug,
        snapshot: Arc<BackupLibrarySnapshot>,
    ) {
        info!("{req}: backup entry added: {slug}");
        self.tabs
            .broadcast(|tab| tab.on_backup_library_entry_added(&self.ctx, &slug));
        self.set_backup_library(snapshot);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_backup_library_entry_updated(
        &mut self,
        req: ReqId,
        old_slug: Slug,
        new_slug: Slug,
        snapshot: Arc<BackupLibrarySnapshot>,
    ) {
        if old_slug == new_slug {
            info!("{req}: backup entry updated: {new_slug}");
        } else {
            info!("{req}: backup entry renamed: {old_slug} -> {new_slug}");
        }
        self.tabs
            .broadcast(|tab| tab.on_backup_library_entry_updated(&self.ctx, &new_slug));
        self.set_backup_library(snapshot);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_backup_library_entry_removed(
        &mut self,
        req: ReqId,
        slug: Slug,
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
            error!("{req}: backup blob read but no device connected");
            self.tabs
                .broadcast(|tab| tab.on_backup_library_op_failed(&self.ctx, req));
            self.status
                .push_error("No camera connected to import backup");
        }
    }

    fn set_backup_library(&mut self, snapshot: Arc<BackupLibrarySnapshot>) {
        self.ctx.backup_library_snapshot = snapshot;
        self.tabs
            .broadcast(|tab| tab.on_backup_library_changed(&self.ctx));
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_backup_library_op_failed(&mut self, req: ReqId, error: Box<BackupLibraryError>) {
        error!("{req}: backup library operation failed: {error}");
        self.tabs
            .broadcast(|tab| tab.on_backup_library_op_failed(&self.ctx, req));
        self.status
            .push_error(format!("Backup library operation failed: {error}"));
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_fs_error(&mut self, error: Box<FsError>) {
        error!("fs error: {error}");
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
            self.ctx.overlay = self.modal.is_some();
            Header::draw(self, frame, header_area);
            self.tabs.draw(&self.ctx, self.active_tab, frame, body_area);
        }
        Status::draw(self, frame, status_area);

        if let Some(modal) = self.modal.as_mut() {
            modal.render(frame, frame.area());
        }
    }
}
