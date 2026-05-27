pub mod backup;
pub mod common;
pub mod device;
pub mod modal;
pub mod render;
pub mod simulation;

use std::time::Duration;

use clap::{ArgAction, Parser};
use crossbeam_channel::{Receiver, Sender, after, select};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use log::{info, warn};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
};

use crate::tui::{
    common::{
        action::{self, Action},
        header, input, status,
        usb::{self, DeviceCandidate},
    },
    device::{DeviceEvent, DeviceHandle, DeviceSnapshot},
    modal::{
        ModalEffect, ModalHandler, ModalOutcome, device::DevicePickerModal, fatal::FatalModal,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Simulation,
    Render,
    Backup,
}

impl Tab {
    pub const ALL: [Self; 3] = [Self::Simulation, Self::Render, Self::Backup];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Simulation => "Simulations",
            Self::Render => "Rendering",
            Self::Backup => "Backups",
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Simulation => 0,
            Self::Render => 1,
            Self::Backup => 2,
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Simulation => Self::Render,
            Self::Render => Self::Backup,
            Self::Backup => Self::Simulation,
        }
    }

    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Simulation => Self::Backup,
            Self::Render => Self::Simulation,
            Self::Backup => Self::Render,
        }
    }
}

pub struct App {
    pub tab: Tab,
    pub snapshot: Option<DeviceSnapshot>,
    pub modal: Option<Box<dyn ModalHandler>>,
    pub status_message: Option<String>,
    quitting: bool,

    input_rx: Receiver<KeyEvent>,
    event_rx: Receiver<DeviceEvent>,
    event_tx: Sender<DeviceEvent>,
    device: Option<DeviceHandle>,
}

pub fn run() -> anyhow::Result<()> {
    let candidates = usb::enumerate()?;
    info!("found {} supported camera(s)", candidates.len());

    let (input_tx, input_rx) = crossbeam_channel::bounded(INPUT_CHANNEL_BOUND);
    input::spawn(input_tx);

    let app = App::new(candidates, input_rx);
    ratatui::run(|terminal| app.run(terminal))
}

impl App {
    fn new(candidates: Vec<DeviceCandidate>, input_rx: Receiver<KeyEvent>) -> Self {
        let (event_tx, event_rx) = crossbeam_channel::unbounded();

        let (modal, device): (Option<Box<dyn ModalHandler>>, _) = match candidates.len() {
            0 => (Some(Box::new(FatalModal::no_device())), None),
            1 => {
                let candidate = candidates.into_iter().next().expect("len > 0");
                let handle = DeviceHandle::spawn(candidate.device, event_tx.clone());
                (None, Some(handle))
            }
            _ => (Some(Box::new(DevicePickerModal::new(candidates))), None),
        };

        Self {
            tab: Tab::Simulation,
            snapshot: None,
            modal,
            status_message: None,
            quitting: false,
            input_rx,
            event_rx,
            event_tx,
            device,
        }
    }

    fn run(mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        while !self.quitting {
            terminal.draw(|frame| draw(frame, &self))?;

            select! {
                recv(self.input_rx) -> msg => self.handle_key(msg?),
                recv(self.event_rx) -> msg => self.handle_event(msg?),
                recv(after(TICK)) -> _ => {}
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
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

        let Some(action) = action::map(key) else {
            return;
        };

        match action {
            Action::Quit => self.quitting = true,
            Action::NextTab => self.tab = self.tab.next(),
            Action::PrevTab => self.tab = self.tab.prev(),
            Action::GotoTab(t) => self.tab = t,
        }
    }

    fn handle_event(&mut self, event: DeviceEvent) {
        match event {
            DeviceEvent::Connected(snap) => {
                info!("device connected: {} ({})", snap.name, snap.usb_id);
                self.snapshot = Some(snap);
            }
            DeviceEvent::InfoUpdated(snap) => {
                self.snapshot = Some(snap);
            }
            DeviceEvent::Disconnected => {
                info!("device disconnected");
                self.snapshot = None;
                self.device = None;
                self.modal = Some(Box::new(FatalModal::disconnect()));
            }
            DeviceEvent::Error(e) => {
                let msg = e.to_string();
                warn!("device error: {msg}");
                self.status_message = Some(msg);
            }
        }
    }

    fn apply_effect(&mut self, effect: ModalEffect) {
        match effect {
            ModalEffect::Quit => self.quitting = true,
            ModalEffect::SelectDevice(candidate) => {
                let handle = DeviceHandle::spawn(candidate.device, self.event_tx.clone());
                self.device = Some(handle);
            }
        }
    }
}

fn draw(frame: &mut Frame, app: &App) {
    let [header_area, body_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    header::render(frame, header_area, app.tab);
    match app.tab {
        Tab::Simulation => simulation::render(frame, body_area),
        Tab::Render => render::render(frame, body_area),
        Tab::Backup => backup::render(frame, body_area),
    }
    status::render(frame, status_area, app);

    if let Some(modal) = &app.modal {
        modal.render(frame, frame.area());
    }
}
