pub mod backup;
pub mod render;
pub mod simulation;
pub mod state;

pub use state::{Buffer, Shadowed};

use std::sync::Arc;

use crossterm::event::KeyEvent;
use fujicore::{
    CoreError,
    generated::{options::CustomSetting, simulations::SimulationBase},
};
use ratatui::{Frame, layout::Rect};

use crate::{
    ui::tabs::{backup::BackupTabState, render::RenderTabState, simulation::SimulationTabState},
    workers::{
        ReqId, ReqIdGen,
        device::{DeviceHandle, DeviceSnapshot},
        fs::{FsHandle, library::LibrarySnapshot},
    },
};

pub struct AppCtx {
    pub device: Option<DeviceHandle>,
    #[allow(dead_code)]
    pub fs: FsHandle,
    pub req: ReqIdGen,
    pub device_snapshot: Option<DeviceSnapshot>,
    pub library_snapshot: Arc<LibrarySnapshot>,
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

#[allow(unused_variables)]
pub trait TabBehavior {
    fn render(&self, ctx: &AppCtx, frame: &mut Frame, area: Rect);

    fn is_capturing_input(&self) -> bool {
        false
    }

    fn on_activate(&mut self, ctx: &AppCtx) {}

    fn on_device_connected(&mut self, ctx: &AppCtx) {}

    fn on_device_disconnected(&mut self, ctx: &AppCtx) {}

    fn on_library_changed(&mut self, ctx: &AppCtx) {}

    fn on_slots_enumerated(&mut self, ctx: &AppCtx, req: ReqId, slots: &[CustomSetting]) {}

    fn on_slot_fetched(&mut self, ctx: &AppCtx, slot: CustomSetting, base: &SimulationBase) {}

    fn on_slot_fetch_failed(&mut self, ctx: &AppCtx, slot: CustomSetting, error: &Arc<CoreError>) {}

    fn on_slot_changed(&mut self, ctx: &AppCtx, slot: CustomSetting) {}

    fn on_slot_push_failed(&mut self, ctx: &AppCtx, slot: CustomSetting, error: &Arc<CoreError>) {}

    fn handle_key(&mut self, ctx: &AppCtx, key: KeyEvent) {}
}

#[derive(Debug)]
pub struct Tabs {
    pub simulation: SimulationTabState,
    pub rendering: RenderTabState,
    pub backup: BackupTabState,
}

impl Default for Tabs {
    fn default() -> Self {
        Self {
            simulation: SimulationTabState::default(),
            rendering: RenderTabState,
            backup: BackupTabState,
        }
    }
}

impl Tabs {
    pub fn render(&self, ctx: &AppCtx, active: Tab, frame: &mut Frame, area: Rect) {
        match active {
            Tab::Simulation => self.simulation.render(ctx, frame, area),
            Tab::Render => self.rendering.render(ctx, frame, area),
            Tab::Backup => self.backup.render(ctx, frame, area),
        }
    }

    pub fn on_activate(&mut self, ctx: &AppCtx, active: Tab) {
        match active {
            Tab::Simulation => self.simulation.on_activate(ctx),
            Tab::Render => self.rendering.on_activate(ctx),
            Tab::Backup => self.backup.on_activate(ctx),
        }
    }

    pub fn handle_key(&mut self, ctx: &AppCtx, active: Tab, key: KeyEvent) {
        match active {
            Tab::Simulation => self.simulation.handle_key(ctx, key),
            Tab::Render => self.rendering.handle_key(ctx, key),
            Tab::Backup => self.backup.handle_key(ctx, key),
        }
    }

    #[must_use]
    pub fn is_capturing_input(&self, active: Tab) -> bool {
        match active {
            Tab::Simulation => self.simulation.is_capturing_input(),
            Tab::Render => self.rendering.is_capturing_input(),
            Tab::Backup => self.backup.is_capturing_input(),
        }
    }

    pub fn on_device_connected(&mut self, ctx: &AppCtx) {
        self.simulation.on_device_connected(ctx);
        self.rendering.on_device_connected(ctx);
        self.backup.on_device_connected(ctx);
    }

    pub fn on_device_disconnected(&mut self, ctx: &AppCtx) {
        self.simulation.on_device_disconnected(ctx);
        self.rendering.on_device_disconnected(ctx);
        self.backup.on_device_disconnected(ctx);
    }

    pub fn on_library_changed(&mut self, ctx: &AppCtx) {
        self.simulation.on_library_changed(ctx);
        self.rendering.on_library_changed(ctx);
        self.backup.on_library_changed(ctx);
    }

    pub fn on_slots_enumerated(&mut self, ctx: &AppCtx, req: ReqId, slots: &[CustomSetting]) {
        self.simulation.on_slots_enumerated(ctx, req, slots);
        self.rendering.on_slots_enumerated(ctx, req, slots);
        self.backup.on_slots_enumerated(ctx, req, slots);
    }

    pub fn on_slot_fetched(&mut self, ctx: &AppCtx, slot: CustomSetting, base: &SimulationBase) {
        self.simulation.on_slot_fetched(ctx, slot, base);
        self.rendering.on_slot_fetched(ctx, slot, base);
        self.backup.on_slot_fetched(ctx, slot, base);
    }

    pub fn on_slot_fetch_failed(
        &mut self,
        ctx: &AppCtx,
        slot: CustomSetting,
        error: &Arc<CoreError>,
    ) {
        self.simulation.on_slot_fetch_failed(ctx, slot, error);
        self.rendering.on_slot_fetch_failed(ctx, slot, error);
        self.backup.on_slot_fetch_failed(ctx, slot, error);
    }

    pub fn on_slot_changed(&mut self, ctx: &AppCtx, slot: CustomSetting) {
        self.simulation.on_slot_changed(ctx, slot);
        self.rendering.on_slot_changed(ctx, slot);
        self.backup.on_slot_changed(ctx, slot);
    }

    pub fn on_slot_push_failed(
        &mut self,
        ctx: &AppCtx,
        slot: CustomSetting,
        error: &Arc<CoreError>,
    ) {
        self.simulation.on_slot_push_failed(ctx, slot, error);
        self.rendering.on_slot_push_failed(ctx, slot, error);
        self.backup.on_slot_push_failed(ctx, slot, error);
    }
}
