pub mod editor;
pub mod list;

use std::{collections::BTreeMap, sync::Arc};

use crossterm::event::{KeyCode, KeyEvent};
use fujicore::{
    CoreError, SupportedCamera,
    features::simulation::SimulationDescriptors,
    generated::{options::CustomSetting, simulations::SimulationBase},
};
use log::{debug, warn};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};
use thiserror::Error;

use crate::{
    ui::tabs::{AppCtx, Buffer, Shadowed, TabBehavior},
    workers::{
        ReqId, ReqIdGen,
        device::{DeviceCommand, DeviceHandle},
        fs::library::{LibraryEntry, LibrarySnapshot, Slug},
    },
};

pub(super) const INDENT: &str = "  ";
pub(super) const COL_SEPARATOR: &str = " ";
pub(super) const DIRTY_MARKER: &str = "*";

pub type SimulationState = Shadowed<SimulationBase>;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum SlotEntry {
    Loading,
    Loaded(Buffer<SimulationState>),
    Failed(Arc<CoreError>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlotsState {
    /// No fetch has been requested.
    #[default]
    Idle,
    /// Waiting for the device to echo back the slot list.
    Requested(ReqId),
    /// Slot list received; awaiting per-slot data.
    InFlight(ReqId),
    /// All slots have resolved.
    Loaded,
    /// Device dropped; refetch is permitted once a device reappears.
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FetchSkipError {
    #[error("no device connected")]
    NoDevice,
    #[error("connected camera has no simulation descriptors")]
    NoDescriptors,
    #[error("a fetch is already requested")]
    AlreadyRequested,
    #[error("a fetch is already in flight")]
    AlreadyInFlight,
    #[error("slots already loaded")]
    AlreadyLoaded,
}

#[derive(Debug, Clone, Error)]
pub enum SlotError {
    #[error("slots enumeration arrived in state {state:?} with req {req} we didn't issue")]
    UnexpectedEnumeration { state: SlotsState, req: ReqId },
    #[error("event arrived for unknown slot {0}")]
    UnknownSlot(CustomSetting),
    #[error("per-slot event arrived while no fetch was in flight")]
    NoFetchInFlight,
}

#[derive(Debug, Default)]
pub struct Slots {
    state: SlotsState,
    entries: Vec<(CustomSetting, SlotEntry)>,
    descriptors: Option<&'static SimulationDescriptors>,
}

impl Slots {
    pub fn request_fetch(
        &mut self,
        device: Option<&DeviceHandle>,
        camera: Option<&'static SupportedCamera>,
        req_gen: &ReqIdGen,
    ) -> Result<ReqId, FetchSkipError> {
        match self.state {
            SlotsState::Requested(_) => return Err(FetchSkipError::AlreadyRequested),
            SlotsState::InFlight(_) => return Err(FetchSkipError::AlreadyInFlight),
            SlotsState::Loaded => return Err(FetchSkipError::AlreadyLoaded),
            SlotsState::Idle | SlotsState::Stale => {}
        }
        let device = device.ok_or(FetchSkipError::NoDevice)?;
        let descriptors = camera
            .and_then(|c| c.simulation)
            .ok_or(FetchSkipError::NoDescriptors)?;
        let req = req_gen.next();
        debug!("{req}: fetching all slots");
        device.send(DeviceCommand::FetchAllSlots { req });
        self.state = SlotsState::Requested(req);
        self.descriptors = Some(descriptors);
        Ok(req)
    }

    pub fn on_enumerated(&mut self, req: ReqId, slots: &[CustomSetting]) -> Result<(), SlotError> {
        match self.state {
            SlotsState::Requested(r) if r == req => {}
            state => return Err(SlotError::UnexpectedEnumeration { state, req }),
        }
        self.entries = slots.iter().map(|s| (*s, SlotEntry::Loading)).collect();
        self.state = SlotsState::InFlight(req);
        Ok(())
    }

    pub fn on_fetched(
        &mut self,
        slot: CustomSetting,
        base: &SimulationBase,
    ) -> Result<(), SlotError> {
        if !matches!(self.state, SlotsState::InFlight(_)) {
            return Err(SlotError::NoFetchInFlight);
        }
        let state = SimulationState::from(base.clone());
        let buffer = Buffer::from(state);
        self.replace(slot, SlotEntry::Loaded(buffer))?;
        self.advance_if_all_resolved();
        Ok(())
    }

    pub fn on_fetch_failed(
        &mut self,
        slot: CustomSetting,
        error: Arc<CoreError>,
    ) -> Result<(), SlotError> {
        if !matches!(self.state, SlotsState::InFlight(_)) {
            return Err(SlotError::NoFetchInFlight);
        }
        self.replace(slot, SlotEntry::Failed(error))?;
        self.advance_if_all_resolved();
        Ok(())
    }

    pub const fn mark_stale(&mut self) {
        self.state = SlotsState::Stale;
    }

    fn replace(&mut self, slot: CustomSetting, entry: SlotEntry) -> Result<(), SlotError> {
        let existing = self
            .entries
            .iter_mut()
            .find(|(s, _)| *s == slot)
            .ok_or(SlotError::UnknownSlot(slot))?;
        existing.1 = entry;
        Ok(())
    }

    fn advance_if_all_resolved(&mut self) {
        if !matches!(self.state, SlotsState::InFlight(_)) {
            return;
        }
        if self
            .entries
            .iter()
            .all(|(_, e)| !matches!(e, SlotEntry::Loading))
        {
            self.state = SlotsState::Loaded;
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = CustomSetting> + '_ {
        self.entries.iter().map(|(s, _)| *s)
    }

    pub fn get(&self, slot: CustomSetting) -> Option<&SlotEntry> {
        self.entries
            .iter()
            .find(|(s, _)| *s == slot)
            .map(|(_, e)| e)
    }
}

impl<'a> IntoIterator for &'a Slots {
    type Item = (CustomSetting, &'a SlotEntry);
    type IntoIter = std::iter::Map<
        std::slice::Iter<'a, (CustomSetting, SlotEntry)>,
        fn(&'a (CustomSetting, SlotEntry)) -> (CustomSetting, &'a SlotEntry),
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter().map(|(s, e)| (*s, e))
    }
}

#[derive(Debug, Clone)]
pub struct LibraryBuffer {
    pub entry: LibraryEntry,
    pub buffer: Buffer<SimulationState>,
    pub descriptors: &'static SimulationDescriptors,
}

#[derive(Debug, Default)]
pub struct LibrarySyncReport {
    pub added: Vec<Slug>,
    pub removed: Vec<Slug>,
    pub updated: Vec<Slug>,
    pub updated_with_conflict: Vec<Slug>,
    pub unsupported: Vec<Slug>,
}

#[derive(Debug, Default)]
pub struct Library {
    entries: BTreeMap<Slug, LibraryBuffer>,
}

impl Library {
    pub fn sync(&mut self, snapshot: &LibrarySnapshot) -> LibrarySyncReport {
        let mut report = LibrarySyncReport::default();

        let removed: Vec<Slug> = self
            .entries
            .keys()
            .filter(|slug| !snapshot.entries.contains_key(*slug))
            .cloned()
            .collect();
        for slug in &removed {
            self.entries.remove(slug);
        }
        report.removed = removed;

        for (slug, entry) in &snapshot.entries {
            let Some(descriptors) = entry
                .source_camera
                .supported_camera()
                .and_then(|c| c.simulation)
            else {
                if self.entries.remove(slug).is_some() {
                    report.removed.push(slug.clone());
                }
                report.unsupported.push(slug.clone());
                continue;
            };

            match self.entries.get_mut(slug) {
                None => {
                    let state = SimulationState::from(entry.simulation.clone());
                    self.entries.insert(
                        slug.clone(),
                        LibraryBuffer {
                            entry: entry.clone(),
                            buffer: Buffer {
                                fetched: state.clone(),
                                working: state,
                            },
                            descriptors,
                        },
                    );
                    report.added.push(slug.clone());
                }
                Some(lib) if lib.buffer.fetched.canonical == entry.simulation => {
                    lib.entry = entry.clone();
                }
                Some(lib) => {
                    let has_unsaved_edits = lib.buffer.dirty();
                    let state = SimulationState::from(entry.simulation.clone());
                    lib.entry = entry.clone();
                    lib.buffer = Buffer {
                        fetched: state.clone(),
                        working: state,
                    };
                    if has_unsaved_edits {
                        report.updated_with_conflict.push(slug.clone());
                    } else {
                        report.updated.push(slug.clone());
                    }
                }
            }
        }

        report
    }

    pub fn keys(&self) -> impl Iterator<Item = &Slug> {
        self.entries.keys()
    }

    pub fn get(&self, slug: &Slug) -> Option<&LibraryBuffer> {
        self.entries.get(slug)
    }
}

impl<'a> IntoIterator for &'a Library {
    type Item = (&'a Slug, &'a LibraryBuffer);
    type IntoIter = std::collections::btree_map::Iter<'a, Slug, LibraryBuffer>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CursorMove {
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum SimulationCursor {
    #[default]
    None,
    Slot(CustomSetting),
    Library(Slug),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pane {
    #[default]
    List,
    Editor,
}

#[derive(Debug, Default)]
pub struct Cursor {
    pane: Pane,
    simulation: SimulationCursor,
    editors: BTreeMap<SimulationCursor, usize>,
}

impl Cursor {
    pub const fn pane(&self) -> Pane {
        self.pane
    }

    pub fn editor_field(&self) -> usize {
        self.editors.get(&self.simulation).copied().unwrap_or(0)
    }

    pub fn handle_key(&mut self, key: KeyEvent, order: &[SimulationCursor], max_field: usize) {
        match (self.pane, key.code) {
            (Pane::List, KeyCode::Up | KeyCode::Char('k')) => self.step_list(CursorMove::Up, order),
            (Pane::List, KeyCode::Down | KeyCode::Char('j')) => {
                self.step_list(CursorMove::Down, order);
            }
            (Pane::List, KeyCode::Enter) if max_field > 0 => self.pane = Pane::Editor,
            (Pane::Editor, KeyCode::Up | KeyCode::Char('k')) => {
                self.step_editor(CursorMove::Up, max_field);
            }
            (Pane::Editor, KeyCode::Down | KeyCode::Char('j')) => {
                self.step_editor(CursorMove::Down, max_field);
            }
            (Pane::Editor, KeyCode::Esc) => self.pane = Pane::List,
            _ => {}
        }
    }

    pub fn step_list(&mut self, dir: CursorMove, order: &[SimulationCursor]) {
        if order.is_empty() {
            self.simulation = SimulationCursor::None;
            return;
        }
        let current = order.iter().position(|c| c == &self.simulation);
        let target = match (current, dir) {
            (None, _) => 0,
            (Some(i), CursorMove::Up) => i.saturating_sub(1),
            (Some(i), CursorMove::Down) => (i + 1).min(order.len() - 1),
        };
        self.simulation = order[target].clone();
    }

    fn step_editor(&mut self, dir: CursorMove, max: usize) {
        if max == 0 || matches!(self.simulation, SimulationCursor::None) {
            return;
        }
        let current = self.editor_field().min(max - 1);
        let next = match dir {
            CursorMove::Up => current.saturating_sub(1),
            CursorMove::Down => (current + 1).min(max - 1),
        };
        self.editors.insert(self.simulation.clone(), next);
    }

    pub fn reset(&mut self, order: &[SimulationCursor]) {
        self.simulation = order.first().cloned().unwrap_or(SimulationCursor::None);
    }

    pub fn ensure_valid(&mut self, order: &[SimulationCursor]) {
        if matches!(self.simulation, SimulationCursor::None) || order.contains(&self.simulation) {
            return;
        }
        let first_or_none = || order.first().cloned().unwrap_or(SimulationCursor::None);
        self.simulation = match &self.simulation {
            SimulationCursor::Library(lost) => order
                .iter()
                .find(|c| matches!(c, SimulationCursor::Library(s) if s >= lost))
                .or_else(|| {
                    order
                        .iter()
                        .rev()
                        .find(|c| matches!(c, SimulationCursor::Library(s) if s < lost))
                })
                .cloned()
                .unwrap_or_else(first_or_none),
            SimulationCursor::Slot(_) => order
                .iter()
                .find(|c| matches!(c, SimulationCursor::Slot(_)))
                .cloned()
                .unwrap_or_else(first_or_none),
            SimulationCursor::None => SimulationCursor::None,
        };
    }
}

#[derive(Debug)]
pub enum Focused<'a> {
    Slot {
        slot: CustomSetting,
        entry: &'a SlotEntry,
        descriptors: &'static SimulationDescriptors,
    },
    Library {
        lib: &'a LibraryBuffer,
        descriptors: &'static SimulationDescriptors,
    },
}

#[derive(Debug, Default)]
pub struct SimulationTabState {
    slots: Slots,
    library: Library,
    cursor: Cursor,
}

impl SimulationTabState {
    pub const fn list_cursor(&self) -> &SimulationCursor {
        &self.cursor.simulation
    }

    pub const fn pane(&self) -> Pane {
        self.cursor.pane()
    }

    pub fn editor_cursor(&self) -> Option<usize> {
        let count = self.visible_field_count();
        if count == 0 {
            return None;
        }
        Some(self.cursor.editor_field().min(count - 1))
    }

    pub fn slot_entries(&self) -> impl Iterator<Item = (CustomSetting, &SlotEntry)> {
        (&self.slots).into_iter()
    }

    pub fn library_entries(&self) -> impl Iterator<Item = (&Slug, &LibraryBuffer)> {
        (&self.library).into_iter()
    }

    fn visible_field_count(&self) -> usize {
        let (descriptors, canonical) = match self.focused() {
            Some(Focused::Slot {
                entry: SlotEntry::Loaded(buf),
                descriptors,
                ..
            }) => (descriptors, &buf.working.canonical),
            Some(Focused::Library { lib, descriptors }) => {
                (descriptors, &lib.buffer.working.canonical)
            }
            _ => return 0,
        };
        descriptors
            .fields
            .iter()
            .filter(|f| (f.display)(canonical).is_some())
            .count()
    }

    pub fn focused(&self) -> Option<Focused<'_>> {
        match &self.cursor.simulation {
            SimulationCursor::None => None,
            SimulationCursor::Slot(slot) => {
                let entry = self.slots.get(*slot)?;
                Some(Focused::Slot {
                    slot: *slot,
                    entry,
                    descriptors: self.slots.descriptors?,
                })
            }
            SimulationCursor::Library(slug) => {
                let lib = self.library.get(slug)?;
                Some(Focused::Library {
                    lib,
                    descriptors: lib.descriptors,
                })
            }
        }
    }

    fn request_fetch(
        &mut self,
        device: Option<&DeviceHandle>,
        camera: Option<&'static SupportedCamera>,
        req_gen: &ReqIdGen,
    ) -> Result<ReqId, FetchSkipError> {
        self.slots.request_fetch(device, camera, req_gen)
    }

    const fn mark_stale(&mut self) {
        self.slots.mark_stale();
    }

    fn apply_enumeration(&mut self, req: ReqId, slots: &[CustomSetting]) -> Result<(), SlotError> {
        self.slots.on_enumerated(req, slots)?;
        let order = self.cursor_order();
        match &self.cursor.simulation {
            SimulationCursor::None => self.cursor.reset(&order),
            SimulationCursor::Slot(_) => self.cursor.ensure_valid(&order),
            SimulationCursor::Library(_) => {}
        }
        Ok(())
    }

    fn apply_slot_fetched(
        &mut self,
        slot: CustomSetting,
        base: &SimulationBase,
    ) -> Result<(), SlotError> {
        self.slots.on_fetched(slot, base)
    }

    fn apply_slot_fetch_failure(
        &mut self,
        slot: CustomSetting,
        error: Arc<CoreError>,
    ) -> Result<(), SlotError> {
        self.slots.on_fetch_failed(slot, error)
    }

    fn sync_library(&mut self, snapshot: &LibrarySnapshot) -> LibrarySyncReport {
        let report = self.library.sync(snapshot);
        let order = self.cursor_order();
        self.cursor.ensure_valid(&order);
        report
    }

    fn cursor_order(&self) -> Vec<SimulationCursor> {
        let mut out = Vec::with_capacity(self.slots.entries.len() + self.library.entries.len());
        for slot in self.slots.keys() {
            out.push(SimulationCursor::Slot(slot));
        }
        for slug in self.library.keys() {
            out.push(SimulationCursor::Library(slug.clone()));
        }
        out
    }
}

impl TabBehavior for SimulationTabState {
    fn render(&self, ctx: &AppCtx, frame: &mut Frame, area: Rect) {
        let [list_area, editor_area] =
            Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
                .areas(area);
        self.render_list(frame, list_area);
        self.render_editor(ctx, frame, editor_area);
    }

    fn on_activate(&mut self, ctx: &AppCtx) {
        let camera = ctx
            .device_snapshot
            .as_ref()
            .and_then(|s| s.usb_id.supported_camera());
        match self.request_fetch(ctx.device.as_ref(), camera, &ctx.req) {
            Ok(req) => debug!("{req}: slot fetch requested"),
            Err(reason) => debug!("slot fetch skipped: {reason}"),
        }
    }

    fn on_device_connected(&mut self, ctx: &AppCtx) {
        let camera = ctx
            .device_snapshot
            .as_ref()
            .and_then(|s| s.usb_id.supported_camera());
        match self.request_fetch(ctx.device.as_ref(), camera, &ctx.req) {
            Ok(req) => debug!("{req}: slot fetch requested"),
            Err(reason) => debug!("slot fetch skipped: {reason}"),
        }
    }

    fn on_device_disconnected(&mut self, _ctx: &AppCtx) {
        self.mark_stale();
    }

    fn on_library_changed(&mut self, ctx: &AppCtx) {
        let report = self.sync_library(&ctx.library_snapshot);
        if !report.updated_with_conflict.is_empty() {
            warn!(
                "library: external changed for entries with unsaved edits: {:?}",
                report.updated_with_conflict
            );
        }
    }

    fn on_slots_enumerated(&mut self, _ctx: &AppCtx, req: ReqId, slots: &[CustomSetting]) {
        if let Err(anomaly) = self.apply_enumeration(req, slots) {
            warn!("slot enumeration anomaly: {anomaly}");
        }
    }

    fn on_slot_fetched(&mut self, _ctx: &AppCtx, slot: CustomSetting, base: &SimulationBase) {
        if let Err(anomaly) = self.apply_slot_fetched(slot, base) {
            warn!("slot fetched anomaly ({slot}): {anomaly}");
        }
    }

    fn on_slot_fetch_failed(&mut self, _ctx: &AppCtx, slot: CustomSetting, error: &Arc<CoreError>) {
        if let Err(anomaly) = self.apply_slot_fetch_failure(slot, Arc::clone(error)) {
            warn!("slot fetch-failed anomaly ({slot}): {anomaly}");
        }
    }

    fn handle_key(&mut self, _ctx: &AppCtx, key: KeyEvent) {
        let order = self.cursor_order();
        let max_field = self.visible_field_count();
        self.cursor.handle_key(key, &order, max_field);
    }
}

#[cfg(test)]
mod tests {
    use fujicore::{
        UsbId,
        generated::{options::FilmSimulation, simulations::SimulationBase},
    };
    use time::OffsetDateTime;

    use super::*;
    use crate::workers::ReqIdGen;

    fn snapshot_with(entries: Vec<(Slug, LibraryEntry)>) -> LibrarySnapshot {
        let mut s = LibrarySnapshot::default();
        for (k, v) in entries {
            s.entries.insert(k, v);
        }
        s
    }

    fn sample_entry(name: &str, sim: SimulationBase) -> LibraryEntry {
        let now = OffsetDateTime::now_utc();
        LibraryEntry {
            name: name.to_owned(),
            description: None,
            source_camera: UsbId {
                vendor: 0x04CB,
                product: 0x02FC,
            },
            created: now,
            modified: now,
            simulation: sim,
        }
    }

    fn req() -> ReqId {
        ReqIdGen::new().next()
    }

    fn enumerate(s: &mut SimulationTabState, slots: &[CustomSetting]) -> ReqId {
        let r = req();
        s.slots.state = SlotsState::Requested(r);
        s.apply_enumeration(r, slots).unwrap();
        r
    }

    fn step(s: &mut SimulationTabState, dir: CursorMove) {
        let order = s.cursor_order();
        s.cursor.step_list(dir, &order);
    }

    #[test]
    fn move_cursor_clamps_at_extremes() {
        let mut s = SimulationTabState::default();
        enumerate(&mut s, &[CustomSetting::C1, CustomSetting::C2]);
        assert_eq!(
            s.cursor.simulation,
            SimulationCursor::Slot(CustomSetting::C1)
        );
        step(&mut s, CursorMove::Up);
        assert_eq!(
            s.cursor.simulation,
            SimulationCursor::Slot(CustomSetting::C1)
        );
        step(&mut s, CursorMove::Down);
        assert_eq!(
            s.cursor.simulation,
            SimulationCursor::Slot(CustomSetting::C2)
        );
        step(&mut s, CursorMove::Down);
        assert_eq!(
            s.cursor.simulation,
            SimulationCursor::Slot(CustomSetting::C2)
        );
    }

    #[test]
    fn cursor_traverses_into_library() {
        let mut s = SimulationTabState::default();
        enumerate(&mut s, &[CustomSetting::C1]);
        let slug = Slug::try_from("velvia-warm").unwrap();
        s.sync_library(&snapshot_with(vec![(
            slug.clone(),
            sample_entry("Velvia Warm", SimulationBase::default()),
        )]));
        step(&mut s, CursorMove::Down);
        assert_eq!(s.cursor.simulation, SimulationCursor::Library(slug));
    }

    #[test]
    fn on_slot_fetched_marks_loaded_when_all_resolve() {
        let mut s = SimulationTabState::default();
        enumerate(&mut s, &[CustomSetting::C1, CustomSetting::C2]);
        assert!(matches!(s.slots.state, SlotsState::InFlight(_)));
        s.apply_slot_fetched(CustomSetting::C1, &SimulationBase::default())
            .unwrap();
        assert!(matches!(s.slots.state, SlotsState::InFlight(_)));
        s.apply_slot_fetched(CustomSetting::C2, &SimulationBase::default())
            .unwrap();
        assert_eq!(s.slots.state, SlotsState::Loaded);
    }

    #[test]
    fn request_fetch_skips_when_no_device() {
        let mut s = SimulationTabState::default();
        let req_gen = ReqIdGen::new();
        assert_eq!(
            s.request_fetch(None, None, &req_gen),
            Err(FetchSkipError::NoDevice)
        );
        assert_eq!(s.slots.state, SlotsState::Idle);
    }

    #[test]
    fn request_fetch_blocks_after_loaded() {
        let mut s = SimulationTabState::default();
        s.slots.state = SlotsState::Loaded;
        let req_gen = ReqIdGen::new();
        assert_eq!(
            s.request_fetch(None, None, &req_gen),
            Err(FetchSkipError::AlreadyLoaded)
        );
    }

    #[test]
    fn mark_stale_unblocks_request() {
        let mut s = SimulationTabState::default();
        s.slots.state = SlotsState::Loaded;
        s.mark_stale();
        assert_eq!(s.slots.state, SlotsState::Stale);
        let req_gen = ReqIdGen::new();
        assert_eq!(
            s.request_fetch(None, None, &req_gen),
            Err(FetchSkipError::NoDevice)
        );
    }

    #[test]
    fn on_enumerated_rejects_mismatched_req() {
        let mut s = SimulationTabState::default();
        let req_gen = ReqIdGen::new();
        let issued = req_gen.next();
        let other = req_gen.next();
        s.slots.state = SlotsState::Requested(issued);
        let err = s
            .apply_enumeration(other, &[CustomSetting::C1])
            .unwrap_err();
        assert!(matches!(err, SlotError::UnexpectedEnumeration { .. }));
    }

    #[test]
    fn on_fetched_rejects_unknown_slot() {
        let mut s = SimulationTabState::default();
        enumerate(&mut s, &[CustomSetting::C1]);
        let err = s
            .apply_slot_fetched(CustomSetting::C2, &SimulationBase::default())
            .unwrap_err();
        assert!(matches!(err, SlotError::UnknownSlot(CustomSetting::C2)));
    }

    #[test]
    fn sync_library_preserves_unchanged_entries() {
        let mut s = SimulationTabState::default();
        let slug = Slug::try_from("entry-a").unwrap();
        let sim = SimulationBase {
            film_simulation: Some(FilmSimulation::Velvia),
            ..Default::default()
        };
        let entry = sample_entry("Entry A", sim);
        let first = s.sync_library(&snapshot_with(vec![(slug.clone(), entry.clone())]));
        assert_eq!(first.added, vec![slug.clone()]);
        let second = s.sync_library(&snapshot_with(vec![(slug, entry)]));
        assert!(second.added.is_empty());
        assert!(second.removed.is_empty());
        assert!(second.updated.is_empty());
        assert!(second.updated_with_conflict.is_empty());
    }

    #[test]
    fn sync_library_flags_conflict_when_user_has_edits() {
        let mut s = SimulationTabState::default();
        let slug = Slug::try_from("entry-a").unwrap();
        s.sync_library(&snapshot_with(vec![(
            slug.clone(),
            sample_entry("Entry A", SimulationBase::default()),
        )]));

        s.library
            .entries
            .get_mut(&slug)
            .unwrap()
            .buffer
            .working
            .canonical = SimulationBase {
            film_simulation: Some(FilmSimulation::Velvia),
            ..Default::default()
        };
        assert!(s.library.entries[&slug].buffer.dirty());

        let external_sim = SimulationBase {
            film_simulation: Some(FilmSimulation::Astia),
            ..Default::default()
        };
        let report = s.sync_library(&snapshot_with(vec![(
            slug.clone(),
            sample_entry("Entry A", external_sim),
        )]));
        assert_eq!(report.updated_with_conflict, vec![slug]);
    }

    #[test]
    fn sync_library_clears_cursor_when_last_entry_removed() {
        let mut s = SimulationTabState::default();
        let slug = Slug::try_from("entry-a").unwrap();
        s.sync_library(&snapshot_with(vec![(
            slug.clone(),
            sample_entry("Entry A", SimulationBase::default()),
        )]));
        step(&mut s, CursorMove::Down);
        assert_eq!(s.cursor.simulation, SimulationCursor::Library(slug));
        let report = s.sync_library(&LibrarySnapshot::default());
        assert_eq!(report.removed.len(), 1);
        assert_eq!(s.cursor.simulation, SimulationCursor::None);
    }

    #[test]
    fn focused_none_when_cursor_none() {
        let s = SimulationTabState::default();
        assert!(s.focused().is_none());
    }

    #[test]
    fn ensure_valid_jumps_to_next_library_neighbor_on_delete() {
        let mut s = SimulationTabState::default();
        let a = Slug::try_from("a").unwrap();
        let b = Slug::try_from("b").unwrap();
        let c = Slug::try_from("c").unwrap();
        s.sync_library(&snapshot_with(vec![
            (a.clone(), sample_entry("A", SimulationBase::default())),
            (b.clone(), sample_entry("B", SimulationBase::default())),
            (c.clone(), sample_entry("C", SimulationBase::default())),
        ]));
        step(&mut s, CursorMove::Down);
        step(&mut s, CursorMove::Down);
        assert_eq!(s.cursor.simulation, SimulationCursor::Library(b));
        s.sync_library(&snapshot_with(vec![
            (a, sample_entry("A", SimulationBase::default())),
            (c.clone(), sample_entry("C", SimulationBase::default())),
        ]));
        assert_eq!(s.cursor.simulation, SimulationCursor::Library(c));
    }

    #[test]
    fn ensure_valid_falls_back_to_previous_library_neighbor_when_last_deleted() {
        let mut s = SimulationTabState::default();
        let a = Slug::try_from("a").unwrap();
        let b = Slug::try_from("b").unwrap();
        s.sync_library(&snapshot_with(vec![
            (a.clone(), sample_entry("A", SimulationBase::default())),
            (b.clone(), sample_entry("B", SimulationBase::default())),
        ]));
        step(&mut s, CursorMove::Down);
        step(&mut s, CursorMove::Down);
        assert_eq!(s.cursor.simulation, SimulationCursor::Library(b));
        s.sync_library(&snapshot_with(vec![(
            a.clone(),
            sample_entry("A", SimulationBase::default()),
        )]));
        assert_eq!(s.cursor.simulation, SimulationCursor::Library(a));
    }

    #[test]
    fn library_buffer_dirty_is_false_after_sync() {
        let mut s = SimulationTabState::default();
        let slug = Slug::try_from("entry-a").unwrap();
        let sim = SimulationBase {
            film_simulation: Some(FilmSimulation::Velvia),
            ..Default::default()
        };
        s.sync_library(&snapshot_with(vec![(
            slug.clone(),
            sample_entry("Entry A", sim),
        )]));
        let lib = s.library.entries.get(&slug).unwrap();
        assert!(!lib.buffer.dirty());
    }
}
