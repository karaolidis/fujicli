use std::{sync::Arc, time::Duration};

use anyhow::bail;
use clap::{ArgAction, Parser};
use crossbeam_channel::{Receiver, Sender, after, select};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use directories::ProjectDirs;
use fujicore::{
    CoreError,
    generated::{options::CustomSetting, simulations::SimulationBase},
};
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
        tabs::{backup, render, simulation},
        widgets::{header, status},
    },
    workers::{
        ReqId, ReqIdGen,
        device::{
            DeviceEvent, DeviceHandle, DeviceSnapshot,
            usb::{self, DeviceCandidate},
        },
        fs::{
            FsCommand, FsError, FsEvent, FsHandle,
            library::{LibraryEntry, LibraryError, LibrarySnapshot, Slug},
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
    pub tab: Tab,
    pub snapshot: Option<DeviceSnapshot>,
    pub library: Arc<LibrarySnapshot>,
    pub modal: Option<Box<dyn ModalHandler>>,
    pub status_message: Option<String>,
    #[allow(dead_code)]
    req: ReqIdGen,
    quitting: bool,

    input_rx: Receiver<KeyEvent>,
    device_rx: Receiver<DeviceEvent>,
    device_tx: Sender<DeviceEvent>,
    device: Option<DeviceHandle>,
    fs_rx: Receiver<FsEvent>,
    #[allow(dead_code)]
    fs: FsHandle,
}

pub fn run() -> anyhow::Result<()> {
    let candidates = usb::enumerate()?;
    info!("found {} supported camera(s)", candidates.len());

    let (input_tx, input_rx) = crossbeam_channel::bounded(INPUT_CHANNEL_BOUND);
    input::spawn(input_tx);

    let Some(dirs) = ProjectDirs::from("", "", "fujicli") else {
        bail!("cannot determine project directories for this platform");
    };

    let library = dirs.data_dir().join("library");
    info!("library directory: {}", library.display());

    let app = App::new(candidates, input_rx, library);
    ratatui::run(|terminal| app.run(terminal))
}

impl App {
    fn new(
        candidates: Vec<DeviceCandidate>,
        input_rx: Receiver<KeyEvent>,
        library_dir: std::path::PathBuf,
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
                    "single camera detected: {} ({:04x}:{:04x}, bus {}.{})",
                    candidate.name,
                    candidate.vendor,
                    candidate.product,
                    candidate.bus,
                    candidate.address
                );
                let handle = DeviceHandle::spawn(candidate.device, device_tx.clone());
                (None, Some(handle))
            }
            n => {
                info!("{n} cameras detected");
                (Some(Box::new(DevicePickerModal::new(candidates))), None)
            }
        };

        let fs = FsHandle::spawn(library_dir, fs_tx);
        let req = ReqIdGen::new();

        let load_req = req.next();
        debug!("{load_req}: loading library");
        fs.send(FsCommand::LoadLibrary { req: load_req });

        Self {
            tab: Tab::Simulation,
            snapshot: None,
            library: LibrarySnapshot::empty(),
            modal,
            status_message: None,
            req,
            quitting: false,
            input_rx,
            device_rx,
            device_tx,
            device,
            fs_rx,
            fs,
        }
    }

    fn run(mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
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
            match modal.handle_key(key) {
                ModalOutcome::Continue => {}
                ModalOutcome::Effect(effect) => {
                    self.modal = None;
                    self.apply_effect(effect);
                }
            }
            return;
        }

        let Some(action) = actions::map(key) else {
            return;
        };

        match action {
            Action::Quit => self.handle_action_quit(),
            Action::NextTab => self.handle_action_next_tab(),
            Action::PrevTab => self.handle_action_prev_tab(),
            Action::GotoTab(t) => self.handle_action_goto_tab(t),
        }
    }

    fn handle_action_quit(&mut self) {
        info!("quit requested");
        self.quitting = true;
    }

    const fn handle_action_next_tab(&mut self) {
        self.tab = self.tab.next();
    }

    const fn handle_action_prev_tab(&mut self) {
        self.tab = self.tab.prev();
    }

    const fn handle_action_goto_tab(&mut self, target: Tab) {
        self.tab = target;
    }

    fn handle_device_event(&mut self, event: DeviceEvent) {
        match event {
            DeviceEvent::Connected(snap) => self.handle_device_connected(snap),
            DeviceEvent::InfoUpdated(snap) => self.handle_device_info_updated(snap),
            DeviceEvent::Disconnected => self.handle_device_disconnected(),
            DeviceEvent::Error(error) => self.handle_device_error(error),
            DeviceEvent::SlotFetched { req, slot, base } => {
                self.handle_slot_fetched(req, slot, base);
            }
            DeviceEvent::SlotFetchFailed { req, slot, error } => {
                self.handle_slot_fetch_failed(req, slot, error);
            }
            DeviceEvent::SlotChanged { req, slot } => self.handle_slot_changed(req, slot),
            DeviceEvent::SlotPushFailed { req, slot, error } => {
                self.handle_slot_push_failed(req, slot, error);
            }
        }
    }

    fn handle_device_connected(&mut self, snap: DeviceSnapshot) {
        info!(
            "device connected: {} ({}, battery {}%)",
            snap.name, snap.usb_id, snap.battery
        );
        self.snapshot = Some(snap);
    }

    fn handle_device_info_updated(&mut self, snap: DeviceSnapshot) {
        debug!("device info: battery {}%", snap.battery);
        self.snapshot = Some(snap);
    }

    fn handle_device_disconnected(&mut self) {
        info!("device disconnected");
        self.snapshot = None;
        self.device = None;
        self.modal = Some(Box::new(FatalModal::disconnect()));
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_device_error(&mut self, error: Box<CoreError>) {
        let msg = error.to_string();
        warn!("device error: {msg}");
        self.status_message = Some(msg);
    }

    #[allow(clippy::needless_pass_by_ref_mut, clippy::unused_self)]
    fn handle_slot_fetched(&mut self, req: ReqId, slot: CustomSetting, _base: SimulationBase) {
        debug!("{req}: slot {slot} fetched");
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_slot_fetch_failed(&mut self, req: ReqId, slot: CustomSetting, error: Box<CoreError>) {
        let msg = format!("{req}: fetch of {slot} failed: {error}");
        warn!("{msg}");
        self.status_message = Some(msg);
    }

    #[allow(clippy::needless_pass_by_ref_mut, clippy::unused_self)]
    fn handle_slot_changed(&mut self, req: ReqId, slot: CustomSetting) {
        info!("{req}: slot {slot} pushed");
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_slot_push_failed(&mut self, req: ReqId, slot: CustomSetting, error: Box<CoreError>) {
        let msg = format!("{req}: push to {slot} failed: {error}");
        warn!("{msg}");
        self.status_message = Some(msg);
    }

    fn handle_fs_event(&mut self, event: FsEvent) {
        match event {
            FsEvent::LibraryLoaded {
                req,
                snapshot,
                skipped,
            } => {
                self.handle_library_loaded(req, snapshot, skipped);
            }
            FsEvent::LibraryReloaded {
                req,
                snapshot,
                skipped,
            } => {
                self.handle_library_reloaded(req, snapshot, skipped);
            }
            FsEvent::LibraryEntryAdded {
                req,
                slug,
                entry,
                snapshot,
            } => self.handle_library_entry_added(req, slug, entry, snapshot),
            FsEvent::LibraryEntryUpdated {
                req,
                old_slug,
                new_slug,
                entry,
                snapshot,
            } => self.handle_library_entry_updated(req, old_slug, new_slug, entry, snapshot),
            FsEvent::LibraryEntryRemoved {
                req,
                slug,
                entry,
                snapshot,
            } => self.handle_library_entry_removed(req, slug, entry, snapshot),
            FsEvent::LibraryOpFailed { req, error } => {
                self.handle_library_op_failed(req, error);
            }
            FsEvent::Error(error) => self.handle_fs_error(error),
        }
    }

    fn handle_library_loaded(
        &mut self,
        req: ReqId,
        snapshot: Arc<LibrarySnapshot>,
        skipped: usize,
    ) {
        info!(
            "{req}: library loaded ({} entries, {skipped} skipped)",
            snapshot.entries.len()
        );
        self.library = snapshot;
    }

    fn handle_library_reloaded(
        &mut self,
        req: ReqId,
        snapshot: Arc<LibrarySnapshot>,
        skipped: usize,
    ) {
        info!(
            "{req}: library reloaded ({} entries, {skipped} skipped)",
            snapshot.entries.len()
        );
        self.library = snapshot;
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_library_entry_added(
        &mut self,
        req: ReqId,
        slug: Slug,
        _entry: LibraryEntry,
        snapshot: Arc<LibrarySnapshot>,
    ) {
        info!("{req}: library entry added: {slug}");
        self.library = snapshot;
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_library_entry_updated(
        &mut self,
        req: ReqId,
        old_slug: Slug,
        new_slug: Slug,
        _entry: LibraryEntry,
        snapshot: Arc<LibrarySnapshot>,
    ) {
        if old_slug == new_slug {
            info!("{req}: library entry updated: {new_slug}");
        } else {
            info!("{req}: library entry renamed: {old_slug} -> {new_slug}");
        }
        self.library = snapshot;
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_library_entry_removed(
        &mut self,
        req: ReqId,
        slug: Slug,
        _entry: LibraryEntry,
        snapshot: Arc<LibrarySnapshot>,
    ) {
        info!("{req}: library entry removed: {slug}");
        self.library = snapshot;
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_library_op_failed(&mut self, req: ReqId, error: Box<LibraryError>) {
        let msg = format!("{req}: library op failed: {error}");
        warn!("{msg}");
        self.status_message = Some(msg);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn handle_fs_error(&mut self, error: Box<FsError>) {
        let msg = format!("fs error: {error}");
        warn!("{msg}");
        self.status_message = Some(msg);
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
            "device selected: {} ({:04x}:{:04x}, bus {}.{})",
            candidate.name, candidate.vendor, candidate.product, candidate.bus, candidate.address
        );
        let handle = DeviceHandle::spawn(candidate.device, self.device_tx.clone());
        self.device = Some(handle);
    }

    fn draw(&self, frame: &mut Frame) {
        let [header_area, body_area, status_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());

        header::render(frame, header_area, self.tab);
        match self.tab {
            Tab::Simulation => simulation::render(frame, body_area),
            Tab::Render => render::render(frame, body_area),
            Tab::Backup => backup::render(frame, body_area),
        }
        status::render(frame, status_area, self);

        if let Some(modal) = &self.modal {
            modal.render(frame, frame.area());
        }
    }
}
