pub mod backup;
pub mod render;
pub mod simulation;
pub mod state;

pub use state::{Buffer, Shadowed};

use std::{path::Path, sync::Arc};

use crossterm::event::KeyEvent;
use fujicore::{
    Capability, CoreError,
    generated::{options::CustomSetting, renders::RenderBase, simulations::SimulationBase},
};
use ratatui::{Frame, layout::Rect};
use ratatui_image::{picker::Picker, thread::ResizeRequest};

use crate::{
    ui::{
        Keybind,
        tabs::{backup::BackupTabState, render::RenderTabState, simulation::SimulationTabState},
        widgets::StatusMessage,
    },
    workers::{
        ReqId, ReqIdGen,
        device::{DeviceHandle, DeviceSnapshot},
        fs::{
            FsHandle, backup::BackupLibrarySnapshot, simulation::SimulationLibrarySnapshot,
            slug::Slug,
        },
    },
};

pub struct AppCtx {
    pub device: Option<DeviceHandle>,
    #[allow(dead_code)]
    pub fs: FsHandle,
    pub req: ReqIdGen,
    pub device_snapshot: Option<DeviceSnapshot>,
    pub simulation_library_snapshot: Arc<SimulationLibrarySnapshot>,
    pub backup_library_snapshot: Arc<BackupLibrarySnapshot>,
    pub image_picker: Picker,
    pub resize_tx: std::sync::mpsc::Sender<ResizeRequest>,
    pub overlay: bool,
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
    pub const fn required_capability(self) -> Capability {
        match self {
            Self::Simulation => Capability::SimulationManagement,
            Self::Render => Capability::RenderManagement,
            Self::Backup => Capability::BackupManagement,
        }
    }

    #[must_use]
    pub fn available(capabilities: &[Capability]) -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|tab| capabilities.contains(&tab.required_capability()))
            .collect()
    }
}

#[allow(unused_variables)]
pub trait TabBehavior {
    fn render(&mut self, ctx: &AppCtx, frame: &mut Frame, area: Rect);

    fn is_capturing_input(&self) -> bool {
        false
    }

    fn keybinds(&self) -> &'static [Keybind] {
        &[]
    }

    fn on_activate(&mut self, ctx: &AppCtx) {}

    fn on_device_connected(&mut self, ctx: &AppCtx) {}

    fn on_device_disconnected(&mut self, ctx: &AppCtx) {}

    fn on_simulation_library_changed(&mut self, ctx: &AppCtx) {}

    fn on_simulation_library_entry_added(&mut self, ctx: &AppCtx, req: ReqId, slug: &Slug) {}

    fn on_simulation_library_entry_saved(&mut self, ctx: &AppCtx, req: ReqId, slug: &Slug) {}

    fn on_simulation_library_op_failed(&mut self, ctx: &AppCtx, req: ReqId) {}

    fn on_simulation_library_entry_renamed(
        &mut self,
        ctx: &AppCtx,
        old_slug: &Slug,
        new_slug: &Slug,
    ) {
    }

    fn on_simulation_slot_fetched(
        &mut self,
        ctx: &AppCtx,
        slot: CustomSetting,
        base: &SimulationBase,
    ) {
    }

    fn on_simulation_slot_fetch_failed(
        &mut self,
        ctx: &AppCtx,
        slot: CustomSetting,
        error: &Arc<CoreError>,
    ) {
    }

    fn on_simulation_slot_changed(&mut self, ctx: &AppCtx, slot: CustomSetting) {}

    fn on_simulation_slot_push_failed(
        &mut self,
        ctx: &AppCtx,
        slot: CustomSetting,
        error: &Arc<CoreError>,
    ) {
    }

    fn on_backup_exported(&mut self, ctx: &AppCtx, req: ReqId, blob: &[u8]) {}

    fn on_backup_export_failed(&mut self, ctx: &AppCtx, req: ReqId, error: &Arc<CoreError>) {}

    fn on_backup_imported(&mut self, ctx: &AppCtx, req: ReqId) {}

    fn on_backup_import_failed(&mut self, ctx: &AppCtx, req: ReqId, error: &Arc<CoreError>) {}

    fn on_backup_library_changed(&mut self, ctx: &AppCtx) {}

    fn on_backup_library_entry_added(&mut self, ctx: &AppCtx, slug: &Slug) {}

    fn on_backup_library_entry_updated(&mut self, ctx: &AppCtx, slug: &Slug) {}

    fn on_backup_library_op_failed(&mut self, ctx: &AppCtx, req: ReqId) {}

    fn on_image_read(&mut self, ctx: &AppCtx, req: ReqId, path: &Path, image: &Arc<[u8]>) {}

    fn on_image_read_failed(&mut self, ctx: &AppCtx, req: ReqId) {}

    fn on_image_loaded(&mut self, ctx: &AppCtx, req: ReqId, profile: &RenderBase) {}

    fn on_image_load_failed(&mut self, ctx: &AppCtx, req: ReqId) {}

    fn on_key(&mut self, ctx: &AppCtx, key: KeyEvent) {}

    fn take_status(&mut self) -> Vec<StatusMessage> {
        Vec::new()
    }
}

#[derive(Debug, Default)]
pub struct Tabs {
    pub simulation: SimulationTabState,
    pub rendering: RenderTabState,
    pub backup: BackupTabState,
}

impl Tabs {
    pub fn draw(&mut self, ctx: &AppCtx, active: Tab, frame: &mut Frame, area: Rect) {
        match active {
            Tab::Simulation => self.simulation.render(ctx, frame, area),
            Tab::Render => self.rendering.render(ctx, frame, area),
            Tab::Backup => self.backup.render(ctx, frame, area),
        }
    }

    pub fn handle_activate(&mut self, ctx: &AppCtx, active: Tab) {
        match active {
            Tab::Simulation => self.simulation.on_activate(ctx),
            Tab::Render => self.rendering.on_activate(ctx),
            Tab::Backup => self.backup.on_activate(ctx),
        }
    }

    pub fn handle_key(&mut self, ctx: &AppCtx, active: Tab, key: KeyEvent) {
        match active {
            Tab::Simulation => self.simulation.on_key(ctx, key),
            Tab::Render => self.rendering.on_key(ctx, key),
            Tab::Backup => self.backup.on_key(ctx, key),
        }
    }

    fn each_mut(&mut self) -> [&mut dyn TabBehavior; 3] {
        [&mut self.simulation, &mut self.rendering, &mut self.backup]
    }

    pub fn broadcast(&mut self, mut hook: impl FnMut(&mut dyn TabBehavior)) {
        for tab in self.each_mut() {
            hook(tab);
        }
    }

    pub fn take_status(&mut self) -> Vec<StatusMessage> {
        let mut statuses = Vec::new();
        for tab in self.each_mut() {
            statuses.append(&mut tab.take_status());
        }
        statuses
    }

    #[must_use]
    pub fn is_capturing_input(&self, active: Tab) -> bool {
        match active {
            Tab::Simulation => self.simulation.is_capturing_input(),
            Tab::Render => self.rendering.is_capturing_input(),
            Tab::Backup => self.backup.is_capturing_input(),
        }
    }

    #[must_use]
    pub fn keybinds(&self, active: Tab) -> &'static [Keybind] {
        match active {
            Tab::Simulation => self.simulation.keybinds(),
            Tab::Render => self.rendering.keybinds(),
            Tab::Backup => self.backup.keybinds(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_filters_by_capability() {
        assert_eq!(
            Tab::available(&[Capability::BackupManagement]),
            vec![Tab::Backup]
        );
    }

    #[test]
    fn available_preserves_canonical_order() {
        let caps = [
            Capability::RenderManagement,
            Capability::BackupManagement,
            Capability::SimulationManagement,
        ];
        assert_eq!(
            Tab::available(&caps),
            vec![Tab::Simulation, Tab::Render, Tab::Backup]
        );
    }

    #[test]
    fn available_is_empty_without_relevant_capabilities() {
        assert!(Tab::available(&[Capability::SimulationParsing]).is_empty());
    }
}
