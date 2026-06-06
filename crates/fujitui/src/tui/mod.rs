use std::{sync::Arc, time::Duration};

use clap::{ArgAction, Parser};
use crossbeam_channel::{Receiver, Sender, after, select};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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
        widgets::{Header, Status},
    },
    workers::{
        ReqId, ReqIdGen,
        device::{
            DeviceEvent, DeviceHandle,
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
    pub ctx: AppCtx,

    pub header: Header,
    pub tabs: Tabs,
    pub active_tab: Tab,
    pub status: Status,
    pub modal: Option<Box<dyn ModalHandler>>,
    pub status_message: Option<String>,

    quitting: bool,

    input_rx: Receiver<KeyEvent>,
    device_rx: Receiver<DeviceEvent>,
    device_tx: Sender<DeviceEvent>,
    fs_rx: Receiver<FsEvent>,
}

pub fn run(dirs: &ProjectDirs) -> anyhow::Result<()> {
    let candidates = usb::enumerate()?;
    info!("found {} supported camera(s)", candidates.len());

    let (input_tx, input_rx) = crossbeam_channel::bounded(INPUT_CHANNEL_BOUND);
    input::spawn(input_tx);

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

        let fs = FsHandle::spawn(library_dir, fs_tx);
        let req = ReqIdGen::new();

        let load_req = req.next();
        debug!("{load_req}: loading library");
        fs.send(FsCommand::LoadLibrary { req: load_req });

        Self {
            ctx: AppCtx {
                device,
                fs,
                req,
                device_snapshot: None,
                library_snapshot: LibrarySnapshot::empty(),
            },
            header: Header,
            tabs: Tabs::default(),
            active_tab: Tab::Simulation,
            status: Status,
            modal,
            status_message: None,
            quitting: false,
            input_rx,
            device_rx,
            device_tx,
            fs_rx,
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

        if key.kind != KeyEventKind::Press {
            return;
        }

        if let Some(action) = actions::map(key) {
            match action {
                Action::Quit => {
                    info!("quit requested");
                    self.quitting = true;
                }
                Action::NextTab => self.set_tab(self.active_tab.next()),
                Action::PrevTab => self.set_tab(self.active_tab.prev()),
                Action::GotoTab(t) => self.set_tab(t),
            }
            return;
        }

        self.tabs.handle_key(&self.ctx, self.active_tab, key);
    }

    fn set_tab(&mut self, target: Tab) {
        self.active_tab = target;
        self.tabs.on_activate(&self.ctx, target);
    }

    fn handle_device_event(&mut self, event: DeviceEvent) {
        match event {
            DeviceEvent::Connected(snap) => {
                info!(
                    "device connected: {} ({}, bus {}, battery {}%)",
                    snap.name, snap.usb_id, snap.bus_address, snap.battery
                );
                self.ctx.device_snapshot = Some(snap);
                self.tabs.on_device_connected(&self.ctx);
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
                self.tabs.on_device_disconnected(&self.ctx);
            }
            DeviceEvent::Error(error) => {
                let msg = error.to_string();
                warn!("device error: {msg}");
                self.status_message = Some(msg);
            }
            DeviceEvent::SlotsEnumerated { req, slots } => {
                debug!("{req}: slots enumerated ({} slots)", slots.len());
                self.tabs.on_slots_enumerated(&self.ctx, req, &slots);
            }
            DeviceEvent::SlotFetched { req, slot, base } => {
                debug!("{req}: slot {slot} fetched");
                self.tabs.on_slot_fetched(&self.ctx, slot, &base);
            }
            DeviceEvent::SlotFetchFailed { req, slot, error } => {
                let msg = format!("{req}: fetch of {slot} failed: {error}");
                warn!("{msg}");
                let error: Arc<CoreError> = error.into();
                self.tabs.on_slot_fetch_failed(&self.ctx, slot, &error);
                self.status_message = Some(msg);
            }
            DeviceEvent::SlotChanged { req, slot } => {
                info!("{req}: slot {slot} pushed");
                self.tabs.on_slot_changed(&self.ctx, slot);
            }
            DeviceEvent::SlotPushFailed { req, slot, error } => {
                let msg = format!("{req}: push to {slot} failed: {error}");
                warn!("{msg}");
                let error: Arc<CoreError> = error.into();
                self.tabs.on_slot_push_failed(&self.ctx, slot, &error);
                self.status_message = Some(msg);
            }
        }
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
        self.set_library(snapshot);
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
        self.set_library(snapshot);
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
        self.set_library(snapshot);
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
        self.set_library(snapshot);
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
        self.set_library(snapshot);
    }

    fn set_library(&mut self, snapshot: Arc<LibrarySnapshot>) {
        self.ctx.library_snapshot = snapshot;
        self.tabs.on_library_changed(&self.ctx);
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
            "device selected: {} ({}, bus {}.{})",
            candidate.name, candidate.usb_id, candidate.bus, candidate.address
        );
        let handle = DeviceHandle::spawn(candidate.device, self.device_tx.clone());
        self.ctx.device = Some(handle);
    }

    fn draw(&self, frame: &mut Frame) {
        let [header_area, body_area, status_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());

        self.header.render(self, frame, header_area);
        self.tabs
            .render(&self.ctx, self.active_tab, frame, body_area);
        self.status.render(self, frame, status_area);

        if let Some(modal) = &self.modal {
            modal.render(frame, frame.area());
        }
    }
}
