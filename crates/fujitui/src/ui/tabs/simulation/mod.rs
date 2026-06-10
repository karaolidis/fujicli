mod state;

use std::sync::Arc;

use crossterm::event::KeyEvent;
use fujicore::{
    CoreError,
    generated::{options::CustomSetting, simulations::SimulationBase},
};
use log::{debug, error};
use ratatui::{Frame, layout::Rect};

use crate::{
    ui::{
        Keybind,
        tabs::{AppCtx, TabBehavior},
        widgets::StatusMessage,
    },
    workers::{ReqId, fs::slug::Slug},
};

pub use state::SimulationTabState;

impl TabBehavior for SimulationTabState {
    fn render(&mut self, _ctx: &AppCtx, frame: &mut Frame, area: Rect) {
        self.draw(frame, area);
    }

    fn is_capturing_input(&self) -> bool {
        self.capturing_input()
    }

    fn keybinds(&self) -> &'static [Keybind] {
        self.keybinds()
    }

    fn on_activate(&mut self, ctx: &AppCtx) {
        match self.request_fetch(ctx) {
            Ok(req) => debug!("{req}: slot fetch requested"),
            Err(reason) => debug!("slot fetch skipped: {reason}"),
        }
    }

    fn on_device_connected(&mut self, ctx: &AppCtx) {
        match self.request_fetch(ctx) {
            Ok(req) => debug!("{req}: slot fetch requested"),
            Err(reason) => debug!("slot fetch skipped: {reason}"),
        }
    }

    fn on_device_disconnected(&mut self, _ctx: &AppCtx) {
        self.invalidate();
        self.cancel_device_actions();
    }

    fn on_simulation_slot_changed(&mut self, ctx: &AppCtx, slot: CustomSetting) {
        self.handle_simulation_slot_changed(ctx, slot);
    }

    fn on_simulation_slot_push_failed(
        &mut self,
        _ctx: &AppCtx,
        slot: CustomSetting,
        _error: &Arc<CoreError>,
    ) {
        self.handle_simulation_slot_push_failed(slot);
    }

    fn on_simulation_library_entry_added(&mut self, _ctx: &AppCtx, req: ReqId, slug: &Slug) {
        self.handle_simulation_library_entry_added(req, slug);
    }

    fn on_simulation_library_entry_saved(&mut self, _ctx: &AppCtx, req: ReqId, slug: &Slug) {
        if let Err(anomaly) = self.handle_simulation_library_entry_saved(req, slug) {
            error!("library save anomaly: {anomaly}");
        }
    }

    fn on_simulation_library_op_failed(&mut self, _ctx: &AppCtx, req: ReqId) {
        self.handle_simulation_library_op_failed(req);
    }

    fn on_simulation_library_entry_renamed(
        &mut self,
        _ctx: &AppCtx,
        old_slug: &Slug,
        new_slug: &Slug,
    ) {
        self.handle_simulation_library_entry_renamed(old_slug, new_slug);
    }

    fn on_simulation_library_changed(&mut self, ctx: &AppCtx) {
        let report = self.sync_library(&ctx.simulation_library_snapshot);
        if !report.updated_with_conflict.is_empty() {
            error!(
                "simulation library: external changed for entries with unsaved edits: {:?}",
                report.updated_with_conflict
            );
        }
    }

    fn on_simulation_slot_fetched(
        &mut self,
        _ctx: &AppCtx,
        slot: CustomSetting,
        base: &SimulationBase,
    ) {
        if let Err(anomaly) = self.handle_simulation_slot_fetched(slot, base) {
            error!("slot fetched anomaly ({slot}): {anomaly}");
        }
    }

    fn on_simulation_slot_fetch_failed(
        &mut self,
        _ctx: &AppCtx,
        slot: CustomSetting,
        error: &Arc<CoreError>,
    ) {
        if let Err(anomaly) = self.handle_simulation_slot_fetch_failed(slot, Arc::clone(error)) {
            error!("slot fetch-failed anomaly ({slot}): {anomaly}");
        }
    }

    fn on_backup_imported(&mut self, ctx: &AppCtx, req: ReqId) {
        debug!("{req}: backup imported; refetching simulation slots");
        self.invalidate();
        match self.request_fetch(ctx) {
            Ok(req) => debug!("{req}: slot refetch requested"),
            Err(reason) => debug!("slot refetch skipped: {reason}"),
        }
    }

    fn on_key(&mut self, ctx: &AppCtx, key: KeyEvent) {
        self.handle_key(ctx, key);
    }

    fn take_status(&mut self) -> Vec<StatusMessage> {
        self.drain_status()
    }
}
