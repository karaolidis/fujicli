use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use log::{debug, info};
use ratatui::{Frame, layout::Rect};
use thiserror::Error;

use crate::{
    ui::{
        Keybind,
        tabs::AppCtx,
        widgets::{ConfirmOutcome, ConfirmState, TextInputState},
    },
    workers::{
        ReqId,
        device::DeviceCommand,
        fs::{FsCommand, backup::BackupLibrarySnapshot, slug::Slug},
    },
};

mod list;

pub(super) use crate::ui::widgets::CursorMove;

use list::BackupListPane;

const INDENT: &str = "  ";
const COL_SEPARATOR: &str = " ";
const BACKUP_NAME_MAX_LEN: usize = 128;

const LIST_KEYBINDS: &[Keybind] = &[
    Keybind {
        keys: "↑ ↓ / j k",
        action: "Move selection",
    },
    Keybind {
        keys: "e",
        action: "Export from camera",
    },
    Keybind {
        keys: "i",
        action: "Import to camera",
    },
    Keybind {
        keys: "r",
        action: "Rename",
    },
    Keybind {
        keys: "D / Del",
        action: "Delete",
    },
    Keybind {
        keys: "/",
        action: "Filter",
    },
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(super) enum BackupCursor {
    #[default]
    None,
    Entry(Slug),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(in crate::ui::tabs::backup) enum ActionSkipError {
    #[error("no device connected")]
    NoDevice,
    #[error("no backup focused")]
    NoSelection,
    #[error("backup belongs to a different camera")]
    CameraMismatch,
    #[error("a pending operation is already in flight")]
    AlreadyPending,
}

#[derive(Debug, Clone, Error)]
pub(in crate::ui::tabs::backup) enum BackupAnomaly {
    #[error("backup export response arrived for req {req} we didn't issue")]
    UnexpectedExport { req: ReqId },
    #[error("backup import response arrived for req {req} we didn't issue")]
    UnexpectedImport { req: ReqId },
    #[error("backup export response arrived but no device snapshot available")]
    NoDeviceSnapshot,
}

#[derive(Debug, Default)]
pub struct BackupTabState {
    list: BackupListPane,
    rename: Option<RenameState>,
    confirm: Option<PendingConfirm>,
    pending: PendingOps,
}

#[derive(Debug)]
pub(super) struct RenameState {
    pub(super) slug: Slug,
    pub(super) text: TextInputState,
}

#[derive(Debug)]
struct PendingConfirm {
    state: ConfirmState,
    action: ConfirmAction,
}

#[derive(Debug, Clone)]
enum ConfirmAction {
    Delete(Slug),
    Import(Slug),
}

impl ConfirmAction {
    const fn targets_device(&self) -> bool {
        matches!(self, Self::Import(_))
    }
}

#[derive(Debug, Default)]
struct PendingOps {
    export: Option<ReqId>,
    import: Option<ReqId>,
}

impl BackupTabState {
    pub(super) fn draw(&mut self, ctx: &AppCtx, frame: &mut Frame, area: Rect) {
        self.list.draw(frame, area, ctx, self.rename.as_ref());

        if let Some(confirm) = self.confirm.as_ref() {
            confirm.state.draw(frame, area);
        }
    }

    pub(super) const fn capturing_input(&self) -> bool {
        self.list.filtering() || self.rename.is_some() || self.confirm.is_some()
    }

    #[allow(clippy::unused_self)]
    pub(super) const fn keybinds(&self) -> &'static [Keybind] {
        LIST_KEYBINDS
    }

    pub fn handle_key(&mut self, ctx: &AppCtx, key: KeyEvent) {
        if self.rename.is_some() {
            self.handle_rename_key(ctx, key);
            return;
        }
        if self.confirm.is_some() {
            self.handle_confirm_key(ctx, key);
            return;
        }
        if self.list.filtering() {
            if self.list.handle_filter_key(key) {
                self.settle(ctx);
            }
            return;
        }
        self.handle_list_key(ctx, key);
    }

    fn handle_list_key(&mut self, ctx: &AppCtx, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let order = self.cursor_order(ctx);
                self.list.step(CursorMove::Up, &order);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let order = self.cursor_order(ctx);
                self.list.step(CursorMove::Down, &order);
            }
            KeyCode::Char('/') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.list.start_filter();
            }
            KeyCode::Char('e') => match self.request_export(ctx) {
                Ok(req) => debug!("{req}: backup export requested"),
                Err(reason) => debug!("backup export skipped: {reason}"),
            },
            KeyCode::Char('i') => match self.prompt_import(ctx) {
                Ok(()) => debug!("backup import prompt opened"),
                Err(reason) => debug!("backup import skipped: {reason}"),
            },
            KeyCode::Char('r') => match self.prompt_rename(ctx) {
                Ok(()) => debug!("backup rename prompt opened"),
                Err(reason) => debug!("backup rename skipped: {reason}"),
            },
            KeyCode::Char('D') | KeyCode::Delete => match self.prompt_delete(ctx) {
                Ok(()) => debug!("backup delete prompt opened"),
                Err(reason) => debug!("backup delete skipped: {reason}"),
            },
            _ => {}
        }
    }

    fn handle_rename_key(&mut self, ctx: &AppCtx, key: KeyEvent) {
        let rename = self.rename.as_mut().expect("guarded");
        match key.code {
            KeyCode::Esc => {
                self.rename = None;
            }
            KeyCode::Enter => {
                let name = rename.text.buffer.trim().to_owned();
                if name.is_empty() {
                    return;
                }
                let slug = rename.slug.clone();
                let req = ctx.req.next();
                info!("{req}: renaming backup {slug} to {name}");
                ctx.fs.send(FsCommand::RenameBackup {
                    req,
                    slug,
                    new_name: name,
                });
                self.rename = None;
            }
            _ => {
                rename.text.handle_edit_key(key);
            }
        }
    }

    fn handle_confirm_key(&mut self, ctx: &AppCtx, key: KeyEvent) {
        match ConfirmState::handle_key(key) {
            ConfirmOutcome::Pending => {}
            ConfirmOutcome::Cancelled => self.confirm = None,
            ConfirmOutcome::Confirmed => {
                let action = self.confirm.take().expect("guarded").action;
                match action {
                    ConfirmAction::Delete(slug) => {
                        let req = ctx.req.next();
                        info!("{req}: deleting backup {slug}");
                        ctx.fs.send(FsCommand::RemoveBackup { req, slug });
                    }
                    ConfirmAction::Import(slug) => {
                        if self.pending.import.is_some() {
                            debug!("backup import skipped: {}", ActionSkipError::AlreadyPending);
                            return;
                        }
                        let req = ctx.req.next();
                        self.pending.import = Some(req);
                        info!("{req}: reading backup blob for import: {slug}");
                        ctx.fs.send(FsCommand::ReadBackupBlob { req, slug });
                    }
                }
            }
        }
    }

    pub(super) fn request_export(&mut self, ctx: &AppCtx) -> Result<ReqId, ActionSkipError> {
        if self.pending.export.is_some() {
            return Err(ActionSkipError::AlreadyPending);
        }
        let device = ctx.device.as_ref().ok_or(ActionSkipError::NoDevice)?;
        let req = ctx.req.next();
        self.pending.export = Some(req);
        device.send(DeviceCommand::ExportBackup { req });
        Ok(req)
    }

    pub(super) fn prompt_import(&mut self, ctx: &AppCtx) -> Result<(), ActionSkipError> {
        let BackupCursor::Entry(slug) = self.list.selection() else {
            return Err(ActionSkipError::NoSelection);
        };
        let slug = slug.clone();
        let entry = ctx
            .backup_library_snapshot
            .entries
            .get(&slug)
            .ok_or(ActionSkipError::NoSelection)?;
        let connected = ctx
            .device_snapshot
            .as_ref()
            .map(|s| s.usb_id)
            .ok_or(ActionSkipError::NoDevice)?;
        if connected != entry.source_camera {
            return Err(ActionSkipError::CameraMismatch);
        }
        self.confirm = Some(PendingConfirm {
            state: ConfirmState {
                title: format!(" Import {} ", entry.name),
                message: format!(
                    "Restore camera from \"{}\"?\nThis overwrites the current settings.",
                    entry.name
                ),
            },
            action: ConfirmAction::Import(slug),
        });
        Ok(())
    }

    pub(super) fn prompt_rename(&mut self, ctx: &AppCtx) -> Result<(), ActionSkipError> {
        let BackupCursor::Entry(slug) = self.list.selection() else {
            return Err(ActionSkipError::NoSelection);
        };
        let slug = slug.clone();
        let entry = ctx
            .backup_library_snapshot
            .entries
            .get(&slug)
            .ok_or(ActionSkipError::NoSelection)?;
        self.rename = Some(RenameState {
            slug,
            text: TextInputState::new_with_max_len(entry.name.clone(), BACKUP_NAME_MAX_LEN),
        });
        Ok(())
    }

    pub(super) fn prompt_delete(&mut self, ctx: &AppCtx) -> Result<(), ActionSkipError> {
        let BackupCursor::Entry(slug) = self.list.selection() else {
            return Err(ActionSkipError::NoSelection);
        };
        let slug = slug.clone();
        let entry = ctx
            .backup_library_snapshot
            .entries
            .get(&slug)
            .ok_or(ActionSkipError::NoSelection)?;
        self.confirm = Some(PendingConfirm {
            state: ConfirmState {
                title: format!(" Delete {} ", entry.name),
                message: format!(
                    "Permanently delete \"{}\"?\nThis cannot be undone.",
                    entry.name
                ),
            },
            action: ConfirmAction::Delete(slug),
        });
        Ok(())
    }

    pub(super) fn handle_backup_exported(
        &mut self,
        ctx: &AppCtx,
        req: ReqId,
        blob: &[u8],
    ) -> Result<(), BackupAnomaly> {
        if self.pending.export != Some(req) {
            return Err(BackupAnomaly::UnexpectedExport { req });
        }
        self.pending.export = None;
        let source_camera = ctx
            .device_snapshot
            .as_ref()
            .map(|s| s.usb_id)
            .ok_or(BackupAnomaly::NoDeviceSnapshot)?;
        let name = Self::default_backup_name(&ctx.backup_library_snapshot);
        let save_req = ctx.req.next();
        info!("{save_req}: saving backup as {name} ({} bytes)", blob.len());
        ctx.fs.send(FsCommand::AddBackup {
            req: save_req,
            name,
            source_camera,
            blob: blob.to_vec(),
        });
        Ok(())
    }

    pub(super) fn handle_backup_export_failed(&mut self, req: ReqId) -> Result<(), BackupAnomaly> {
        if self.pending.export != Some(req) {
            return Err(BackupAnomaly::UnexpectedExport { req });
        }
        self.pending.export = None;
        Ok(())
    }

    pub(super) fn handle_backup_imported(&mut self, req: ReqId) -> Result<(), BackupAnomaly> {
        if self.pending.import != Some(req) {
            return Err(BackupAnomaly::UnexpectedImport { req });
        }
        self.pending.import = None;
        Ok(())
    }

    pub(super) fn handle_backup_import_failed(&mut self, req: ReqId) -> Result<(), BackupAnomaly> {
        if self.pending.import != Some(req) {
            return Err(BackupAnomaly::UnexpectedImport { req });
        }
        self.pending.import = None;
        Ok(())
    }

    pub(super) fn handle_backup_library_op_failed(&mut self, req: ReqId) {
        if self.pending.import == Some(req) {
            self.pending.import = None;
        }
        if self.pending.export == Some(req) {
            self.pending.export = None;
        }
    }

    pub(super) fn cancel_device_actions(&mut self) {
        if self
            .confirm
            .as_ref()
            .is_some_and(|c| c.action.targets_device())
        {
            self.confirm = None;
        }
    }

    pub(super) fn focus_entry(&mut self, slug: &Slug) {
        self.list.set_selection(BackupCursor::Entry(slug.clone()));
    }

    pub(super) fn settle(&mut self, ctx: &AppCtx) {
        let order = self.cursor_order(ctx);
        self.list.ensure_valid(&order);
    }

    fn cursor_order(&self, ctx: &AppCtx) -> Vec<BackupCursor> {
        self.list.order(ctx)
    }

    fn default_backup_name(snapshot: &BackupLibrarySnapshot) -> String {
        const FORMAT: &[time::format_description::BorrowedFormatItem<'_>] =
            time::macros::format_description!("[year]-[month]-[day]-[hour][minute][second]");

        let stamp = time::OffsetDateTime::now_utc()
            .format(&FORMAT)
            .expect("formatting OffsetDateTime never fails");
        let base = format!("backup-{stamp}");
        let base_slug = Slug::try_from(base.as_str());
        if base_slug.is_ok_and(|s| !snapshot.entries.contains_key(&s)) {
            return base;
        }
        for n in 2..u32::MAX {
            let candidate = format!("{base}-{n}");
            if let Ok(slug) = Slug::try_from(candidate.as_str())
                && !snapshot.entries.contains_key(&slug)
            {
                return candidate;
            }
        }
        base
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crossbeam_channel::unbounded;
    use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};
    use fujicore::{UsbId, generated::cameras::C_X_S20};
    use time::OffsetDateTime;

    use super::*;
    use crate::{
        ui::tabs::AppCtx,
        workers::{
            ReqIdGen,
            device::DeviceSnapshot,
            fs::{
                FsHandle,
                backup::{BackupLibraryEntry, BackupLibrarySnapshot},
                simulation::SimulationLibrarySnapshot,
            },
        },
    };

    fn key_press(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    fn typed(c: char) -> KeyEvent {
        key_press(KeyCode::Char(c))
    }

    fn cam_usb() -> UsbId {
        C_X_S20.usb_id
    }

    fn other_usb() -> UsbId {
        UsbId {
            vendor: 0xFFFF,
            product: 0xFFFF,
        }
    }

    fn snapshot_with(entries: Vec<(Slug, BackupLibraryEntry)>) -> Arc<BackupLibrarySnapshot> {
        let mut s = BackupLibrarySnapshot::default();
        for (k, v) in entries {
            s.entries.insert(k, v);
        }
        Arc::new(s)
    }

    fn sample(name: &str, source_camera: UsbId) -> BackupLibraryEntry {
        let now = OffsetDateTime::from_unix_timestamp(0).unwrap();
        BackupLibraryEntry {
            name: name.to_owned(),
            source_camera,
            created: now,
            modified: now,
        }
    }

    fn make_ctx(connected: Option<UsbId>, entries: Vec<(Slug, BackupLibraryEntry)>) -> AppCtx {
        let (tx, _rx) = unbounded();
        let dir = tempfile::tempdir().unwrap().keep();
        let fs = FsHandle::spawn(dir.join("sim"), dir.join("backups"), tx);
        AppCtx {
            device: None,
            fs,
            req: ReqIdGen::new(),
            device_snapshot: connected.map(|usb| DeviceSnapshot {
                name: "X-S20",
                usb_id: usb,
                bus_address: "0:0".to_owned(),
                battery: 100,
                capabilities: &[],
            }),
            simulation_library_snapshot: SimulationLibrarySnapshot::empty(),
            backup_library_snapshot: snapshot_with(entries),
        }
    }

    fn slug(s: &str) -> Slug {
        Slug::try_from(s).unwrap()
    }

    fn step(s: &mut BackupTabState, ctx: &AppCtx, dir: CursorMove) {
        let order = s.cursor_order(ctx);
        s.list.step(dir, &order);
    }

    #[test]
    fn list_shows_only_matching_camera_backups() {
        let mine = slug("mine");
        let other = slug("other");
        let ctx = make_ctx(
            Some(cam_usb()),
            vec![
                (mine.clone(), sample("Mine", cam_usb())),
                (other, sample("Other", other_usb())),
            ],
        );
        let mut s = BackupTabState::default();
        s.settle(&ctx);
        assert_eq!(s.list.selection(), &BackupCursor::Entry(mine));
    }

    #[test]
    fn list_empty_when_no_device() {
        let ctx = make_ctx(None, vec![(slug("x"), sample("X", cam_usb()))]);
        let mut s = BackupTabState::default();
        s.settle(&ctx);
        assert_eq!(s.list.selection(), &BackupCursor::None);
    }

    #[test]
    fn step_clamps_at_extremes() {
        let a = slug("a");
        let b = slug("b");
        let ctx = make_ctx(
            Some(cam_usb()),
            vec![
                (a.clone(), sample("A", cam_usb())),
                (b.clone(), sample("B", cam_usb())),
            ],
        );
        let mut s = BackupTabState::default();
        s.settle(&ctx);
        assert_eq!(s.list.selection(), &BackupCursor::Entry(a.clone()));
        step(&mut s, &ctx, CursorMove::Up);
        assert_eq!(s.list.selection(), &BackupCursor::Entry(a));
        step(&mut s, &ctx, CursorMove::Down);
        assert_eq!(s.list.selection(), &BackupCursor::Entry(b.clone()));
        step(&mut s, &ctx, CursorMove::Down);
        assert_eq!(s.list.selection(), &BackupCursor::Entry(b));
    }

    #[test]
    fn slash_opens_filter() {
        let ctx = make_ctx(Some(cam_usb()), vec![]);
        let mut s = BackupTabState::default();
        s.handle_key(&ctx, typed('/'));
        assert!(s.list.filtering());
    }

    #[test]
    fn capturing_input_reflects_filter() {
        let ctx = make_ctx(Some(cam_usb()), vec![]);
        let mut s = BackupTabState::default();
        assert!(!s.capturing_input());
        s.handle_key(&ctx, typed('/'));
        assert!(s.capturing_input());
    }

    #[test]
    fn delete_opens_confirm() {
        let a = slug("a");
        let ctx = make_ctx(Some(cam_usb()), vec![(a, sample("A", cam_usb()))]);
        let mut s = BackupTabState::default();
        s.settle(&ctx);
        s.handle_key(&ctx, key_press(KeyCode::Char('D')));
        assert!(s.confirm.is_some());
    }

    #[test]
    fn import_requires_matching_device() {
        let a = slug("a");
        let ctx = make_ctx(Some(other_usb()), vec![(a, sample("A", cam_usb()))]);
        let mut s = BackupTabState::default();
        s.settle(&ctx);
        let err = s.prompt_import(&ctx).unwrap_err();
        assert!(matches!(err, ActionSkipError::NoSelection));
    }

    #[test]
    fn commit_focuses_resulting_entry_over_prior_selection() {
        let new = slug("beta");
        let other = slug("alpha");
        let ctx = make_ctx(
            Some(cam_usb()),
            vec![
                (other.clone(), sample("Alpha", cam_usb())),
                (new.clone(), sample("Beta", cam_usb())),
            ],
        );
        let mut s = BackupTabState::default();
        s.list.set_selection(BackupCursor::Entry(other));
        s.focus_entry(&new);
        s.settle(&ctx);
        assert_eq!(s.list.selection(), &BackupCursor::Entry(new));
    }

    #[test]
    fn import_rejects_camera_mismatch_for_forced_selection() {
        let a = slug("a");
        let ctx = make_ctx(Some(cam_usb()), vec![(a.clone(), sample("A", other_usb()))]);
        let mut s = BackupTabState::default();
        s.list.set_selection(BackupCursor::Entry(a));
        let err = s.prompt_import(&ctx).unwrap_err();
        assert!(matches!(err, ActionSkipError::CameraMismatch));
    }

    #[test]
    fn export_without_device_errors() {
        let ctx = make_ctx(None, vec![]);
        let mut s = BackupTabState::default();
        let err = s.request_export(&ctx).unwrap_err();
        assert!(matches!(err, ActionSkipError::NoDevice));
    }

    #[test]
    fn confirm_cancelled_clears() {
        let a = slug("a");
        let ctx = make_ctx(Some(cam_usb()), vec![(a, sample("A", cam_usb()))]);
        let mut s = BackupTabState::default();
        s.settle(&ctx);
        s.handle_key(&ctx, key_press(KeyCode::Char('D')));
        assert!(s.confirm.is_some());
        s.handle_key(&ctx, key_press(KeyCode::Char('n')));
        assert!(s.confirm.is_none());
    }

    #[test]
    fn export_skipped_without_device_leaves_pending_clear() {
        let ctx = make_ctx(None, vec![]);
        let mut s = BackupTabState::default();
        s.handle_key(&ctx, typed('e'));
        assert!(s.pending.export.is_none());
    }

    #[test]
    fn unexpected_export_response_is_anomaly() {
        let ctx = make_ctx(Some(cam_usb()), vec![]);
        let mut s = BackupTabState::default();
        let bogus = ctx.req.next();
        let err = s.handle_backup_exported(&ctx, bogus, &[]).unwrap_err();
        assert!(matches!(err, BackupAnomaly::UnexpectedExport { .. }));
    }

    #[test]
    fn export_response_saves_without_rename_popup_or_optimistic_select() {
        let ctx = make_ctx(Some(cam_usb()), vec![]);
        let mut s = BackupTabState::default();
        let req = ctx.req.next();
        s.pending.export = Some(req);
        s.handle_backup_exported(&ctx, req, &[1, 2, 3]).unwrap();
        assert!(
            s.rename.is_none(),
            "new backup must not open the rename popup"
        );
        assert!(s.pending.export.is_none());
        assert_eq!(s.list.selection(), &BackupCursor::None);
    }

    #[test]
    fn added_entry_is_focused_on_completion() {
        let new = slug("backup-2026-06-09-101010");
        let mut s = BackupTabState::default();
        assert_eq!(s.list.selection(), &BackupCursor::None);
        s.focus_entry(&new);
        assert_eq!(s.list.selection(), &BackupCursor::Entry(new));
    }

    #[test]
    fn default_backup_name_disambiguates_on_collision() {
        use std::collections::BTreeMap;
        let mut entries = BTreeMap::new();
        let snap_empty = BackupLibrarySnapshot::default();
        let first = BackupTabState::default_backup_name(&snap_empty);
        let first_slug = Slug::try_from(first.as_str()).unwrap();
        entries.insert(first_slug, sample(&first, cam_usb()));
        let snap = BackupLibrarySnapshot { entries };
        let second = BackupTabState::default_backup_name(&snap);
        assert_ne!(first, second);
        assert!(second.ends_with("-2"));
    }
}
