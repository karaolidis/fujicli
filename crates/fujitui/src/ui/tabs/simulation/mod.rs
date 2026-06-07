pub mod editor;
pub mod list;

use std::{collections::BTreeMap, slice::Iter, sync::Arc};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fujicore::{
    CoreError, SupportedCamera,
    features::simulation::{
        Direction, EnumOps, Extreme, Magnitude, OptionDescriptor, OptionOps, SetOutcome,
        SimulationDescriptors,
    },
    generated::{
        options::{CustomSetting, CustomSettingName},
        simulations::SimulationBase,
    },
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
pub(super) const FILTER_PROMPT: &str = "> ";

pub type SimulationState = Shadowed<SimulationBase>;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum SlotEntry {
    Loading,
    Loaded(Buffer<SimulationState>),
    Failed(Arc<CoreError>),
}

impl SlotEntry {
    pub const fn name(&self) -> Option<&CustomSettingName> {
        match self {
            Self::Loaded(buf) => buf.working.canonical.custom_setting_name.as_ref(),
            _ => None,
        }
    }
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

    pub fn handle_enumerated(
        &mut self,
        req: ReqId,
        slots: &[CustomSetting],
    ) -> Result<(), SlotError> {
        match self.state {
            SlotsState::Requested(r) if r == req => {}
            state => return Err(SlotError::UnexpectedEnumeration { state, req }),
        }
        self.entries = slots.iter().map(|s| (*s, SlotEntry::Loading)).collect();
        self.state = SlotsState::InFlight(req);
        Ok(())
    }

    pub fn handle_fetched(
        &mut self,
        slot: CustomSetting,
        base: &SimulationBase,
    ) -> Result<(), SlotError> {
        if !matches!(self.state, SlotsState::InFlight(_)) {
            return Err(SlotError::NoFetchInFlight);
        }
        let shadow = self
            .descriptors
            .map_or_else(|| base.clone(), |d| d.new_shadow_from(base));
        let state = SimulationState {
            canonical: base.clone(),
            shadow,
        };
        let buffer = Buffer::from(state);
        self.replace(slot, SlotEntry::Loaded(buffer))?;
        self.advance_if_all_resolved();
        Ok(())
    }

    pub fn handle_fetch_failed(
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

    pub fn get_mut(&mut self, slot: CustomSetting) -> Option<&mut SlotEntry> {
        self.entries
            .iter_mut()
            .find(|(s, _)| *s == slot)
            .map(|(_, e)| e)
    }
}

impl<'a> IntoIterator for &'a Slots {
    type Item = (CustomSetting, &'a SlotEntry);
    type IntoIter = std::iter::Map<
        Iter<'a, (CustomSetting, SlotEntry)>,
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

            let state = SimulationState {
                canonical: entry.simulation.clone(),
                shadow: descriptors.new_shadow_from(&entry.simulation),
            };
            match self.entries.get_mut(slug) {
                None => {
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

    pub fn get_mut(&mut self, slug: &Slug) -> Option<&mut LibraryBuffer> {
        self.entries.get_mut(slug)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditorAction {
    Bump(Direction),
    BigBump(Direction),
    Jump(Extreme),
    EnterEditMode,
}

#[derive(Debug)]
pub(super) struct InlineEdit {
    pub descriptor: &'static OptionDescriptor<SimulationBase>,
    pub target: SimulationCursor,
    pub status: InlineStatus,
    pub kind: InlineKind,
}

#[derive(Debug)]
pub(super) enum InlineKind {
    TextInput(TextInputState),
    Picker(PickerState),
}

#[derive(Debug, Default)]
pub(super) struct TextInputState {
    pub buffer: String,
    pub cursor_col: usize,
}

#[derive(Debug)]
pub(super) struct PickerState {
    pub filter: String,
    pub cursor_row: usize,
    pub rows: Vec<PickerRow>,
}

impl PickerState {
    fn compute_rows(
        ops: &EnumOps<SimulationBase>,
        canonical: &SimulationBase,
        descriptors: &SimulationDescriptors,
    ) -> Vec<PickerRow> {
        let validator = descriptors.partial_validator();
        ops.variants
            .iter()
            .filter_map(|variant| {
                let mut probe = canonical.clone();
                matches!(
                    (ops.set_by_id)(&mut probe, variant.id, &validator),
                    SetOutcome::Set,
                )
                .then(|| PickerRow {
                    id: variant.id,
                    label: variant.name,
                    label_lower: variant.name.to_lowercase(),
                })
            })
            .collect()
    }

    pub(super) fn visible_rows(&self) -> Vec<&PickerRow> {
        if self.filter.is_empty() {
            return self.rows.iter().collect();
        }
        let needle = self.filter.to_lowercase();
        self.rows
            .iter()
            .filter(|r| r.label_lower.contains(&needle))
            .collect()
    }
}

impl InlineEdit {
    fn handle_key(&mut self, key: KeyEvent) -> EditModeOutcome {
        let Self {
            descriptor,
            status,
            kind,
            ..
        } = self;
        match kind {
            InlineKind::TextInput(text) => {
                let max_len = match &descriptor.ops {
                    OptionOps::String(ops) => ops.max_len.unwrap_or(usize::MAX),
                    _ => unreachable!("TextInput; descriptor.ops is OptionOps::String"),
                };
                match key.code {
                    KeyCode::Esc => EditModeOutcome::Cancel,
                    KeyCode::Enter => EditModeOutcome::CommitText(text.buffer.clone()),
                    KeyCode::Left => {
                        if text.cursor_col > 0 {
                            text.cursor_col -= 1;
                        }
                        EditModeOutcome::Continue
                    }
                    KeyCode::Right => {
                        let len = text.buffer.chars().count();
                        if text.cursor_col < len {
                            text.cursor_col += 1;
                        }
                        EditModeOutcome::Continue
                    }
                    KeyCode::Home => {
                        text.cursor_col = 0;
                        EditModeOutcome::Continue
                    }
                    KeyCode::End => {
                        text.cursor_col = text.buffer.chars().count();
                        EditModeOutcome::Continue
                    }
                    KeyCode::Backspace => {
                        if text.cursor_col > 0 {
                            let byte_start = text
                                .buffer
                                .char_indices()
                                .nth(text.cursor_col - 1)
                                .map_or(text.buffer.len(), |(i, _)| i);
                            let byte_end = text
                                .buffer
                                .char_indices()
                                .nth(text.cursor_col)
                                .map_or(text.buffer.len(), |(i, _)| i);
                            text.buffer.drain(byte_start..byte_end);
                            text.cursor_col -= 1;
                            *status = InlineStatus::Idle;
                        }
                        EditModeOutcome::Continue
                    }
                    KeyCode::Delete => {
                        let len = text.buffer.chars().count();
                        if text.cursor_col < len {
                            let byte_start = text
                                .buffer
                                .char_indices()
                                .nth(text.cursor_col)
                                .map_or(text.buffer.len(), |(i, _)| i);
                            let byte_end = text
                                .buffer
                                .char_indices()
                                .nth(text.cursor_col + 1)
                                .map_or(text.buffer.len(), |(i, _)| i);
                            text.buffer.drain(byte_start..byte_end);
                            *status = InlineStatus::Idle;
                        }
                        EditModeOutcome::Continue
                    }
                    KeyCode::Char(c) if !c.is_control() => {
                        let len = text.buffer.chars().count();
                        if len < max_len {
                            let byte_pos = text
                                .buffer
                                .char_indices()
                                .nth(text.cursor_col)
                                .map_or(text.buffer.len(), |(i, _)| i);
                            text.buffer.insert(byte_pos, c);
                            text.cursor_col += 1;
                            *status = InlineStatus::Idle;
                        }
                        EditModeOutcome::Continue
                    }
                    _ => EditModeOutcome::Continue,
                }
            }
            InlineKind::Picker(picker) => match key.code {
                KeyCode::Esc => EditModeOutcome::Cancel,
                KeyCode::Enter => picker
                    .visible_rows()
                    .get(picker.cursor_row)
                    .map_or(EditModeOutcome::Continue, |row| {
                        EditModeOutcome::CommitPick(row.id)
                    }),
                KeyCode::Up => {
                    picker.cursor_row = picker.cursor_row.saturating_sub(1);
                    EditModeOutcome::Continue
                }
                KeyCode::Down => {
                    let len = picker.visible_rows().len();
                    if len > 0 {
                        picker.cursor_row = (picker.cursor_row + 1).min(len - 1);
                    }
                    EditModeOutcome::Continue
                }
                KeyCode::Backspace => {
                    if picker.filter.pop().is_some() {
                        let len = picker.visible_rows().len();
                        picker.cursor_row = picker.cursor_row.min(len.saturating_sub(1));
                        *status = InlineStatus::Idle;
                    }
                    EditModeOutcome::Continue
                }
                KeyCode::Char(c) if !c.is_control() => {
                    picker.filter.push(c);
                    let len = picker.visible_rows().len();
                    picker.cursor_row = picker.cursor_row.min(len.saturating_sub(1));
                    *status = InlineStatus::Idle;
                    EditModeOutcome::Continue
                }
                _ => EditModeOutcome::Continue,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct PickerRow {
    pub id: &'static str,
    pub label: &'static str,
    pub label_lower: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum InlineStatus {
    #[default]
    Idle,
    Rejected,
}

enum EditModeOutcome {
    Continue,
    Cancel,
    CommitText(String),
    CommitPick(&'static str),
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

    pub(super) fn handle_key(
        &mut self,
        key: KeyEvent,
        order: &[SimulationCursor],
        max_field: usize,
    ) -> Option<EditorAction> {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
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
            (Pane::Editor, KeyCode::Left) => {
                let dir = Direction::Prev;
                return Some(if shift {
                    EditorAction::BigBump(dir)
                } else {
                    EditorAction::Bump(dir)
                });
            }
            (Pane::Editor, KeyCode::Right) => {
                let dir = Direction::Next;
                return Some(if shift {
                    EditorAction::BigBump(dir)
                } else {
                    EditorAction::Bump(dir)
                });
            }
            (Pane::Editor, KeyCode::Home) => return Some(EditorAction::Jump(Extreme::Min)),
            (Pane::Editor, KeyCode::End) => return Some(EditorAction::Jump(Extreme::Max)),
            (Pane::Editor, KeyCode::Enter) => return Some(EditorAction::EnterEditMode),
            _ => {}
        }
        None
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

    pub fn prune_editors(&mut self, order: &[SimulationCursor]) {
        self.editors.retain(|key, _| order.contains(key));
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

#[derive(Debug)]
pub(super) enum FocusedMut<'a> {
    Slot {
        buffer: &'a mut Buffer<SimulationState>,
        descriptors: &'static SimulationDescriptors,
    },
    Library {
        buffer: &'a mut Buffer<SimulationState>,
        descriptors: &'static SimulationDescriptors,
    },
}

impl<'a> FocusedMut<'a> {
    pub(super) const fn parts(
        self,
    ) -> (
        &'a mut Buffer<SimulationState>,
        &'static SimulationDescriptors,
    ) {
        match self {
            Self::Slot {
                buffer,
                descriptors,
            }
            | Self::Library {
                buffer,
                descriptors,
            } => (buffer, descriptors),
        }
    }
}

#[derive(Debug, Default)]
pub struct SimulationTabState {
    slots: Slots,
    library: Library,
    cursor: Cursor,
    editing: Option<InlineEdit>,
    filter: TextInputState,
    filtering: bool,
}

impl SimulationTabState {
    pub const fn list_cursor(&self) -> &SimulationCursor {
        &self.cursor.simulation
    }

    pub(super) const fn editing(&self) -> Option<&InlineEdit> {
        self.editing.as_ref()
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
        let needle = self.filter.buffer.to_lowercase();
        (&self.slots).into_iter().filter(move |(_, entry)| {
            if needle.is_empty() {
                return true;
            }
            entry
                .name()
                .is_some_and(|n| n.to_lowercase().contains(&needle))
        })
    }

    pub fn library_entries(&self) -> impl Iterator<Item = (&Slug, &LibraryBuffer)> {
        let needle = self.filter.buffer.to_lowercase();
        (&self.library).into_iter().filter(move |(_, lib)| {
            needle.is_empty() || lib.entry.name.to_lowercase().contains(&needle)
        })
    }

    pub(super) const fn filter(&self) -> &TextInputState {
        &self.filter
    }

    pub(super) const fn filtering(&self) -> bool {
        self.filtering
    }

    fn visible_field_count(&self) -> usize {
        let Some((descriptors, canonical)) = self.focused_canonical() else {
            return 0;
        };
        descriptors
            .fields
            .iter()
            .filter(|f| (f.display)(canonical).is_some())
            .count()
    }

    fn focused_canonical(&self) -> Option<(&SimulationDescriptors, &SimulationBase)> {
        match self.focused()? {
            Focused::Slot {
                entry: SlotEntry::Loaded(buf),
                descriptors,
                ..
            } => Some((descriptors, &buf.working.canonical)),
            Focused::Library { lib, descriptors } => {
                Some((descriptors, &lib.buffer.working.canonical))
            }
            Focused::Slot { .. } => None,
        }
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

    pub(super) fn focused_mut(&mut self) -> Option<FocusedMut<'_>> {
        let Self {
            slots,
            library,
            cursor,
            ..
        } = self;
        match &cursor.simulation {
            SimulationCursor::None => None,
            SimulationCursor::Slot(slot) => {
                let descriptors = slots.descriptors?;
                let entry = slots.get_mut(*slot)?;
                match entry {
                    SlotEntry::Loaded(buffer) => Some(FocusedMut::Slot {
                        buffer,
                        descriptors,
                    }),
                    _ => None,
                }
            }
            SimulationCursor::Library(slug) => {
                let lib = library.get_mut(slug)?;
                Some(FocusedMut::Library {
                    buffer: &mut lib.buffer,
                    descriptors: lib.descriptors,
                })
            }
        }
    }

    pub(super) fn handle_editor_action(&mut self, action: EditorAction) {
        let cursor_field = self.cursor.editor_field();
        let Some(desc) = self.focused_descriptor(cursor_field) else {
            return;
        };
        if matches!(action, EditorAction::EnterEditMode) {
            self.enter_edit_mode(desc);
            return;
        }
        let Some(focused) = self.focused_mut() else {
            return;
        };
        let (buffer, descriptors) = focused.parts();
        let outcome = {
            let validator = descriptors.partial_validator();
            let canonical = &mut buffer.working.canonical;
            match (&desc.ops, action) {
                (OptionOps::Enum(ops), EditorAction::Bump(d)) => {
                    (ops.cycle)(canonical, d, &validator)
                }
                (OptionOps::Integer(ops), EditorAction::Bump(d)) => {
                    (ops.step_fn)(canonical, d, Magnitude::Single, &validator)
                }
                (OptionOps::Integer(ops), EditorAction::BigBump(d)) => {
                    (ops.step_fn)(canonical, d, Magnitude::Big, &validator)
                }
                (OptionOps::Integer(ops), EditorAction::Jump(e)) => {
                    (ops.jump_fn)(canonical, e, &validator)
                }
                (OptionOps::Float(ops), EditorAction::Bump(d)) => {
                    (ops.step_fn)(canonical, d, Magnitude::Single, &validator)
                }
                (OptionOps::Float(ops), EditorAction::BigBump(d)) => {
                    (ops.step_fn)(canonical, d, Magnitude::Big, &validator)
                }
                (OptionOps::Float(ops), EditorAction::Jump(e)) => {
                    (ops.jump_fn)(canonical, e, &validator)
                }
                _ => return,
            }
        };
        if outcome.is_ok() {
            Self::mirror_and_settle(&mut buffer.working, desc, descriptors);
        }
    }

    fn focused_descriptor(
        &self,
        cursor_field: usize,
    ) -> Option<&'static OptionDescriptor<SimulationBase>> {
        let (descriptors, canonical) = self.focused_canonical()?;
        descriptors
            .visible_fields(canonical)
            .get(cursor_field)
            .copied()
    }

    fn enter_edit_mode(&mut self, desc: &'static OptionDescriptor<SimulationBase>) {
        let Some((descriptors, canonical)) = self.focused_canonical() else {
            return;
        };
        let target = self.cursor.simulation.clone();
        let kind = match &desc.ops {
            OptionOps::String(_) => {
                let buffer = (desc.display)(canonical).unwrap_or_default();
                let cursor_col = buffer.chars().count();
                Some(InlineKind::TextInput(TextInputState { buffer, cursor_col }))
            }
            OptionOps::Enum(ops) => {
                let rows = PickerState::compute_rows(ops, canonical, descriptors);
                let cursor_row = (desc.display)(canonical)
                    .as_deref()
                    .and_then(|cur| rows.iter().position(|r| r.label == cur))
                    .unwrap_or(0);
                Some(InlineKind::Picker(PickerState {
                    filter: String::new(),
                    cursor_row,
                    rows,
                }))
            }
            _ => None,
        };
        self.editing = kind.map(|kind| InlineEdit {
            descriptor: desc,
            target,
            status: InlineStatus::Idle,
            kind,
        });
    }

    fn handle_edit_mode_key(&mut self, key: KeyEvent) {
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        match editing.handle_key(key) {
            EditModeOutcome::Continue => {}
            EditModeOutcome::Cancel => self.editing = None,
            EditModeOutcome::CommitText(text) => self.commit_text(&text),
            EditModeOutcome::CommitPick(idx) => self.commit_pick(idx),
        }
    }

    fn handle_filter_key(&mut self, key: KeyEvent) {
        let filter = &mut self.filter;
        let mut order_dirty = false;
        let mut close = false;
        let mut clear = false;
        match key.code {
            KeyCode::Esc => {
                close = true;
                clear = !filter.buffer.is_empty();
            }
            KeyCode::Enter => close = true,
            KeyCode::Backspace => {
                if filter.buffer.is_empty() {
                    close = true;
                } else if filter.cursor_col > 0 {
                    let byte_start = filter
                        .buffer
                        .char_indices()
                        .nth(filter.cursor_col - 1)
                        .map_or(filter.buffer.len(), |(i, _)| i);
                    let byte_end = filter
                        .buffer
                        .char_indices()
                        .nth(filter.cursor_col)
                        .map_or(filter.buffer.len(), |(i, _)| i);
                    filter.buffer.drain(byte_start..byte_end);
                    filter.cursor_col -= 1;
                    order_dirty = true;
                }
            }
            KeyCode::Delete => {
                let len = filter.buffer.chars().count();
                if filter.cursor_col < len {
                    let byte_start = filter
                        .buffer
                        .char_indices()
                        .nth(filter.cursor_col)
                        .map_or(filter.buffer.len(), |(i, _)| i);
                    let byte_end = filter
                        .buffer
                        .char_indices()
                        .nth(filter.cursor_col + 1)
                        .map_or(filter.buffer.len(), |(i, _)| i);
                    filter.buffer.drain(byte_start..byte_end);
                    order_dirty = true;
                }
            }
            KeyCode::Left => {
                if filter.cursor_col > 0 {
                    filter.cursor_col -= 1;
                }
            }
            KeyCode::Right => {
                let len = filter.buffer.chars().count();
                if filter.cursor_col < len {
                    filter.cursor_col += 1;
                }
            }
            KeyCode::Home => filter.cursor_col = 0,
            KeyCode::End => filter.cursor_col = filter.buffer.chars().count(),
            KeyCode::Char(c) if !c.is_control() => {
                let byte_pos = filter
                    .buffer
                    .char_indices()
                    .nth(filter.cursor_col)
                    .map_or(filter.buffer.len(), |(i, _)| i);
                filter.buffer.insert(byte_pos, c);
                filter.cursor_col += 1;
                order_dirty = true;
            }
            _ => {}
        }
        if clear {
            self.filter.buffer.clear();
            self.filter.cursor_col = 0;
            order_dirty = true;
        }
        if close {
            self.filtering = false;
        }
        if order_dirty {
            let order = self.cursor_order();
            self.cursor.ensure_valid(&order);
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.editing.is_some() {
            self.handle_edit_mode_key(key);
            return;
        }
        if self.filtering {
            self.handle_filter_key(key);
            return;
        }
        if self.cursor.pane() == Pane::List
            && matches!(key.code, KeyCode::Char('/'))
            && !key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.filtering = true;
            self.filter.cursor_col = self.filter.buffer.chars().count();
            return;
        }
        let max_field = self.visible_field_count();
        let action = if self.cursor.pane() == Pane::List {
            let order = self.cursor_order();
            self.cursor.handle_key(key, &order, max_field)
        } else {
            self.cursor.handle_key(key, &[], max_field)
        };
        if let Some(action) = action {
            self.handle_editor_action(action);
        }
    }

    fn commit_text(&mut self, text: &str) {
        let Some(edit) = self.editing.as_ref() else {
            return;
        };
        if !self.cursor_matches_edit_target() {
            self.editing = None;
            return;
        }
        let desc = edit.descriptor;
        let Some(focused) = self.focused_mut() else {
            self.editing = None;
            return;
        };
        let (buffer, descriptors) = focused.parts();
        let OptionOps::String(ops) = &desc.ops else {
            unreachable!("commit_text; descriptor.ops is OptionOps::String");
        };
        let outcome = {
            let validator = descriptors.partial_validator();
            (ops.set_by_text)(&mut buffer.working.canonical, text, &validator)
        };
        if matches!(outcome, SetOutcome::Set) {
            Self::mirror_and_settle(&mut buffer.working, desc, descriptors);
        }
        self.finish_commit(&outcome, desc, |canonical| {
            let validator = |_b: SimulationBase| None;
            let _ = (ops.set_by_text)(canonical, text, &validator);
        });
    }

    fn commit_pick(&mut self, id: &'static str) {
        let Some(edit) = self.editing.as_ref() else {
            return;
        };
        if !self.cursor_matches_edit_target() {
            self.editing = None;
            return;
        }
        let desc = edit.descriptor;
        let Some(focused) = self.focused_mut() else {
            self.editing = None;
            return;
        };
        let (buffer, descriptors) = focused.parts();
        let OptionOps::Enum(ops) = &desc.ops else {
            unreachable!("commit_pick; descriptor.ops is OptionOps::Enum");
        };
        let outcome = {
            let validator = descriptors.partial_validator();
            (ops.set_by_id)(&mut buffer.working.canonical, id, &validator)
        };
        if matches!(outcome, SetOutcome::Set) {
            Self::mirror_and_settle(&mut buffer.working, desc, descriptors);
        }
        self.finish_commit(&outcome, desc, |canonical| {
            let validator = |_b: SimulationBase| None;
            let _ = (ops.set_by_id)(canonical, id, &validator);
        });
    }

    fn cursor_matches_edit_target(&self) -> bool {
        self.editing
            .as_ref()
            .is_some_and(|e| e.target == self.cursor.simulation)
    }

    fn finish_commit<F: FnOnce(&mut SimulationBase)>(
        &mut self,
        outcome: &SetOutcome,
        desc: &'static OptionDescriptor<SimulationBase>,
        write_rejected_to_shadow: F,
    ) {
        match outcome {
            SetOutcome::Set => self.editing = None,
            SetOutcome::InvalidInput(_) => {
                if let Some(edit) = self.editing.as_mut() {
                    edit.status = InlineStatus::Rejected;
                }
            }
            SetOutcome::Rejected => {
                if let Some(focused) = self.focused_mut() {
                    let (buffer, _) = focused.parts();
                    let mut probe = buffer.working.shadow.clone();
                    write_rejected_to_shadow(&mut probe);
                    (desc.copy_from)(&mut buffer.working.shadow, &probe);
                }
                if let Some(edit) = self.editing.as_mut() {
                    edit.status = InlineStatus::Rejected;
                }
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

    fn handle_slots_enumerated(
        &mut self,
        req: ReqId,
        slots: &[CustomSetting],
    ) -> Result<(), SlotError> {
        self.slots.handle_enumerated(req, slots)?;
        let order = self.cursor_order();
        match &self.cursor.simulation {
            SimulationCursor::None => self.cursor.reset(&order),
            SimulationCursor::Slot(_) => self.cursor.ensure_valid(&order),
            SimulationCursor::Library(_) => {}
        }
        self.cursor.prune_editors(&order);
        Ok(())
    }

    fn handle_slot_fetched(
        &mut self,
        slot: CustomSetting,
        base: &SimulationBase,
    ) -> Result<(), SlotError> {
        self.slots.handle_fetched(slot, base)
    }

    fn handle_slot_fetch_failed(
        &mut self,
        slot: CustomSetting,
        error: Arc<CoreError>,
    ) -> Result<(), SlotError> {
        self.slots.handle_fetch_failed(slot, error)
    }

    fn sync_library(&mut self, snapshot: &LibrarySnapshot) -> LibrarySyncReport {
        let report = self.library.sync(snapshot);
        let order = self.cursor_order();
        self.cursor.ensure_valid(&order);
        self.cursor.prune_editors(&order);
        report
    }

    fn cursor_order(&self) -> Vec<SimulationCursor> {
        let mut out = Vec::with_capacity(self.slots.entries.len() + self.library.entries.len());
        for (slot, _) in self.slot_entries() {
            out.push(SimulationCursor::Slot(slot));
        }
        for (slug, _) in self.library_entries() {
            out.push(SimulationCursor::Library(slug.clone()));
        }
        out
    }

    fn mirror_and_settle(
        state: &mut SimulationState,
        edited: &OptionDescriptor<SimulationBase>,
        descriptors: &SimulationDescriptors,
    ) {
        (edited.copy_from)(&mut state.shadow, &state.canonical);
        Self::settle(state, descriptors, Some(edited));
    }

    fn settle(
        state: &mut SimulationState,
        descriptors: &SimulationDescriptors,
        edited: Option<&OptionDescriptor<SimulationBase>>,
    ) {
        let delta: Vec<&'static OptionDescriptor<SimulationBase>> = descriptors
            .fields
            .iter()
            .copied()
            .filter(|d| !(d.eq)(&state.canonical, &state.shadow))
            .filter(|d| (d.display)(&state.shadow).is_some())
            .collect();
        if delta.is_empty() {
            return;
        }

        let pre_canonical = state.canonical.clone();
        let preserves_edit =
            |b: &SimulationBase| -> bool { edited.is_none_or(|e| (e.eq)(b, &pre_canonical)) };

        let mut combined = state.canonical.clone();
        for d in &delta {
            (d.copy_from)(&mut combined, &state.shadow);
        }
        if let Ok(v) = (descriptors.validate_partial)(combined)
            && preserves_edit(&v)
        {
            state.canonical = v;
            return;
        }

        for d in delta {
            let mut candidate = state.canonical.clone();
            (d.copy_from)(&mut candidate, &state.shadow);
            if let Ok(v) = (descriptors.validate_partial)(candidate)
                && preserves_edit(&v)
            {
                state.canonical = v;
            }
        }
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

    fn is_capturing_input(&self) -> bool {
        self.editing.is_some() || self.filtering
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
        if let Err(anomaly) = self.handle_slots_enumerated(req, slots) {
            warn!("slot enumeration anomaly: {anomaly}");
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

    fn on_key(&mut self, _ctx: &AppCtx, key: KeyEvent) {
        self.handle_key(key);
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use fujicore::{
        UsbId,
        features::simulation::Direction,
        generated::{
            cameras::C_X_S20_SIMULATION,
            options::{Clarity, CustomSettingName, FilmSimulation, MonochromaticColorTemperature},
            simulations::SimulationBase,
        },
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
        s.handle_slots_enumerated(r, slots).unwrap();
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
    fn handle_slot_fetched_marks_loaded_when_all_resolve() {
        let mut s = SimulationTabState::default();
        enumerate(&mut s, &[CustomSetting::C1, CustomSetting::C2]);
        assert!(matches!(s.slots.state, SlotsState::InFlight(_)));
        s.handle_slot_fetched(CustomSetting::C1, &SimulationBase::default())
            .unwrap();
        assert!(matches!(s.slots.state, SlotsState::InFlight(_)));
        s.handle_slot_fetched(CustomSetting::C2, &SimulationBase::default())
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
    fn handle_slots_enumerated_rejects_mismatched_req() {
        let mut s = SimulationTabState::default();
        let req_gen = ReqIdGen::new();
        let issued = req_gen.next();
        let other = req_gen.next();
        s.slots.state = SlotsState::Requested(issued);
        let err = s
            .handle_slots_enumerated(other, &[CustomSetting::C1])
            .unwrap_err();
        assert!(matches!(err, SlotError::UnexpectedEnumeration { .. }));
    }

    #[test]
    fn handle_slot_fetched_rejects_unknown_slot() {
        let mut s = SimulationTabState::default();
        enumerate(&mut s, &[CustomSetting::C1]);
        let err = s
            .handle_slot_fetched(CustomSetting::C2, &SimulationBase::default())
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

    fn loaded_slot(base: &SimulationBase) -> SimulationTabState {
        let mut s = SimulationTabState::default();
        s.slots.descriptors = Some(&C_X_S20_SIMULATION);
        let r = req();
        s.slots.state = SlotsState::Requested(r);
        s.handle_slots_enumerated(r, &[CustomSetting::C1]).unwrap();
        s.handle_slot_fetched(CustomSetting::C1, base).unwrap();
        s
    }

    fn focus_field(s: &mut SimulationTabState, field_name: &str) -> bool {
        let visible = {
            let Some((d, canon)) = s.focused_canonical() else {
                return false;
            };
            d.visible_fields(canon)
        };
        let Some(idx) = visible.iter().position(|f| f.name == field_name) else {
            return false;
        };
        s.cursor.pane = Pane::Editor;
        s.cursor.editors.insert(s.cursor.simulation.clone(), idx);
        true
    }

    fn key_press(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        }
    }

    fn seeded_string_base() -> SimulationBase {
        SimulationBase {
            custom_setting_name: Some(CustomSettingName::default()),
            ..Default::default()
        }
    }

    fn loaded_canonical(s: &SimulationTabState) -> &SimulationBase {
        match s.slots.get(CustomSetting::C1).unwrap() {
            SlotEntry::Loaded(buf) => &buf.working.canonical,
            _ => panic!("expected Loaded slot"),
        }
    }

    fn loaded_dirty(s: &SimulationTabState) -> bool {
        match s.slots.get(CustomSetting::C1).unwrap() {
            SlotEntry::Loaded(buf) => buf.dirty(),
            _ => panic!("expected Loaded slot"),
        }
    }

    #[test]
    fn editor_bump_changes_canonical_and_marks_dirty() {
        let mut s = loaded_slot(&SimulationBase {
            clarity: Some(Clarity::default()),
            ..Default::default()
        });
        assert!(focus_field(&mut s, "Clarity"));
        let before = loaded_canonical(&s).clarity;
        s.handle_editor_action(EditorAction::Bump(Direction::Next));
        let after = loaded_canonical(&s).clarity;
        assert_ne!(before, after);
        assert!(loaded_dirty(&s));
    }

    #[test]
    fn editor_bump_at_max_silent() {
        let at_max = Clarity::try_from(Clarity::MAX).unwrap();
        let mut s = loaded_slot(&SimulationBase {
            clarity: Some(at_max),
            ..Default::default()
        });
        assert!(focus_field(&mut s, "Clarity"));
        s.handle_editor_action(EditorAction::Bump(Direction::Next));
        assert_eq!(loaded_canonical(&s).clarity, Some(at_max));
        assert!(!loaded_dirty(&s));
    }

    #[test]
    fn enter_edit_mode_on_string_starts_text_input() {
        let mut s = loaded_slot(&seeded_string_base());
        assert!(focus_field(&mut s, "Custom Setting Name"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        assert!(matches!(
            s.editing,
            Some(InlineEdit {
                kind: InlineKind::TextInput(_),
                ..
            })
        ));
    }

    #[test]
    fn enter_edit_mode_on_enum_starts_picker_with_reachable_rows() {
        let mut s = loaded_slot(&SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        assert!(focus_field(&mut s, "Film Simulation"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        let Some(InlineEdit {
            kind: InlineKind::Picker(picker),
            ..
        }) = s.editing.as_ref()
        else {
            panic!("expected picker");
        };
        assert!(!picker.rows.is_empty());
    }

    #[test]
    fn picker_compute_rows_filters_out_variants_set_by_id_rejects() {
        use fujicore::features::simulation::{BumpError, EnumOps, VariantInfo};
        fn cycle_unused(
            _: &mut SimulationBase,
            _: fujicore::features::simulation::Direction,
            _: &fujicore::features::simulation::Validator<'_, SimulationBase>,
        ) -> Result<(), BumpError> {
            Err(BumpError::Exhausted)
        }
        fn rejects_b(
            _: &mut SimulationBase,
            id: &str,
            _: &fujicore::features::simulation::Validator<'_, SimulationBase>,
        ) -> SetOutcome {
            if id == "b" {
                SetOutcome::Rejected
            } else {
                SetOutcome::Set
            }
        }
        const STUB_OPS: EnumOps<SimulationBase> = EnumOps {
            variants: &[
                VariantInfo { id: "a", name: "A" },
                VariantInfo { id: "b", name: "B" },
                VariantInfo { id: "c", name: "C" },
            ],
            cycle: cycle_unused,
            set_by_id: rejects_b,
            set_default: |_| {},
        };
        const STUB_DESCRIPTORS: SimulationDescriptors = SimulationDescriptors {
            fields: &[],
            validate: |b| Ok(b),
            validate_partial: |b| Ok(b),
        };
        let rows =
            PickerState::compute_rows(&STUB_OPS, &SimulationBase::default(), &STUB_DESCRIPTORS);
        let ids: Vec<&str> = rows.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec!["a", "c"]);
    }

    #[test]
    fn picker_row_carries_canonical_id_distinct_from_display_label() {
        let mut s = loaded_slot(&SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        assert!(focus_field(&mut s, "Film Simulation"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        let Some(InlineEdit {
            kind: InlineKind::Picker(picker),
            ..
        }) = s.editing.as_ref()
        else {
            panic!("expected picker");
        };
        let velvia = picker
            .rows
            .iter()
            .find(|r| r.label == "Velvia")
            .expect("Velvia present");
        assert_eq!(velvia.id, "velvia");
    }

    #[test]
    fn commit_pick_lands_via_canonical_id() {
        let mut s = loaded_slot(&SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        assert!(focus_field(&mut s, "Film Simulation"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        s.commit_pick("velvia");
        assert_eq!(
            loaded_canonical(&s).film_simulation,
            Some(FilmSimulation::Velvia),
        );
    }

    #[test]
    fn picker_visible_rows_substring_match() {
        let rows = vec![
            PickerRow {
                id: "provia",
                label: "Provia",
                label_lower: "provia".to_owned(),
            },
            PickerRow {
                id: "velvia",
                label: "Velvia",
                label_lower: "velvia".to_owned(),
            },
            PickerRow {
                id: "astia",
                label: "Astia",
                label_lower: "astia".to_owned(),
            },
        ];
        let with_filter = |filter: &str| PickerState {
            filter: filter.to_owned(),
            cursor_row: 0,
            rows: rows.clone(),
        };
        let labels = |s: &PickerState| -> Vec<&'static str> {
            s.visible_rows().iter().map(|r| r.label).collect()
        };
        assert_eq!(labels(&with_filter("")), vec!["Provia", "Velvia", "Astia"]);
        assert_eq!(labels(&with_filter("vel")), vec!["Velvia"]);
        assert_eq!(
            labels(&with_filter("ia")),
            vec!["Provia", "Velvia", "Astia"]
        );
        assert_eq!(labels(&with_filter("VEL")), vec!["Velvia"]);
        assert!(with_filter("xyz").visible_rows().is_empty());
    }

    #[test]
    fn text_input_typing_updates_buffer() {
        let mut s = loaded_slot(&seeded_string_base());
        assert!(focus_field(&mut s, "Custom Setting Name"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        s.handle_edit_mode_key(key_press(KeyCode::Char('A')));
        s.handle_edit_mode_key(key_press(KeyCode::Char('B')));
        let Some(InlineEdit {
            kind: InlineKind::TextInput(text),
            ..
        }) = s.editing.as_ref()
        else {
            panic!("expected text input");
        };
        assert!(text.buffer.ends_with("AB"));
        assert_eq!(text.cursor_col, text.buffer.chars().count());
    }

    #[test]
    fn text_input_backspace_deletes() {
        let mut s = loaded_slot(&seeded_string_base());
        assert!(focus_field(&mut s, "Custom Setting Name"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        s.handle_edit_mode_key(key_press(KeyCode::Char('A')));
        s.handle_edit_mode_key(key_press(KeyCode::Backspace));
        let Some(InlineEdit {
            kind: InlineKind::TextInput(text),
            ..
        }) = s.editing.as_ref()
        else {
            panic!("expected text input");
        };
        assert!(!text.buffer.ends_with('A'));
    }

    #[test]
    fn text_input_esc_cancels() {
        let mut s = loaded_slot(&seeded_string_base());
        assert!(focus_field(&mut s, "Custom Setting Name"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        s.handle_edit_mode_key(key_press(KeyCode::Esc));
        assert!(s.editing.is_none());
    }

    #[test]
    fn picker_starts_on_currently_set_variant() {
        let mut s = loaded_slot(&SimulationBase {
            film_simulation: Some(FilmSimulation::Velvia),
            ..Default::default()
        });
        assert!(focus_field(&mut s, "Film Simulation"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        let Some(InlineEdit {
            kind: InlineKind::Picker(picker),
            ..
        }) = s.editing.as_ref()
        else {
            panic!("expected picker");
        };
        assert!(picker.cursor_row > 0);
        assert_eq!(picker.rows[picker.cursor_row].label, "Velvia");
    }

    #[test]
    fn picker_arrows_walk_visible_rows() {
        let mut s = loaded_slot(&SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        assert!(focus_field(&mut s, "Film Simulation"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        s.handle_edit_mode_key(key_press(KeyCode::Down));
        let Some(InlineEdit {
            kind: InlineKind::Picker(picker),
            ..
        }) = s.editing.as_ref()
        else {
            panic!("expected picker");
        };
        assert_eq!(picker.cursor_row, 1);
        s.handle_edit_mode_key(key_press(KeyCode::Up));
        let Some(InlineEdit {
            kind: InlineKind::Picker(picker),
            ..
        }) = s.editing.as_ref()
        else {
            panic!("expected picker");
        };
        assert_eq!(picker.cursor_row, 0);
    }

    #[test]
    fn picker_filter_typing_clamps_cursor() {
        let mut s = loaded_slot(&SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        assert!(focus_field(&mut s, "Film Simulation"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        s.handle_edit_mode_key(key_press(KeyCode::Down));
        s.handle_edit_mode_key(key_press(KeyCode::Down));
        for c in "Velv".chars() {
            s.handle_edit_mode_key(key_press(KeyCode::Char(c)));
        }
        let Some(InlineEdit {
            kind: InlineKind::Picker(picker),
            ..
        }) = s.editing.as_ref()
        else {
            panic!("expected picker");
        };
        assert_eq!(picker.filter, "Velv");
        assert_eq!(picker.cursor_row, 0);
    }

    #[test]
    fn picker_enter_commits_when_pick_set() {
        let mut s = loaded_slot(&SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        assert!(focus_field(&mut s, "Film Simulation"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        s.handle_edit_mode_key(key_press(KeyCode::Down));
        s.handle_edit_mode_key(key_press(KeyCode::Enter));
        assert!(s.editing.is_none());
        assert_ne!(
            loaded_canonical(&s).film_simulation,
            Some(FilmSimulation::Provia),
        );
        assert!(loaded_dirty(&s));
    }

    #[test]
    fn is_capturing_input_reflects_edit_state() {
        let mut s = loaded_slot(&seeded_string_base());
        assert!(!s.is_capturing_input());
        assert!(focus_field(&mut s, "Custom Setting Name"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        assert!(s.is_capturing_input());
        s.handle_edit_mode_key(key_press(KeyCode::Esc));
        assert!(!s.is_capturing_input());
    }

    #[test]
    fn global_action_mapper_would_swallow_digit_without_bypass() {
        let key = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
        let action = crate::ui::actions::map(key);
        assert!(matches!(action, Some(crate::ui::Action::GotoTab(_))));
    }

    #[test]
    fn text_input_accepts_digits_without_escaping_to_global_nav() {
        let mut s = loaded_slot(&seeded_string_base());
        assert!(focus_field(&mut s, "Custom Setting Name"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        for c in "123".chars() {
            s.handle_edit_mode_key(key_press(KeyCode::Char(c)));
        }
        let Some(InlineEdit {
            kind: InlineKind::TextInput(text),
            ..
        }) = s.editing.as_ref()
        else {
            panic!("expected text input");
        };
        assert!(text.buffer.ends_with("123"));
    }

    fn loaded_canonical_film(s: &SimulationTabState) -> Option<FilmSimulation> {
        loaded_canonical(s).film_simulation
    }

    fn loaded_shadow(s: &SimulationTabState) -> &SimulationBase {
        match s.slots.get(CustomSetting::C1).unwrap() {
            SlotEntry::Loaded(buf) => &buf.working.shadow,
            _ => panic!("expected Loaded slot"),
        }
    }

    #[test]
    fn slot_load_seeds_shadow_with_mono_defaults_when_film_is_provia() {
        let s = loaded_slot(&SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        let shadow = loaded_shadow(&s);
        assert!(shadow.monochromatic_color_temperature.is_some());
        let canonical = loaded_canonical(&s);
        assert_eq!(canonical.monochromatic_color_temperature, None);
    }

    #[test]
    fn switching_to_monochrome_reveals_mono_fields_via_settle() {
        let mut s = loaded_slot(&SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        assert_eq!(loaded_canonical(&s).monochromatic_color_temperature, None);

        assert!(focus_field(&mut s, "Film Simulation"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        s.commit_pick("acros");

        assert_eq!(loaded_canonical_film(&s), Some(FilmSimulation::Acros));
        assert!(
            loaded_canonical(&s)
                .monochromatic_color_temperature
                .is_some()
        );
    }

    #[test]
    fn bump_mirrors_canonical_into_shadow() {
        let mut s = loaded_slot(&SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            clarity: Some(Clarity::default()),
            ..Default::default()
        });
        assert!(focus_field(&mut s, "Clarity"));
        let shadow_mono_before = loaded_shadow(&s).monochromatic_color_temperature;
        assert!(shadow_mono_before.is_some());

        s.handle_editor_action(EditorAction::Bump(Direction::Next));

        let canonical = loaded_canonical(&s).clarity;
        let shadow = loaded_shadow(&s).clarity;
        assert_eq!(canonical, shadow);
        assert_eq!(
            loaded_shadow(&s).monochromatic_color_temperature,
            shadow_mono_before,
        );
    }

    #[test]
    fn unrelated_edit_does_not_aggressively_fill_rule_allowed_fields() {
        let mut s = loaded_slot(&SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            clarity: Some(Clarity::default()),
            ..Default::default()
        });
        let canonical_before = loaded_canonical(&s).clone();
        assert!(focus_field(&mut s, "Clarity"));
        s.handle_editor_action(EditorAction::Bump(Direction::Next));
        let canonical_after = loaded_canonical(&s);
        for desc in C_X_S20_SIMULATION.fields {
            if desc.name == "Clarity" {
                continue;
            }
            assert_eq!(
                (desc.display)(canonical_after),
                (desc.display)(&canonical_before),
            );
        }
    }

    #[test]
    fn round_trip_film_change_preserves_user_typed_mono_value() {
        let mut s = loaded_slot(&SimulationBase {
            film_simulation: Some(FilmSimulation::Acros),
            monochromatic_color_temperature: Some(MonochromaticColorTemperature::default()),
            ..Default::default()
        });
        assert!(focus_field(&mut s, "Monochromatic Color Temperature"));
        for _ in 0..3 {
            s.handle_editor_action(EditorAction::Bump(Direction::Next));
        }
        let user_value = loaded_canonical(&s).monochromatic_color_temperature;
        assert_ne!(user_value, None);
        assert!(focus_field(&mut s, "Film Simulation"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        s.commit_pick("provia");
        assert_eq!(loaded_canonical(&s).monochromatic_color_temperature, None);
        assert_eq!(
            loaded_shadow(&s).monochromatic_color_temperature,
            user_value
        );
        s.handle_editor_action(EditorAction::EnterEditMode);
        s.commit_pick("acros");
        assert_eq!(
            loaded_canonical(&s).monochromatic_color_temperature,
            user_value,
        );
    }

    #[test]
    fn commit_aborts_when_focus_drifted_under_inline_edit() {
        let mut s = loaded_slot(&seeded_string_base());
        assert!(focus_field(&mut s, "Custom Setting Name"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        s.handle_edit_mode_key(key_press(KeyCode::Char('Z')));
        s.cursor.simulation = SimulationCursor::None;
        let canonical_before = loaded_canonical(&s).custom_setting_name.clone();
        s.commit_text("VALUE_NOT_LANDED");
        assert!(s.editing.is_none());
        assert_eq!(loaded_canonical(&s).custom_setting_name, canonical_before);
    }

    #[test]
    fn settle_no_op_when_canonical_already_matches_shadow() {
        let mut state = SimulationState {
            canonical: SimulationBase {
                film_simulation: Some(FilmSimulation::Provia),
                ..Default::default()
            },
            shadow: SimulationBase {
                film_simulation: Some(FilmSimulation::Provia),
                ..Default::default()
            },
        };
        let before = state.canonical.clone();
        SimulationTabState::settle(&mut state, &C_X_S20_SIMULATION, None);
        assert_eq!(state.canonical, before);
    }

    #[test]
    fn edit_mode_arrow_consumed_by_text_input_not_dispatch() {
        let mut s = loaded_slot(&seeded_string_base());
        assert!(focus_field(&mut s, "Custom Setting Name"));
        s.handle_editor_action(EditorAction::EnterEditMode);
        s.handle_edit_mode_key(key_press(KeyCode::Char('X')));
        let cursor_before = match s.editing.as_ref() {
            Some(InlineEdit {
                kind: InlineKind::TextInput(text),
                ..
            }) => text.cursor_col,
            _ => panic!("expected text input"),
        };
        s.handle_edit_mode_key(key_press(KeyCode::Left));
        let cursor_after = match s.editing.as_ref() {
            Some(InlineEdit {
                kind: InlineKind::TextInput(text),
                ..
            }) => text.cursor_col,
            _ => panic!("expected text input"),
        };
        assert_eq!(cursor_after + 1, cursor_before);
        assert!(matches!(
            s.editing,
            Some(InlineEdit {
                kind: InlineKind::TextInput(_),
                ..
            })
        ));
    }

    fn typed(c: char) -> KeyEvent {
        key_press(KeyCode::Char(c))
    }

    fn type_into_filter(s: &mut SimulationTabState, text: &str) {
        for c in text.chars() {
            s.handle_filter_key(typed(c));
        }
    }

    fn named_sim(name: &str) -> SimulationBase {
        SimulationBase {
            custom_setting_name: Some(name.parse().expect("valid name")),
            ..Default::default()
        }
    }

    fn multi_slot_setup(slots: &[(CustomSetting, Option<&str>)]) -> SimulationTabState {
        let mut s = SimulationTabState::default();
        s.slots.descriptors = Some(&C_X_S20_SIMULATION);
        let ids: Vec<_> = slots.iter().map(|(c, _)| *c).collect();
        let r = req();
        s.slots.state = SlotsState::Requested(r);
        s.handle_slots_enumerated(r, &ids).unwrap();
        for (slot, name) in slots {
            let base = name.map_or_else(SimulationBase::default, named_sim);
            s.handle_slot_fetched(*slot, &base).unwrap();
        }
        s
    }

    fn slash() -> KeyEvent {
        key_press(KeyCode::Char('/'))
    }

    #[test]
    fn filter_default_state_is_inactive_and_empty() {
        let s = SimulationTabState::default();
        assert!(!s.filtering());
        assert!(s.filter().buffer.is_empty());
        assert_eq!(s.filter().cursor_col, 0);
    }

    #[test]
    fn slash_in_list_pane_opens_filter() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        assert!(s.filtering());
    }

    #[test]
    fn slash_in_editor_pane_does_not_open_filter() {
        let mut s = loaded_slot(&seeded_string_base());
        assert!(focus_field(&mut s, "Custom Setting Name"));
        assert_eq!(s.pane(), Pane::Editor);
        s.handle_key(slash());
        assert!(!s.filtering());
    }

    #[test]
    fn slash_seeks_cursor_to_end_of_retained_buffer() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        s.handle_filter_key(key_press(KeyCode::Enter));
        assert!(!s.filtering());
        assert_eq!(s.filter().buffer, "vel");
        s.handle_key(slash());
        assert!(s.filtering());
        assert_eq!(s.filter().cursor_col, 3);
    }

    #[test]
    fn typing_into_filter_appends_chars() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        assert_eq!(s.filter().buffer, "vel");
        assert_eq!(s.filter().cursor_col, 3);
    }

    #[test]
    fn esc_closes_and_clears_filter() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        s.handle_filter_key(key_press(KeyCode::Esc));
        assert!(!s.filtering());
        assert!(s.filter().buffer.is_empty());
        assert_eq!(s.filter().cursor_col, 0);
    }

    #[test]
    fn enter_closes_and_keeps_filter() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        s.handle_filter_key(key_press(KeyCode::Enter));
        assert!(!s.filtering());
        assert_eq!(s.filter().buffer, "vel");
    }

    #[test]
    fn backspace_on_empty_closes_filter() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        s.handle_filter_key(key_press(KeyCode::Backspace));
        assert!(!s.filtering());
    }

    #[test]
    fn backspace_with_chars_removes_last_char_and_stays_open() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        s.handle_filter_key(key_press(KeyCode::Backspace));
        assert!(s.filtering());
        assert_eq!(s.filter().buffer, "ve");
        assert_eq!(s.filter().cursor_col, 2);
    }

    #[test]
    fn cursor_keys_move_within_filter_without_closing() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        s.handle_filter_key(key_press(KeyCode::Home));
        assert_eq!(s.filter().cursor_col, 0);
        s.handle_filter_key(key_press(KeyCode::Right));
        assert_eq!(s.filter().cursor_col, 1);
        s.handle_filter_key(key_press(KeyCode::End));
        assert_eq!(s.filter().cursor_col, 3);
        assert!(s.filtering());
    }

    #[test]
    fn is_capturing_input_true_while_filtering() {
        let mut s = SimulationTabState::default();
        assert!(!<SimulationTabState as TabBehavior>::is_capturing_input(&s));
        s.handle_key(slash());
        assert!(<SimulationTabState as TabBehavior>::is_capturing_input(&s));
        s.handle_filter_key(key_press(KeyCode::Esc));
        assert!(!<SimulationTabState as TabBehavior>::is_capturing_input(&s));
    }

    #[test]
    fn slot_entries_filter_by_name_substring() {
        let mut s = multi_slot_setup(&[
            (CustomSetting::C1, Some("velvia warm")),
            (CustomSetting::C2, Some("provia")),
            (CustomSetting::C3, Some("velvia cool")),
        ]);
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        let visible: Vec<_> = s.slot_entries().map(|(slot, _)| slot).collect();
        assert_eq!(visible, vec![CustomSetting::C1, CustomSetting::C3]);
    }

    #[test]
    fn slot_entries_hides_unnamed_when_filter_active() {
        let mut s = multi_slot_setup(&[
            (CustomSetting::C1, Some("velvia warm")),
            (CustomSetting::C2, None),
        ]);
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        let visible: Vec<_> = s.slot_entries().map(|(slot, _)| slot).collect();
        assert_eq!(visible, vec![CustomSetting::C1]);
    }

    #[test]
    fn slot_entries_shows_unnamed_when_filter_empty() {
        let s = multi_slot_setup(&[
            (CustomSetting::C1, Some("velvia warm")),
            (CustomSetting::C2, None),
        ]);
        let visible: Vec<_> = s.slot_entries().map(|(slot, _)| slot).collect();
        assert_eq!(visible, vec![CustomSetting::C1, CustomSetting::C2]);
    }

    #[test]
    fn slot_entries_hides_loading_when_filter_active() {
        let mut s = SimulationTabState::default();
        s.slots.descriptors = Some(&C_X_S20_SIMULATION);
        enumerate(&mut s, &[CustomSetting::C1, CustomSetting::C2]);
        s.handle_slot_fetched(CustomSetting::C1, &named_sim("velvia"))
            .unwrap();
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        let visible: Vec<_> = s.slot_entries().map(|(slot, _)| slot).collect();
        assert_eq!(visible, vec![CustomSetting::C1]);
    }

    #[test]
    fn library_entries_filter_by_name_substring() {
        let mut s = SimulationTabState::default();
        let velvia_warm = Slug::try_from("velvia-warm").unwrap();
        let provia = Slug::try_from("provia").unwrap();
        let velvia_cool = Slug::try_from("velvia-cool").unwrap();
        s.sync_library(&snapshot_with(vec![
            (
                velvia_warm.clone(),
                sample_entry("Velvia Warm", SimulationBase::default()),
            ),
            (provia, sample_entry("Provia", SimulationBase::default())),
            (
                velvia_cool.clone(),
                sample_entry("Velvia Cool", SimulationBase::default()),
            ),
        ]));
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        let visible: Vec<_> = s.library_entries().map(|(slug, _)| slug.clone()).collect();
        assert_eq!(visible, vec![velvia_cool, velvia_warm]);
    }

    #[test]
    fn filter_matching_is_case_insensitive() {
        let mut s = multi_slot_setup(&[(CustomSetting::C1, Some("VELVIA WARM"))]);
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        let visible: Vec<_> = s.slot_entries().map(|(slot, _)| slot).collect();
        assert_eq!(visible, vec![CustomSetting::C1]);
    }

    #[test]
    fn cursor_rehomes_when_filter_narrows_past_focused_slot() {
        let mut s = multi_slot_setup(&[
            (CustomSetting::C1, Some("velvia")),
            (CustomSetting::C2, Some("provia")),
        ]);
        step(&mut s, CursorMove::Down);
        assert_eq!(
            s.cursor.simulation,
            SimulationCursor::Slot(CustomSetting::C2)
        );
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        assert_eq!(
            s.cursor.simulation,
            SimulationCursor::Slot(CustomSetting::C1)
        );
    }

    #[test]
    fn cursor_stays_when_focused_entry_still_matches() {
        let mut s = multi_slot_setup(&[
            (CustomSetting::C1, Some("velvia warm")),
            (CustomSetting::C2, Some("velvia cool")),
            (CustomSetting::C3, Some("provia")),
        ]);
        step(&mut s, CursorMove::Down);
        assert_eq!(
            s.cursor.simulation,
            SimulationCursor::Slot(CustomSetting::C2)
        );
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        assert_eq!(
            s.cursor.simulation,
            SimulationCursor::Slot(CustomSetting::C2)
        );
    }

    #[test]
    fn library_sync_under_filter_preserves_state() {
        let mut s = SimulationTabState::default();
        let velvia_warm = Slug::try_from("velvia-warm").unwrap();
        let velvia_cool = Slug::try_from("velvia-cool").unwrap();
        s.sync_library(&snapshot_with(vec![
            (
                velvia_warm.clone(),
                sample_entry("Velvia Warm", SimulationBase::default()),
            ),
            (
                velvia_cool.clone(),
                sample_entry("Velvia Cool", SimulationBase::default()),
            ),
        ]));
        step(&mut s, CursorMove::Down);
        assert_eq!(
            s.cursor.simulation,
            SimulationCursor::Library(velvia_cool.clone())
        );
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");

        let astia = Slug::try_from("astia").unwrap();
        s.sync_library(&snapshot_with(vec![
            (astia, sample_entry("Astia", SimulationBase::default())),
            (
                velvia_warm.clone(),
                sample_entry("Velvia Warm", SimulationBase::default()),
            ),
            (
                velvia_cool.clone(),
                sample_entry("Velvia Cool", SimulationBase::default()),
            ),
        ]));

        assert!(s.filtering());
        assert_eq!(s.filter().buffer, "vel");
        assert_eq!(
            s.cursor.simulation,
            SimulationCursor::Library(velvia_cool.clone())
        );
        let visible: Vec<_> = s.library_entries().map(|(slug, _)| slug.clone()).collect();
        assert_eq!(visible, vec![velvia_cool, velvia_warm]);
    }

    #[test]
    fn library_sync_rehomes_cursor_when_focused_entry_removed_under_filter() {
        let mut s = SimulationTabState::default();
        let velvia_warm = Slug::try_from("velvia-warm").unwrap();
        let velvia_cool = Slug::try_from("velvia-cool").unwrap();
        s.sync_library(&snapshot_with(vec![
            (
                velvia_warm.clone(),
                sample_entry("Velvia Warm", SimulationBase::default()),
            ),
            (
                velvia_cool.clone(),
                sample_entry("Velvia Cool", SimulationBase::default()),
            ),
        ]));
        step(&mut s, CursorMove::Down);
        assert_eq!(
            s.cursor.simulation,
            SimulationCursor::Library(velvia_cool.clone())
        );

        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        assert_eq!(s.cursor.simulation, SimulationCursor::Library(velvia_cool));

        s.sync_library(&snapshot_with(vec![(
            velvia_warm.clone(),
            sample_entry("Velvia Warm", SimulationBase::default()),
        )]));

        assert_eq!(s.cursor.simulation, SimulationCursor::Library(velvia_warm));
    }

    #[test]
    fn library_sync_adds_entry_matching_active_filter() {
        let mut s = SimulationTabState::default();
        let velvia = Slug::try_from("velvia-warm").unwrap();
        s.sync_library(&snapshot_with(vec![(
            velvia.clone(),
            sample_entry("Velvia Warm", SimulationBase::default()),
        )]));
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");

        let velvia2 = Slug::try_from("velvia-cool").unwrap();
        s.sync_library(&snapshot_with(vec![
            (
                velvia.clone(),
                sample_entry("Velvia Warm", SimulationBase::default()),
            ),
            (
                velvia2.clone(),
                sample_entry("Velvia Cool", SimulationBase::default()),
            ),
        ]));

        let visible: Vec<_> = s.library_entries().map(|(slug, _)| slug.clone()).collect();
        assert!(visible.contains(&velvia));
        assert!(visible.contains(&velvia2));
    }

    #[test]
    fn slot_rename_can_hide_slot_from_active_filter() {
        let mut s = multi_slot_setup(&[
            (CustomSetting::C1, Some("velvia warm")),
            (CustomSetting::C2, Some("provia")),
        ]);
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        let visible: Vec<_> = s.slot_entries().map(|(slot, _)| slot).collect();
        assert_eq!(visible, vec![CustomSetting::C1]);

        if let Some(SlotEntry::Loaded(buf)) = s.slots.get_mut(CustomSetting::C1) {
            buf.working.canonical.custom_setting_name = Some("astia".parse().unwrap());
        }

        assert!(s.slot_entries().next().is_none());
    }

    #[test]
    fn slot_entry_name_returns_some_when_loaded_with_name() {
        let s = loaded_slot(&named_sim("Velvia Warm"));
        let entry = s.slots.get(CustomSetting::C1).unwrap();
        assert_eq!(
            entry.name().map(ToString::to_string),
            Some("Velvia Warm".to_owned()),
        );
    }

    #[test]
    fn slot_entry_name_returns_none_when_loaded_without_name() {
        let s = loaded_slot(&SimulationBase::default());
        let entry = s.slots.get(CustomSetting::C1).unwrap();
        assert!(entry.name().is_none());
    }

    #[test]
    fn slot_entry_name_returns_none_when_loading() {
        let entry = SlotEntry::Loading;
        assert!(entry.name().is_none());
    }

    #[test]
    fn slot_entry_name_returns_none_when_failed() {
        let err = Arc::new(CoreError::NoImagingInterface);
        let entry = SlotEntry::Failed(err);
        assert!(entry.name().is_none());
    }
}
