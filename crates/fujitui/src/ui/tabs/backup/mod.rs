mod state;

use std::sync::Arc;

use crossterm::event::KeyEvent;
use fujicore::CoreError;
use log::error;
use ratatui::{Frame, layout::Rect};

use crate::{
    ui::{
        Keybind,
        tabs::{AppCtx, TabBehavior},
    },
    workers::{ReqId, fs::slug::Slug},
};

pub use state::BackupTabState;

impl TabBehavior for BackupTabState {
    fn render(&mut self, ctx: &AppCtx, frame: &mut Frame, area: Rect) {
        self.draw(ctx, frame, area);
    }

    fn is_capturing_input(&self) -> bool {
        self.capturing_input()
    }

    fn keybinds(&self) -> &'static [Keybind] {
        self.keybinds()
    }

    fn on_device_connected(&mut self, ctx: &AppCtx) {
        self.settle(ctx);
    }

    fn on_device_disconnected(&mut self, ctx: &AppCtx) {
        self.settle(ctx);
        self.cancel_device_actions();
    }

    fn on_backup_library_changed(&mut self, ctx: &AppCtx) {
        self.settle(ctx);
    }

    fn on_backup_exported(&mut self, ctx: &AppCtx, req: ReqId, blob: &[u8]) {
        if let Err(anomaly) = self.handle_backup_exported(ctx, req, blob) {
            error!("backup export anomaly: {anomaly}");
        }
    }

    fn on_backup_export_failed(&mut self, _ctx: &AppCtx, req: ReqId, _error: &Arc<CoreError>) {
        if let Err(anomaly) = self.handle_backup_export_failed(req) {
            error!("backup export-failed anomaly: {anomaly}");
        }
    }

    fn on_backup_imported(&mut self, _ctx: &AppCtx, req: ReqId) {
        if let Err(anomaly) = self.handle_backup_imported(req) {
            error!("backup imported anomaly: {anomaly}");
        }
    }

    fn on_backup_import_failed(&mut self, _ctx: &AppCtx, req: ReqId, _error: &Arc<CoreError>) {
        if let Err(anomaly) = self.handle_backup_import_failed(req) {
            error!("backup import-failed anomaly: {anomaly}");
        }
    }

    fn on_backup_library_op_failed(&mut self, _ctx: &AppCtx, req: ReqId) {
        self.handle_backup_library_op_failed(req);
    }

    fn on_backup_library_entry_added(&mut self, _ctx: &AppCtx, slug: &Slug) {
        self.focus_entry(slug);
    }

    fn on_backup_library_entry_updated(&mut self, _ctx: &AppCtx, slug: &Slug) {
        self.focus_entry(slug);
    }

    fn on_key(&mut self, ctx: &AppCtx, key: KeyEvent) {
        self.handle_key(ctx, key);
    }
}
