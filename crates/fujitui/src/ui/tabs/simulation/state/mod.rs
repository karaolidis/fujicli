use std::{collections::BTreeMap, sync::Arc};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fujicore::{
    CoreError,
    features::simulation::SimulationDescriptors,
    generated::{options::CustomSetting, simulations::SimulationBase},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};

use crate::{
    ui::{
        tabs::{AppCtx, Shadowed},
        widgets::TextInputState,
    },
    workers::{
        ReqId,
        fs::{simulation::SimulationLibrarySnapshot, slug::Slug},
    },
};

pub(super) type SimulationState = Shadowed<SimulationBase>;

mod editor;
mod library;
mod list;
mod slots;

use editor::{EditorOutcome, EditorState};
use library::{SimulationLibrarySyncReport, SimulationLibraryView};
use list::ListPane;
use slots::{FetchSkipError, SlotEntry, SlotError, Slots};

const INDENT: &str = "  ";
const DIRTY_MARKER: &str = "*";

#[derive(Debug, Clone, Copy)]
pub(super) enum CursorMove {
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(super) enum SimulationCursor {
    #[default]
    None,
    Slot(CustomSetting),
    Library(Slug),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Pane {
    #[default]
    List,
    Editor,
}

#[derive(Debug)]
pub(super) enum EditorTarget<'a> {
    None,
    Loading {
        title: String,
    },
    Failed {
        title: String,
        error: &'a Arc<CoreError>,
    },
    Ready {
        title: String,
        working: &'a SimulationState,
        fetched: &'a SimulationBase,
        descriptors: &'static SimulationDescriptors,
        dirty: bool,
    },
}

impl<'a> EditorTarget<'a> {
    fn resolve(
        selection: &SimulationCursor,
        slots: &'a Slots,
        library: &'a SimulationLibraryView,
    ) -> Self {
        match selection {
            SimulationCursor::None => EditorTarget::None,
            SimulationCursor::Slot(slot) => {
                let Some(descriptors) = slots.descriptors else {
                    return EditorTarget::None;
                };
                match slots.get(*slot) {
                    Some(SlotEntry::Loading) => EditorTarget::Loading {
                        title: slot.to_string(),
                    },
                    Some(SlotEntry::Failed(error)) => EditorTarget::Failed {
                        title: slot.to_string(),
                        error,
                    },
                    Some(SlotEntry::Loaded(buf)) => EditorTarget::Ready {
                        title: slot.to_string(),
                        working: &buf.working,
                        fetched: &buf.fetched.canonical,
                        descriptors,
                        dirty: buf.dirty(),
                    },
                    None => EditorTarget::None,
                }
            }
            SimulationCursor::Library(slug) => {
                library
                    .get(slug)
                    .map_or(EditorTarget::None, |lib| EditorTarget::Ready {
                        title: lib.entry.name.clone(),
                        working: &lib.buffer.working,
                        fetched: &lib.buffer.fetched.canonical,
                        descriptors: lib.descriptors,
                        dirty: lib.buffer.dirty(),
                    })
            }
        }
    }

    fn visible_field_count(&self) -> usize {
        match self {
            Self::Ready {
                working,
                descriptors,
                ..
            } => descriptors.visible_fields(&working.canonical).len(),
            _ => 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct SimulationTabState {
    slots: Slots,
    library: SimulationLibraryView,
    list: ListPane,
    editors: BTreeMap<SimulationCursor, EditorState>,
    focus: Pane,
}

impl SimulationTabState {
    pub(super) fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let [list_area, editor_area] =
            Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
                .areas(area);
        let Self {
            slots,
            library,
            list,
            editors,
            focus,
        } = self;
        list.draw(frame, list_area, *focus == Pane::List, slots, library);
        let selection = list.selection().clone();
        let target = EditorTarget::resolve(&selection, slots, library);
        editors.entry(selection).or_default().draw(
            frame,
            editor_area,
            *focus == Pane::Editor,
            target,
        );
    }

    pub(super) fn capturing_input(&self) -> bool {
        self.list.filtering() || self.is_editing()
    }

    fn is_editing(&self) -> bool {
        self.editors
            .get(self.list.selection())
            .is_some_and(EditorState::is_editing)
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.is_editing() {
            self.handle_editor_key(key);
            return;
        }
        if self.list.filtering() {
            if self.list.handle_filter_key(key) {
                self.settle();
            }
            return;
        }
        match self.focus {
            Pane::List => self.handle_list_key(key),
            Pane::Editor => self.handle_editor_key(key),
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let order = self.cursor_order();
                self.list.step(CursorMove::Up, &order);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let order = self.cursor_order();
                self.list.step(CursorMove::Down, &order);
            }
            KeyCode::Char('/') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.list.start_filter();
            }
            KeyCode::Enter => {
                let target =
                    EditorTarget::resolve(self.list.selection(), &self.slots, &self.library);
                if target.visible_field_count() > 0 {
                    self.focus = Pane::Editor;
                }
            }
            _ => {}
        }
    }

    fn handle_editor_key(&mut self, key: KeyEvent) {
        let Self {
            slots,
            library,
            list,
            editors,
            focus,
        } = self;
        let selection = list.selection().clone();
        let outcome = match &selection {
            SimulationCursor::None => EditorOutcome::ExitToList,
            SimulationCursor::Slot(slot) => match (slots.descriptors, slots.get_mut(*slot)) {
                (Some(descriptors), Some(SlotEntry::Loaded(buffer))) => editors
                    .entry(selection.clone())
                    .or_default()
                    .handle_key(key, buffer, descriptors),
                _ => {
                    if matches!(key.code, KeyCode::Esc) {
                        EditorOutcome::ExitToList
                    } else {
                        EditorOutcome::Continue
                    }
                }
            },
            SimulationCursor::Library(slug) => match library.get_mut(slug) {
                Some(lib) => editors.entry(selection.clone()).or_default().handle_key(
                    key,
                    &mut lib.buffer,
                    lib.descriptors,
                ),
                None => {
                    if matches!(key.code, KeyCode::Esc) {
                        EditorOutcome::ExitToList
                    } else {
                        EditorOutcome::Continue
                    }
                }
            },
        };
        if matches!(outcome, EditorOutcome::ExitToList) {
            *focus = Pane::List;
        }
    }

    pub(super) fn request_fetch(&mut self, ctx: &AppCtx) -> Result<ReqId, FetchSkipError> {
        let camera = ctx
            .device_snapshot
            .as_ref()
            .and_then(|s| s.usb_id.supported_camera());
        self.slots
            .request_fetch(ctx.device.as_ref(), camera, &ctx.req)
    }

    pub(super) const fn mark_stale(&mut self) {
        self.slots.mark_stale();
    }

    pub(super) fn handle_slots_enumerated(
        &mut self,
        req: ReqId,
        slots: &[CustomSetting],
    ) -> Result<(), SlotError> {
        self.slots.handle_enumerated(req, slots)?;
        let order = self.cursor_order();
        self.list.settle_selection(&order);
        self.prune_editors(&order);
        Ok(())
    }

    pub(super) fn handle_slots_enumeration_failed(&mut self, req: ReqId) -> Result<(), SlotError> {
        self.slots.handle_enumeration_failed(req)
    }

    pub(super) fn handle_slot_fetched(
        &mut self,
        slot: CustomSetting,
        base: &SimulationBase,
    ) -> Result<(), SlotError> {
        self.slots.handle_fetched(slot, base)
    }

    pub(super) fn handle_slot_fetch_failed(
        &mut self,
        slot: CustomSetting,
        error: Arc<CoreError>,
    ) -> Result<(), SlotError> {
        self.slots.handle_fetch_failed(slot, error)
    }

    pub(super) fn sync_library(
        &mut self,
        snapshot: &SimulationLibrarySnapshot,
    ) -> SimulationLibrarySyncReport {
        let report = self.library.sync(snapshot);
        let order = self.settle();
        self.prune_editors(&order);
        report
    }

    fn settle(&mut self) -> Vec<SimulationCursor> {
        let order = self.cursor_order();
        self.list.ensure_valid(&order);
        order
    }

    fn cursor_order(&self) -> Vec<SimulationCursor> {
        self.list.order(&self.slots, &self.library)
    }

    fn prune_editors(&mut self, order: &[SimulationCursor]) {
        self.editors
            .retain(|selection, _| order.contains(selection));
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyEventKind, KeyEventState};
    use fujicore::{
        UsbId,
        generated::{cameras::C_X_S20_SIMULATION, options::CustomSettingName},
    };
    use time::OffsetDateTime;

    use super::{slots::SlotsState, *};
    use crate::{
        ui::tabs::TabBehavior,
        workers::{ReqIdGen, fs::simulation::SimulationLibraryEntry},
    };

    fn key_press(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    fn slash() -> KeyEvent {
        key_press(KeyCode::Char('/'))
    }

    fn typed(c: char) -> KeyEvent {
        key_press(KeyCode::Char(c))
    }

    fn type_into_filter(s: &mut SimulationTabState, text: &str) {
        for c in text.chars() {
            s.handle_key(typed(c));
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
        s.list.step(dir, &order);
    }

    fn slot_ids(s: &SimulationTabState) -> Vec<CustomSetting> {
        s.list
            .slot_entries(&s.slots)
            .map(|(slot, _)| slot)
            .collect()
    }

    fn library_slugs(s: &SimulationTabState) -> Vec<Slug> {
        s.list
            .library_entries(&s.library)
            .map(|(slug, _)| slug.clone())
            .collect()
    }

    fn snapshot_with(entries: Vec<(Slug, SimulationLibraryEntry)>) -> SimulationLibrarySnapshot {
        let mut s = SimulationLibrarySnapshot::default();
        for (k, v) in entries {
            s.entries.insert(k, v);
        }
        s
    }

    fn sample_entry(name: &str, sim: SimulationBase) -> SimulationLibraryEntry {
        let now = OffsetDateTime::now_utc();
        SimulationLibraryEntry {
            name: name.to_owned(),
            source_camera: UsbId {
                vendor: 0x04CB,
                product: 0x02FC,
            },
            created: now,
            modified: now,
            simulation: sim,
        }
    }

    fn named_sim(name: &str) -> SimulationBase {
        SimulationBase {
            custom_setting_name: Some(name.parse().expect("valid name")),
            ..Default::default()
        }
    }

    fn seeded_string_base() -> SimulationBase {
        SimulationBase {
            custom_setting_name: Some(CustomSettingName::default()),
            ..Default::default()
        }
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

    fn focus_field(s: &mut SimulationTabState, name: &str) -> bool {
        let selection = s.list.selection().clone();
        let idx = {
            let EditorTarget::Ready {
                working,
                descriptors,
                ..
            } = EditorTarget::resolve(&selection, &s.slots, &s.library)
            else {
                return false;
            };
            descriptors
                .visible_fields(&working.canonical)
                .iter()
                .position(|f| f.name == name)
        };
        let Some(idx) = idx else {
            return false;
        };
        s.focus = Pane::Editor;
        s.editors.entry(selection).or_default().set_field(idx);
        true
    }

    #[test]
    fn move_cursor_clamps_at_extremes() {
        let mut s = SimulationTabState::default();
        enumerate(&mut s, &[CustomSetting::C1, CustomSetting::C2]);
        assert_eq!(
            s.list.selection(),
            &SimulationCursor::Slot(CustomSetting::C1)
        );
        step(&mut s, CursorMove::Up);
        assert_eq!(
            s.list.selection(),
            &SimulationCursor::Slot(CustomSetting::C1)
        );
        step(&mut s, CursorMove::Down);
        assert_eq!(
            s.list.selection(),
            &SimulationCursor::Slot(CustomSetting::C2)
        );
        step(&mut s, CursorMove::Down);
        assert_eq!(
            s.list.selection(),
            &SimulationCursor::Slot(CustomSetting::C2)
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
        assert_eq!(s.list.selection(), &SimulationCursor::Library(slug));
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
        assert_eq!(s.list.selection(), &SimulationCursor::Library(slug));
        let report = s.sync_library(&SimulationLibrarySnapshot::default());
        assert_eq!(report.removed.len(), 1);
        assert_eq!(s.list.selection(), &SimulationCursor::None);
    }

    #[test]
    fn focused_none_when_cursor_none() {
        let s = SimulationTabState::default();
        assert!(matches!(
            EditorTarget::resolve(s.list.selection(), &s.slots, &s.library),
            EditorTarget::None
        ));
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
        assert_eq!(s.list.selection(), &SimulationCursor::Library(b));
        s.sync_library(&snapshot_with(vec![
            (a, sample_entry("A", SimulationBase::default())),
            (c.clone(), sample_entry("C", SimulationBase::default())),
        ]));
        assert_eq!(s.list.selection(), &SimulationCursor::Library(c));
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
        assert_eq!(s.list.selection(), &SimulationCursor::Library(b));
        s.sync_library(&snapshot_with(vec![(
            a.clone(),
            sample_entry("A", SimulationBase::default()),
        )]));
        assert_eq!(s.list.selection(), &SimulationCursor::Library(a));
    }

    #[test]
    fn is_capturing_input_reflects_edit_state() {
        let mut s = loaded_slot(&seeded_string_base());
        assert!(!s.is_capturing_input());
        assert!(focus_field(&mut s, "Custom Setting Name"));
        s.handle_key(key_press(KeyCode::Enter));
        assert!(s.is_capturing_input());
        s.handle_key(key_press(KeyCode::Esc));
        assert!(!s.is_capturing_input());
    }

    #[test]
    fn global_action_mapper_would_swallow_digit_without_bypass() {
        let key = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
        let action = crate::ui::actions::map(key);
        assert!(matches!(action, Some(crate::ui::Action::GotoTab(_))));
    }

    #[test]
    fn filter_default_state_is_inactive_and_empty() {
        let s = SimulationTabState::default();
        assert!(!s.list.filtering());
        assert!(s.list.filter().buffer.is_empty());
        assert_eq!(s.list.filter().cursor_col, 0);
    }

    #[test]
    fn slash_in_list_pane_opens_filter() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        assert!(s.list.filtering());
    }

    #[test]
    fn slash_in_editor_pane_does_not_open_filter() {
        let mut s = loaded_slot(&seeded_string_base());
        assert!(focus_field(&mut s, "Custom Setting Name"));
        assert_eq!(s.focus, Pane::Editor);
        s.handle_key(slash());
        assert!(!s.list.filtering());
    }

    #[test]
    fn slash_seeks_cursor_to_end_of_retained_buffer() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        s.handle_key(key_press(KeyCode::Enter));
        assert!(!s.list.filtering());
        assert_eq!(s.list.filter().buffer, "vel");
        s.handle_key(slash());
        assert!(s.list.filtering());
        assert_eq!(s.list.filter().cursor_col, 3);
    }

    #[test]
    fn typing_into_filter_appends_chars() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        assert_eq!(s.list.filter().buffer, "vel");
        assert_eq!(s.list.filter().cursor_col, 3);
    }

    #[test]
    fn esc_closes_and_clears_filter() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        s.handle_key(key_press(KeyCode::Esc));
        assert!(!s.list.filtering());
        assert!(s.list.filter().buffer.is_empty());
        assert_eq!(s.list.filter().cursor_col, 0);
    }

    #[test]
    fn enter_closes_and_keeps_filter() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        s.handle_key(key_press(KeyCode::Enter));
        assert!(!s.list.filtering());
        assert_eq!(s.list.filter().buffer, "vel");
    }

    #[test]
    fn backspace_on_empty_closes_filter() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        s.handle_key(key_press(KeyCode::Backspace));
        assert!(!s.list.filtering());
    }

    #[test]
    fn backspace_with_chars_removes_last_char_and_stays_open() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        s.handle_key(key_press(KeyCode::Backspace));
        assert!(s.list.filtering());
        assert_eq!(s.list.filter().buffer, "ve");
        assert_eq!(s.list.filter().cursor_col, 2);
    }

    #[test]
    fn cursor_keys_move_within_filter_without_closing() {
        let mut s = SimulationTabState::default();
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        s.handle_key(key_press(KeyCode::Home));
        assert_eq!(s.list.filter().cursor_col, 0);
        s.handle_key(key_press(KeyCode::Right));
        assert_eq!(s.list.filter().cursor_col, 1);
        s.handle_key(key_press(KeyCode::End));
        assert_eq!(s.list.filter().cursor_col, 3);
        assert!(s.list.filtering());
    }

    #[test]
    fn is_capturing_input_true_while_filtering() {
        let mut s = SimulationTabState::default();
        assert!(!s.is_capturing_input());
        s.handle_key(slash());
        assert!(s.is_capturing_input());
        s.handle_key(key_press(KeyCode::Esc));
        assert!(!s.is_capturing_input());
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
        assert_eq!(slot_ids(&s), vec![CustomSetting::C1, CustomSetting::C3]);
    }

    #[test]
    fn slot_entries_hides_unnamed_when_filter_active() {
        let mut s = multi_slot_setup(&[
            (CustomSetting::C1, Some("velvia warm")),
            (CustomSetting::C2, None),
        ]);
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        assert_eq!(slot_ids(&s), vec![CustomSetting::C1]);
    }

    #[test]
    fn slot_entries_shows_unnamed_when_filter_empty() {
        let s = multi_slot_setup(&[
            (CustomSetting::C1, Some("velvia warm")),
            (CustomSetting::C2, None),
        ]);
        assert_eq!(slot_ids(&s), vec![CustomSetting::C1, CustomSetting::C2]);
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
        assert_eq!(slot_ids(&s), vec![CustomSetting::C1]);
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
        assert_eq!(library_slugs(&s), vec![velvia_cool, velvia_warm]);
    }

    #[test]
    fn filter_matching_is_case_insensitive() {
        let mut s = multi_slot_setup(&[(CustomSetting::C1, Some("VELVIA WARM"))]);
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        assert_eq!(slot_ids(&s), vec![CustomSetting::C1]);
    }

    #[test]
    fn cursor_rehomes_when_filter_narrows_past_focused_slot() {
        let mut s = multi_slot_setup(&[
            (CustomSetting::C1, Some("velvia")),
            (CustomSetting::C2, Some("provia")),
        ]);
        step(&mut s, CursorMove::Down);
        assert_eq!(
            s.list.selection(),
            &SimulationCursor::Slot(CustomSetting::C2)
        );
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        assert_eq!(
            s.list.selection(),
            &SimulationCursor::Slot(CustomSetting::C1)
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
            s.list.selection(),
            &SimulationCursor::Slot(CustomSetting::C2)
        );
        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        assert_eq!(
            s.list.selection(),
            &SimulationCursor::Slot(CustomSetting::C2)
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
            s.list.selection(),
            &SimulationCursor::Library(velvia_cool.clone())
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

        assert!(s.list.filtering());
        assert_eq!(s.list.filter().buffer, "vel");
        assert_eq!(
            s.list.selection(),
            &SimulationCursor::Library(velvia_cool.clone())
        );
        assert_eq!(library_slugs(&s), vec![velvia_cool, velvia_warm]);
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
            s.list.selection(),
            &SimulationCursor::Library(velvia_cool.clone())
        );

        s.handle_key(slash());
        type_into_filter(&mut s, "vel");
        assert_eq!(s.list.selection(), &SimulationCursor::Library(velvia_cool));

        s.sync_library(&snapshot_with(vec![(
            velvia_warm.clone(),
            sample_entry("Velvia Warm", SimulationBase::default()),
        )]));

        assert_eq!(s.list.selection(), &SimulationCursor::Library(velvia_warm));
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

        let visible = library_slugs(&s);
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
        assert_eq!(slot_ids(&s), vec![CustomSetting::C1]);

        if let Some(SlotEntry::Loaded(buf)) = s.slots.get_mut(CustomSetting::C1) {
            buf.working.canonical.custom_setting_name = Some("astia".parse().unwrap());
        }

        assert!(slot_ids(&s).is_empty());
    }
}
