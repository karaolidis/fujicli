mod state;

use std::sync::Arc;

use crossterm::event::KeyEvent;
use fujicore::{
    CoreError,
    generated::{options::CustomSetting, simulations::SimulationBase},
};
use log::{debug, warn};
use ratatui::{Frame, layout::Rect};

use crate::{
    ui::{
        Keybind,
        tabs::{AppCtx, TabBehavior},
    },
    workers::ReqId,
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
        self.mark_stale();
    }

    fn on_simulation_library_changed(&mut self, ctx: &AppCtx) {
        let report = self.sync_library(&ctx.simulation_library_snapshot);
        if !report.updated_with_conflict.is_empty() {
            warn!(
                "simulation library: external changed for entries with unsaved edits: {:?}",
                report.updated_with_conflict
            );
        }
    }

    fn on_slots_enumerated(&mut self, _ctx: &AppCtx, req: ReqId, slots: &[CustomSetting]) {
        if let Err(anomaly) = self.handle_slots_enumerated(req, slots) {
            warn!("slot enumeration anomaly: {anomaly}");
        }
    }

    fn on_slots_enumeration_failed(&mut self, _ctx: &AppCtx, req: ReqId) {
        if let Err(anomaly) = self.handle_slots_enumeration_failed(req) {
            warn!("slot enumeration failed anomaly: {anomaly}");
        }
    }

    fn on_slot_fetched(&mut self, _ctx: &AppCtx, slot: CustomSetting, base: &SimulationBase) {
        if let Err(anomaly) = self.handle_slot_fetched(slot, base) {
            warn!("slot fetched anomaly ({slot}): {anomaly}");
        }
    }

    fn on_slot_fetch_failed(&mut self, _ctx: &AppCtx, slot: CustomSetting, error: &Arc<CoreError>) {
        if let Err(anomaly) = self.handle_slot_fetch_failed(slot, Arc::clone(error)) {
            warn!("slot fetch-failed anomaly ({slot}): {anomaly}");
        }
    }

    fn on_backup_imported(&mut self, ctx: &AppCtx, req: ReqId) {
        debug!("{req}: backup imported; refetching simulation slots");
        self.mark_stale();
        match self.request_fetch(ctx) {
            Ok(req) => debug!("{req}: slot refetch requested"),
            Err(reason) => debug!("slot refetch skipped: {reason}"),
        }
    }

    fn on_key(&mut self, _ctx: &AppCtx, key: KeyEvent) {
        self.handle_key(key);
    }
}
