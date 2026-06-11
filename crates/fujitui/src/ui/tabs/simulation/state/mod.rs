use std::{
    collections::{BTreeMap, HashMap, HashSet},
    mem::take,
    sync::Arc,
};

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
use thiserror::Error;

use crate::{
    ui::{
        Keybind, danger, muted,
        tabs::{AppCtx, Shadowed},
        widgets::{
            ConfirmOutcome, ConfirmState, EditorOutcome, EditorState, StatusMessage, TextInputState,
        },
    },
    workers::{
        ReqId,
        device::DeviceCommand,
        fs::{
            FsCommand,
            simulation::{SimulationLibraryEdit, SimulationLibrarySnapshot},
            slug::Slug,
        },
    },
};

pub(super) type SimulationState = Shadowed<SimulationBase>;

mod apply;
mod library;
mod list;
mod slots;

pub(super) use crate::ui::widgets::CursorMove;

use apply::{ApplyOutcome, ApplyState};
use library::{SimulationLibrarySyncReport, SimulationLibraryView};
use list::SimulationListPane;
use slots::{FetchSkipError, SlotEntry, SlotError, Slots};

const INDENT: &str = "  ";
const DIRTY_MARKER: &str = "*";
const LIBRARY_NAME_MAX_LEN: usize = 128;

const NAV_KEYBINDS: &[Keybind] = &[
    Keybind {
        keys: "↑ ↓ / j k",
        action: "Move selection",
    },
    Keybind {
        keys: "/",
        action: "Filter",
    },
];

const SLOT_KEYBINDS: &[Keybind] = &[
    Keybind {
        keys: "↑ ↓ / j k",
        action: "Move selection",
    },
    Keybind {
        keys: "Enter",
        action: "Edit",
    },
    Keybind {
        keys: "w",
        action: "Write to camera",
    },
    Keybind {
        keys: "s",
        action: "Save to library",
    },
    Keybind {
        keys: "u",
        action: "Revert changes",
    },
    Keybind {
        keys: "/",
        action: "Filter",
    },
];

const LIBRARY_KEYBINDS: &[Keybind] = &[
    Keybind {
        keys: "↑ ↓ / j k",
        action: "Move selection",
    },
    Keybind {
        keys: "Enter",
        action: "Edit",
    },
    Keybind {
        keys: "w",
        action: "Save to disk",
    },
    Keybind {
        keys: "a",
        action: "Apply to slot",
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
        keys: "u",
        action: "Revert changes",
    },
    Keybind {
        keys: "/",
        action: "Filter",
    },
];

const EDITOR_KEYBINDS: &[Keybind] = &[
    Keybind {
        keys: "↑ ↓ / j k",
        action: "Move field",
    },
    Keybind {
        keys: "(⇧)← →",
        action: "Adjust / jump value",
    },
    Keybind {
        keys: "Home / End",
        action: "Min / max",
    },
    Keybind {
        keys: "Enter",
        action: "Edit value",
    },
    Keybind {
        keys: "w",
        action: "Write changes",
    },
    Keybind {
        keys: "u",
        action: "Revert changes",
    },
    Keybind {
        keys: "Esc",
        action: "Back to list",
    },
];

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

#[derive(Debug)]
enum ConfirmAction {
    Delete(Slug),
    Apply { slug: Slug, slot: CustomSetting },
}

impl ConfirmAction {
    const fn targets_device(&self) -> bool {
        matches!(self, Self::Apply { .. })
    }
}

#[derive(Debug, Clone, Copy, Error)]
pub(super) enum LibraryAnomaly {
    #[error("library save response arrived for req {req} we didn't issue")]
    UnexpectedSave { req: ReqId },
}

#[derive(Debug, Default)]
struct PendingOps {
    pushes: HashSet<CustomSetting>,
    saves: HashMap<ReqId, Slug>,
    adds: HashMap<ReqId, CustomSetting>,
}

#[derive(Debug, Default)]
pub struct SimulationTabState {
    slots: Slots,
    library: SimulationLibraryView,
    list: SimulationListPane,
    editors: BTreeMap<SimulationCursor, EditorState<SimulationBase>>,
    focus: Pane,
    rename: Option<RenameState>,
    confirm: Option<PendingConfirm>,
    apply: Option<ApplyState>,
    pending: PendingOps,
    status: Vec<StatusMessage>,
}

impl SimulationTabState {
    pub(super) fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let [list_area, editor_area] =
            Layout::horizontal([Constraint::Percentage(25), Constraint::Percentage(75)])
                .areas(area);
        {
            let Self {
                slots,
                library,
                list,
                editors,
                focus,
                rename,
                ..
            } = self;
            list.draw(
                frame,
                list_area,
                *focus == Pane::List,
                slots,
                library,
                rename.as_ref(),
            );
            let selection = list.selection().clone();
            let target = EditorTarget::resolve(&selection, slots, library);
            let active = *focus == Pane::Editor;
            match target {
                EditorTarget::None => {
                    EditorState::<SimulationBase>::draw_message(
                        frame,
                        editor_area,
                        active,
                        None,
                        "(no entry selected)",
                        muted(),
                    );
                }
                EditorTarget::Loading { title } => {
                    EditorState::<SimulationBase>::draw_message(
                        frame,
                        editor_area,
                        active,
                        Some(&title),
                        "loading...",
                        muted(),
                    );
                }
                EditorTarget::Failed { title, error } => {
                    EditorState::<SimulationBase>::draw_message(
                        frame,
                        editor_area,
                        active,
                        Some(&title),
                        &format!("fetch failed: {error}"),
                        danger(),
                    );
                }
                EditorTarget::Ready {
                    title,
                    working,
                    fetched,
                    descriptors,
                    dirty,
                } => {
                    editors.entry(selection).or_default().draw(
                        frame,
                        editor_area,
                        active,
                        &title,
                        working,
                        fetched,
                        descriptors,
                        dirty,
                    );
                }
            }
        }
        if let Some(apply) = self.apply.as_mut() {
            apply.draw(frame, area);
        }
        if let Some(confirm) = self.confirm.as_ref() {
            confirm.state.draw(frame, area);
        }
    }

    pub(super) fn capturing_input(&self) -> bool {
        self.list.filtering()
            || self.is_editing()
            || self.rename.is_some()
            || self.confirm.is_some()
            || self.apply.is_some()
    }

    fn is_editing(&self) -> bool {
        self.editors
            .get(self.list.selection())
            .is_some_and(EditorState::is_editing)
    }

    pub(super) const fn keybinds(&self) -> &'static [Keybind] {
        match self.focus {
            Pane::Editor => EDITOR_KEYBINDS,
            Pane::List => match self.list.selection() {
                SimulationCursor::None => NAV_KEYBINDS,
                SimulationCursor::Slot(_) => SLOT_KEYBINDS,
                SimulationCursor::Library(_) => LIBRARY_KEYBINDS,
            },
        }
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
        if self.apply.is_some() {
            self.handle_apply_key(key);
            return;
        }
        if self.is_editing() {
            self.handle_editor_key(ctx, key);
            return;
        }
        if self.list.filtering() {
            if self.list.handle_filter_key(key) {
                self.settle();
            }
            return;
        }
        match self.focus {
            Pane::List => self.handle_list_key(ctx, key),
            Pane::Editor => self.handle_editor_key(ctx, key),
        }
    }

    fn handle_list_key(&mut self, ctx: &AppCtx, key: KeyEvent) {
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
            KeyCode::Char('w') => self.persist(ctx),
            KeyCode::Char('u') => self.revert(),
            _ => match self.list.selection().clone() {
                SimulationCursor::Slot(slot) => self.handle_slot_key(ctx, slot, key),
                SimulationCursor::Library(slug) => self.handle_library_key(ctx, &slug, key),
                SimulationCursor::None => {}
            },
        }
    }

    fn handle_slot_key(&mut self, ctx: &AppCtx, slot: CustomSetting, key: KeyEvent) {
        if key.code == KeyCode::Char('s') {
            self.save_to_library(ctx, slot);
        }
    }

    fn handle_library_key(&mut self, ctx: &AppCtx, slug: &Slug, key: KeyEvent) {
        match key.code {
            KeyCode::Char('a') => self.prompt_apply(ctx, slug),
            KeyCode::Char('r') => self.prompt_rename(slug),
            KeyCode::Char('D') | KeyCode::Delete => self.prompt_delete(slug),
            _ => {}
        }
    }

    fn handle_editor_key(&mut self, ctx: &AppCtx, key: KeyEvent) {
        if !self.is_editing() {
            match key.code {
                KeyCode::Char('w') => {
                    self.persist(ctx);
                    return;
                }
                KeyCode::Char('u') => {
                    self.revert();
                    return;
                }
                _ => {}
            }
        }
        let Self {
            slots,
            library,
            list,
            editors,
            focus,
            ..
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

    fn persist(&mut self, ctx: &AppCtx) {
        match self.list.selection().clone() {
            SimulationCursor::Slot(slot) => self.write_simulation_slot(ctx, slot),
            SimulationCursor::Library(slug) => self.write_simulation_library_entry(ctx, &slug),
            SimulationCursor::None => {}
        }
    }

    fn revert(&mut self) {
        match self.list.selection().clone() {
            SimulationCursor::Slot(slot) => {
                if let Some(SlotEntry::Loaded(buf)) = self.slots.get_mut(slot) {
                    buf.working = buf.fetched.clone();
                }
            }
            SimulationCursor::Library(slug) => {
                let lib = self
                    .library
                    .get_mut(&slug)
                    .expect("library cursor references a live entry");
                lib.buffer.working = lib.buffer.fetched.clone();
            }
            SimulationCursor::None => {}
        }
    }

    fn write_simulation_slot(&mut self, ctx: &AppCtx, slot: CustomSetting) {
        if self.pending.pushes.contains(&slot) {
            self.status.push(StatusMessage::error(format!(
                "{slot} is already being written."
            )));
            return;
        }
        let Some(device) = ctx.device.as_ref() else {
            return;
        };
        let Some(SlotEntry::Loaded(buf)) = self.slots.get(slot) else {
            return;
        };
        if !buf.dirty() {
            return;
        }
        let base = buf.working.canonical.clone();
        let req = ctx.req.next();
        device.send(DeviceCommand::PushSlot { req, slot, base });
        self.pending.pushes.insert(slot);
    }

    fn write_simulation_library_entry(&mut self, ctx: &AppCtx, slug: &Slug) {
        let lib = self
            .library
            .get(slug)
            .expect("library cursor references a live entry");
        if !lib.buffer.dirty() {
            return;
        }
        let name = lib.entry.name.clone();
        let simulation = lib.buffer.working.canonical.clone();
        if self.pending.saves.values().any(|pending| pending == slug) {
            self.status.push(StatusMessage::error(format!(
                "\"{name}\" is already saving."
            )));
            return;
        }
        let req = ctx.req.next();
        self.pending.saves.insert(req, slug.clone());
        ctx.fs.send(FsCommand::UpdateSimulation {
            req,
            slug: slug.clone(),
            edit: SimulationLibraryEdit { name, simulation },
        });
    }

    fn save_to_library(&mut self, ctx: &AppCtx, slot: CustomSetting) {
        if self.pending.adds.values().any(|s| *s == slot) {
            self.status.push(StatusMessage::error(format!(
                "{slot} is already saving to the library."
            )));
            return;
        }
        let Some(SlotEntry::Loaded(buf)) = self.slots.get(slot) else {
            return;
        };
        let Some(snapshot) = ctx.device_snapshot.as_ref() else {
            return;
        };
        let source_camera = snapshot.usb_id;
        let simulation = buf.working.canonical.clone();
        let name = Self::default_library_name(&simulation, &ctx.simulation_library_snapshot);
        let req = ctx.req.next();
        self.pending.adds.insert(req, slot);
        ctx.fs.send(FsCommand::AddSimulation {
            req,
            init: SimulationLibraryEdit { name, simulation },
            source_camera,
        });
    }

    fn prompt_apply(&mut self, ctx: &AppCtx, slug: &Slug) {
        let lib = self
            .library
            .get(slug)
            .expect("library cursor references a live entry");
        let Some(snapshot) = ctx.device_snapshot.as_ref() else {
            return;
        };
        if lib.entry.source_camera != snapshot.usb_id {
            self.status.push(StatusMessage::error(
                "This simulation is designed for a different camera.",
            ));
            return;
        }
        let slots: Vec<_> = self
            .slots
            .entries
            .iter()
            .filter(|(_, entry)| matches!(entry, SlotEntry::Loaded(_)))
            .map(|(slot, entry)| (*slot, Self::simulation_slot_label(*slot, entry)))
            .collect();
        if slots.is_empty() {
            self.status.push(StatusMessage::error(
                "No camera slots available to apply to.",
            ));
            return;
        }
        let entry_name = lib.entry.name.clone();
        self.apply = Some(ApplyState::new(slug.clone(), entry_name, slots));
    }

    fn prompt_rename(&mut self, slug: &Slug) {
        let lib = self
            .library
            .get(slug)
            .expect("library cursor references a live entry");
        if lib.buffer.dirty() {
            self.status.push(StatusMessage::error(
                "Save or revert changes before renaming this simulation.",
            ));
            return;
        }
        self.rename = Some(RenameState {
            text: TextInputState::new_with_max_len(lib.entry.name.clone(), LIBRARY_NAME_MAX_LEN),
            slug: slug.clone(),
        });
    }

    fn prompt_delete(&mut self, slug: &Slug) {
        let lib = self
            .library
            .get(slug)
            .expect("library cursor references a live entry");
        self.confirm = Some(PendingConfirm {
            state: ConfirmState {
                title: format!(" Delete {} ", lib.entry.name),
                message: format!(
                    "Permanently delete \"{}\"?\nThis cannot be undone.",
                    lib.entry.name
                ),
            },
            action: ConfirmAction::Delete(slug.clone()),
        });
    }

    fn handle_rename_key(&mut self, ctx: &AppCtx, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.rename = None,
            KeyCode::Enter => {
                let Some(rename) = self.rename.as_ref() else {
                    return;
                };
                let name = rename.text.buffer.trim().to_owned();
                if name.is_empty() {
                    return;
                }
                let slug = rename.slug.clone();
                let Some(lib) = self.library.get(&slug) else {
                    self.rename = None;
                    return;
                };
                let simulation = lib.entry.simulation.clone();
                self.rename = None;
                let req = ctx.req.next();
                if Slug::try_from(name.as_str()).is_ok_and(|new| new == slug) {
                    self.pending.saves.insert(req, slug.clone());
                }
                ctx.fs.send(FsCommand::UpdateSimulation {
                    req,
                    slug,
                    edit: SimulationLibraryEdit { name, simulation },
                });
            }
            _ => {
                if let Some(rename) = self.rename.as_mut() {
                    rename.text.handle_edit_key(key);
                }
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
                        ctx.fs.send(FsCommand::RemoveSimulation { req, slug });
                    }
                    ConfirmAction::Apply { slug, slot } => self.execute_apply(ctx, &slug, slot),
                }
            }
        }
    }

    fn handle_apply_key(&mut self, key: KeyEvent) {
        let Some(apply) = self.apply.as_mut() else {
            return;
        };
        match apply.handle_key(key) {
            ApplyOutcome::Pending => {}
            ApplyOutcome::Cancelled => self.apply = None,
            ApplyOutcome::Picked(slot) => {
                let apply = self.apply.take().expect("guarded");
                let (slug, entry_name) = apply.into_parts();
                self.confirm = Some(PendingConfirm {
                    state: ConfirmState {
                        title: format!(" Apply to {slot} "),
                        message: format!(
                            "Apply \"{entry_name}\" to {slot}?\nThis overwrites that slot."
                        ),
                    },
                    action: ConfirmAction::Apply { slug, slot },
                });
            }
        }
    }

    fn execute_apply(&mut self, ctx: &AppCtx, slug: &Slug, slot: CustomSetting) {
        if self.pending.pushes.contains(&slot) {
            self.status.push(StatusMessage::error(format!(
                "{slot} is already being written."
            )));
            return;
        }
        let Some(device) = ctx.device.as_ref() else {
            return;
        };
        let Some(lib) = self.library.get(slug) else {
            return;
        };
        let base = lib.buffer.working.canonical.clone();
        let req = ctx.req.next();
        device.send(DeviceCommand::PushSlot { req, slot, base });
        self.pending.pushes.insert(slot);
    }

    pub(super) fn handle_simulation_slot_changed(&mut self, ctx: &AppCtx, slot: CustomSetting) {
        self.pending.pushes.remove(&slot);
        self.slots
            .request_refetch(slot, ctx.device.as_ref(), &ctx.req);
    }

    pub(super) fn handle_simulation_slot_push_failed(&mut self, slot: CustomSetting) {
        self.pending.pushes.remove(&slot);
    }

    pub(super) fn handle_simulation_library_entry_saved(
        &mut self,
        req: ReqId,
        slug: &Slug,
    ) -> Result<(), LibraryAnomaly> {
        if self.pending.saves.remove(&req).is_none() {
            return Err(LibraryAnomaly::UnexpectedSave { req });
        }
        if let Some(lib) = self.library.get_mut(slug) {
            lib.buffer.fetched = lib.buffer.working.clone();
        }
        Ok(())
    }

    pub(super) fn handle_simulation_library_entry_added(&mut self, req: ReqId, slug: &Slug) {
        self.pending.adds.remove(&req);
        self.focus_entry(SimulationCursor::Library(slug.clone()));
    }

    pub(super) fn handle_simulation_library_op_failed(&mut self, req: ReqId) {
        self.pending.saves.remove(&req);
        self.pending.adds.remove(&req);
    }

    pub(super) fn handle_simulation_library_entry_renamed(
        &mut self,
        _old_slug: &Slug,
        new_slug: &Slug,
    ) {
        if self.library.get(new_slug).is_some() {
            self.list
                .set_selection(SimulationCursor::Library(new_slug.clone()));
        }
    }

    pub(super) fn focus_entry(&mut self, selection: SimulationCursor) {
        self.list.set_selection(selection);
    }

    fn simulation_slot_label(slot: CustomSetting, entry: &SlotEntry) -> String {
        match entry {
            SlotEntry::Loaded(_) => entry
                .name()
                .map_or_else(|| format!("{slot} (unnamed)"), |n| format!("{slot} {n}")),
            SlotEntry::Loading => format!("{slot} (loading...)"),
            SlotEntry::Failed(_) => format!("{slot} (failed)"),
        }
    }

    fn default_library_name(
        simulation: &SimulationBase,
        snapshot: &SimulationLibrarySnapshot,
    ) -> String {
        let base = simulation
            .custom_setting_name
            .as_ref()
            .map(ToString::to_string)
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "simulation".to_owned());
        let free =
            |name: &str| Slug::try_from(name).is_ok_and(|s| !snapshot.entries.contains_key(&s));
        if free(&base) {
            return base;
        }
        for n in 2..u32::MAX {
            let candidate = format!("{base} {n}");
            if free(&candidate) {
                return candidate;
            }
        }
        base
    }

    pub(super) fn request_fetch(&mut self, ctx: &AppCtx) -> Result<ReqId, FetchSkipError> {
        let camera = ctx
            .device_snapshot
            .as_ref()
            .and_then(|s| s.usb_id.supported_camera());
        let req = self
            .slots
            .request_fetch(ctx.device.as_ref(), camera, &ctx.req)?;
        let order = self.cursor_order();
        self.settle_selection(&order);
        self.prune_editors(&order);
        Ok(req)
    }

    fn settle_selection(&mut self, order: &[SimulationCursor]) {
        if matches!(self.list.selection(), SimulationCursor::None) {
            self.list
                .set_selection(order.first().cloned().unwrap_or(SimulationCursor::None));
        } else {
            self.list.ensure_valid(order);
        }
    }

    pub(super) fn cancel_device_actions(&mut self) {
        self.pending.pushes.clear();
        self.apply = None;
        if self
            .confirm
            .as_ref()
            .is_some_and(|c| c.action.targets_device())
        {
            self.confirm = None;
        }
    }

    pub(super) fn invalidate(&mut self) {
        self.slots.invalidate();
    }

    pub(super) fn handle_simulation_slot_fetched(
        &mut self,
        slot: CustomSetting,
        base: &SimulationBase,
    ) -> Result<(), SlotError> {
        self.slots.handle_fetched(slot, base)
    }

    pub(super) fn handle_simulation_slot_fetch_failed(
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

    pub(super) fn drain_status(&mut self) -> Vec<StatusMessage> {
        take(&mut self.status)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use crossbeam_channel::unbounded;
    use crossterm::event::{KeyEventKind, KeyEventState};
    use fujicore::{
        UsbId,
        generated::{cameras::C_X_S20_SIMULATION, options::CustomSettingName},
    };
    use ratatui_image::picker::Picker;
    use time::OffsetDateTime;

    use super::*;
    use crate::{
        ui::{tabs::TabBehavior, widgets::status::Severity},
        workers::{
            ReqIdGen,
            device::DeviceSnapshot,
            fs::{FsHandle, backup::BackupLibrarySnapshot, simulation::SimulationLibraryEntry},
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

    fn slash() -> KeyEvent {
        key_press(KeyCode::Char('/'))
    }

    fn typed(c: char) -> KeyEvent {
        key_press(KeyCode::Char(c))
    }

    fn ctx() -> &'static AppCtx {
        static CTX: OnceLock<AppCtx> = OnceLock::new();
        CTX.get_or_init(|| {
            let (tx, _rx) = unbounded();
            let dir = tempfile::tempdir().unwrap().keep();
            let fs = FsHandle::spawn(
                dir.join("sim"),
                dir.join("backups"),
                dir.join("renders"),
                tx,
            );
            AppCtx {
                device: None,
                fs,
                req: ReqIdGen::new(),
                device_snapshot: None,
                simulation_library_snapshot: SimulationLibrarySnapshot::empty(),
                backup_library_snapshot: BackupLibrarySnapshot::empty(),
                image_picker: Picker::halfblocks(),
                resize_tx: std::sync::mpsc::channel().0,
                overlay: false,
            }
        })
    }

    fn type_into_filter(s: &mut SimulationTabState, text: &str) {
        for c in text.chars() {
            s.handle_key(ctx(), typed(c));
        }
    }

    fn enumerate(s: &mut SimulationTabState, slots: &[CustomSetting]) {
        s.slots.entries = slots.iter().map(|c| (*c, SlotEntry::Loading)).collect();
        let order = s.cursor_order();
        s.settle_selection(&order);
    }

    fn step(s: &mut SimulationTabState, dir: CursorMove) {
        let order = s.cursor_order();
        s.list.step(dir, &order);
    }

    fn slot_ids(s: &SimulationTabState) -> Vec<CustomSetting> {
        s.list
            .simulation_slot_entries(&s.slots)
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
        enumerate(&mut s, &[CustomSetting::C1]);
        s.handle_simulation_slot_fetched(CustomSetting::C1, base)
            .unwrap();
        s
    }

    fn multi_slot_setup(slots: &[(CustomSetting, Option<&str>)]) -> SimulationTabState {
        let mut s = SimulationTabState::default();
        s.slots.descriptors = Some(&C_X_S20_SIMULATION);
        let ids: Vec<_> = slots.iter().map(|(c, _)| *c).collect();
        enumerate(&mut s, &ids);
        for (slot, name) in slots {
            let base = name.map_or_else(SimulationBase::default, named_sim);
            s.handle_simulation_slot_fetched(*slot, &base).unwrap();
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
        s.handle_key(ctx(), key_press(KeyCode::Enter));
        assert!(s.is_capturing_input());
        s.handle_key(ctx(), key_press(KeyCode::Esc));
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
        assert!(s.list.filter_text().buffer.is_empty());
        assert_eq!(s.list.filter_text().cursor_col, 0);
    }

    #[test]
    fn slash_in_list_pane_opens_filter() {
        let mut s = SimulationTabState::default();
        s.handle_key(ctx(), slash());
        assert!(s.list.filtering());
    }

    #[test]
    fn slash_in_editor_pane_does_not_open_filter() {
        let mut s = loaded_slot(&seeded_string_base());
        assert!(focus_field(&mut s, "Custom Setting Name"));
        assert_eq!(s.focus, Pane::Editor);
        s.handle_key(ctx(), slash());
        assert!(!s.list.filtering());
    }

    #[test]
    fn slash_seeks_cursor_to_end_of_retained_buffer() {
        let mut s = SimulationTabState::default();
        s.handle_key(ctx(), slash());
        type_into_filter(&mut s, "vel");
        s.handle_key(ctx(), key_press(KeyCode::Enter));
        assert!(!s.list.filtering());
        assert_eq!(s.list.filter_text().buffer, "vel");
        s.handle_key(ctx(), slash());
        assert!(s.list.filtering());
        assert_eq!(s.list.filter_text().cursor_col, 3);
    }

    #[test]
    fn typing_into_filter_appends_chars() {
        let mut s = SimulationTabState::default();
        s.handle_key(ctx(), slash());
        type_into_filter(&mut s, "vel");
        assert_eq!(s.list.filter_text().buffer, "vel");
        assert_eq!(s.list.filter_text().cursor_col, 3);
    }

    #[test]
    fn esc_closes_and_clears_filter() {
        let mut s = SimulationTabState::default();
        s.handle_key(ctx(), slash());
        type_into_filter(&mut s, "vel");
        s.handle_key(ctx(), key_press(KeyCode::Esc));
        assert!(!s.list.filtering());
        assert!(s.list.filter_text().buffer.is_empty());
        assert_eq!(s.list.filter_text().cursor_col, 0);
    }

    #[test]
    fn enter_closes_and_keeps_filter() {
        let mut s = SimulationTabState::default();
        s.handle_key(ctx(), slash());
        type_into_filter(&mut s, "vel");
        s.handle_key(ctx(), key_press(KeyCode::Enter));
        assert!(!s.list.filtering());
        assert_eq!(s.list.filter_text().buffer, "vel");
    }

    #[test]
    fn backspace_on_empty_closes_filter() {
        let mut s = SimulationTabState::default();
        s.handle_key(ctx(), slash());
        s.handle_key(ctx(), key_press(KeyCode::Backspace));
        assert!(!s.list.filtering());
    }

    #[test]
    fn backspace_with_chars_removes_last_char_and_stays_open() {
        let mut s = SimulationTabState::default();
        s.handle_key(ctx(), slash());
        type_into_filter(&mut s, "vel");
        s.handle_key(ctx(), key_press(KeyCode::Backspace));
        assert!(s.list.filtering());
        assert_eq!(s.list.filter_text().buffer, "ve");
        assert_eq!(s.list.filter_text().cursor_col, 2);
    }

    #[test]
    fn cursor_keys_move_within_filter_without_closing() {
        let mut s = SimulationTabState::default();
        s.handle_key(ctx(), slash());
        type_into_filter(&mut s, "vel");
        s.handle_key(ctx(), key_press(KeyCode::Home));
        assert_eq!(s.list.filter_text().cursor_col, 0);
        s.handle_key(ctx(), key_press(KeyCode::Right));
        assert_eq!(s.list.filter_text().cursor_col, 1);
        s.handle_key(ctx(), key_press(KeyCode::End));
        assert_eq!(s.list.filter_text().cursor_col, 3);
        assert!(s.list.filtering());
    }

    #[test]
    fn is_capturing_input_true_while_filtering() {
        let mut s = SimulationTabState::default();
        assert!(!s.is_capturing_input());
        s.handle_key(ctx(), slash());
        assert!(s.is_capturing_input());
        s.handle_key(ctx(), key_press(KeyCode::Esc));
        assert!(!s.is_capturing_input());
    }

    #[test]
    fn slot_entries_filter_by_name_substring() {
        let mut s = multi_slot_setup(&[
            (CustomSetting::C1, Some("velvia warm")),
            (CustomSetting::C2, Some("provia")),
            (CustomSetting::C3, Some("velvia cool")),
        ]);
        s.handle_key(ctx(), slash());
        type_into_filter(&mut s, "vel");
        assert_eq!(slot_ids(&s), vec![CustomSetting::C1, CustomSetting::C3]);
    }

    #[test]
    fn slot_entries_hides_unnamed_when_filter_active() {
        let mut s = multi_slot_setup(&[
            (CustomSetting::C1, Some("velvia warm")),
            (CustomSetting::C2, None),
        ]);
        s.handle_key(ctx(), slash());
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
        s.handle_simulation_slot_fetched(CustomSetting::C1, &named_sim("velvia"))
            .unwrap();
        s.handle_key(ctx(), slash());
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
        s.handle_key(ctx(), slash());
        type_into_filter(&mut s, "vel");
        assert_eq!(library_slugs(&s), vec![velvia_cool, velvia_warm]);
    }

    #[test]
    fn filter_matching_is_case_insensitive() {
        let mut s = multi_slot_setup(&[(CustomSetting::C1, Some("VELVIA WARM"))]);
        s.handle_key(ctx(), slash());
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
        s.handle_key(ctx(), slash());
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
        s.handle_key(ctx(), slash());
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
        s.handle_key(ctx(), slash());
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
        assert_eq!(s.list.filter_text().buffer, "vel");
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

        s.handle_key(ctx(), slash());
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
        s.handle_key(ctx(), slash());
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
        s.handle_key(ctx(), slash());
        type_into_filter(&mut s, "vel");
        assert_eq!(slot_ids(&s), vec![CustomSetting::C1]);

        if let Some(SlotEntry::Loaded(buf)) = s.slots.get_mut(CustomSetting::C1) {
            buf.working.canonical.custom_setting_name = Some("astia".parse().unwrap());
        }

        assert!(slot_ids(&s).is_empty());
    }

    fn lib_usb() -> UsbId {
        UsbId {
            vendor: 0x04CB,
            product: 0x02FC,
        }
    }

    fn connected_ctx(usb: UsbId) -> AppCtx {
        let (tx, _rx) = unbounded();
        let dir = tempfile::tempdir().unwrap().keep();
        let fs = FsHandle::spawn(
            dir.join("sim"),
            dir.join("backups"),
            dir.join("renders"),
            tx,
        );
        AppCtx {
            device: None,
            fs,
            req: ReqIdGen::new(),
            device_snapshot: Some(DeviceSnapshot {
                name: "X-S20",
                usb_id: usb,
                bus_address: "0:0".to_owned(),
                battery: 100,
                capabilities: &[],
            }),
            simulation_library_snapshot: SimulationLibrarySnapshot::empty(),
            backup_library_snapshot: BackupLibrarySnapshot::empty(),
            image_picker: Picker::halfblocks(),
            resize_tx: std::sync::mpsc::channel().0,
            overlay: false,
        }
    }

    fn library_only(name: &str) -> (SimulationTabState, Slug) {
        let mut s = SimulationTabState::default();
        let slug = Slug::try_from("velvia-warm").unwrap();
        s.sync_library(&snapshot_with(vec![(
            slug.clone(),
            sample_entry(name, SimulationBase::default()),
        )]));
        s.list
            .set_selection(SimulationCursor::Library(slug.clone()));
        (s, slug)
    }

    #[test]
    fn keybinds_track_selection_kind() {
        let mut s = multi_slot_setup(&[(CustomSetting::C1, Some("a"))]);
        assert_eq!(s.keybinds(), SLOT_KEYBINDS);
        let (lib, slug) = library_only("Velvia Warm");
        s.library = lib.library;
        s.list.set_selection(SimulationCursor::Library(slug));
        assert_eq!(s.keybinds(), LIBRARY_KEYBINDS);
        s.focus = Pane::Editor;
        assert_eq!(s.keybinds(), EDITOR_KEYBINDS);
    }

    #[test]
    fn revert_discards_slot_edits() {
        let mut s = loaded_slot(&seeded_string_base());
        if let Some(SlotEntry::Loaded(buf)) = s.slots.get_mut(CustomSetting::C1) {
            buf.working.canonical.custom_setting_name = Some("changed".parse().unwrap());
        }
        assert!(matches!(
            s.slots.get(CustomSetting::C1),
            Some(SlotEntry::Loaded(b)) if b.dirty()
        ));
        s.list
            .set_selection(SimulationCursor::Slot(CustomSetting::C1));
        s.handle_key(ctx(), typed('u'));
        assert!(matches!(
            s.slots.get(CustomSetting::C1),
            Some(SlotEntry::Loaded(b)) if !b.dirty()
        ));
    }

    #[test]
    fn settle_pushed_slot_clears_pending() {
        let mut s = loaded_slot(&seeded_string_base());
        s.pending.pushes.insert(CustomSetting::C1);
        s.handle_simulation_slot_changed(ctx(), CustomSetting::C1);
        assert!(s.pending.pushes.is_empty());
    }

    #[test]
    fn failed_push_clears_pending_but_keeps_edits() {
        let mut s = loaded_slot(&seeded_string_base());
        if let Some(SlotEntry::Loaded(buf)) = s.slots.get_mut(CustomSetting::C1) {
            buf.working.canonical.custom_setting_name = Some("changed".parse().unwrap());
        }
        s.pending.pushes.insert(CustomSetting::C1);
        s.handle_simulation_slot_push_failed(CustomSetting::C1);
        assert!(s.pending.pushes.is_empty());
        assert!(matches!(
            s.slots.get(CustomSetting::C1),
            Some(SlotEntry::Loaded(b)) if b.dirty()
        ));
    }

    #[test]
    fn targeted_refetch_rebuilds_slot_from_camera_truth() {
        let mut s = loaded_slot(&seeded_string_base());
        if let Some(SlotEntry::Loaded(buf)) = s.slots.get_mut(CustomSetting::C1) {
            buf.working.canonical.custom_setting_name = Some("changed".parse().unwrap());
        }
        assert!(matches!(
            s.slots.get(CustomSetting::C1),
            Some(SlotEntry::Loaded(b)) if b.dirty()
        ));
        if let Some(entry) = s.slots.get_mut(CustomSetting::C1) {
            *entry = SlotEntry::Loading;
        }
        s.handle_simulation_slot_fetched(CustomSetting::C1, &seeded_string_base())
            .unwrap();
        assert!(matches!(
            s.slots.get(CustomSetting::C1),
            Some(SlotEntry::Loaded(b)) if !b.dirty()
        ));
    }

    #[test]
    fn pushes_queue_per_slot_and_clear_independently() {
        let mut s = loaded_slot(&seeded_string_base());
        s.pending.pushes.insert(CustomSetting::C1);
        s.pending.pushes.insert(CustomSetting::C2);
        s.handle_simulation_slot_changed(ctx(), CustomSetting::C1);
        assert!(!s.pending.pushes.contains(&CustomSetting::C1));
        assert!(s.pending.pushes.contains(&CustomSetting::C2));
        s.handle_simulation_slot_changed(ctx(), CustomSetting::C2);
        assert!(s.pending.pushes.is_empty());
    }

    #[test]
    fn rename_blocked_while_dirty_with_status_error() {
        let (mut s, slug) = library_only("Velvia Warm");
        if let Some(lib) = s.library.get_mut(&slug) {
            lib.buffer.working.canonical = named_sim("Velvia Cool");
        }
        assert!(s.library.get(&slug).unwrap().buffer.dirty());

        s.handle_key(ctx(), typed('r'));

        assert!(s.rename.is_none(), "rename must be refused while dirty");
        let posted = s.drain_status();
        assert_eq!(posted.len(), 1);
        assert_eq!(posted[0].severity, Severity::Error);
    }

    #[test]
    fn rename_follows_entry_to_new_slug() {
        let (mut s, old_slug) = library_only("Velvia Warm");
        let new_slug = Slug::try_from("renamed").unwrap();
        s.sync_library(&snapshot_with(vec![(
            new_slug.clone(),
            sample_entry("Renamed", SimulationBase::default()),
        )]));
        s.handle_simulation_library_entry_renamed(&old_slug, &new_slug);
        assert_eq!(s.list.selection(), &SimulationCursor::Library(new_slug));
    }

    #[test]
    fn save_records_pending_then_settle_clears_dirty() {
        let (mut s, slug) = library_only("Velvia Warm");
        if let Some(lib) = s.library.get_mut(&slug) {
            lib.buffer.working.canonical = named_sim("Velvia Cool");
        }
        assert!(s.library.get(&slug).unwrap().buffer.dirty());

        let ctx = connected_ctx(lib_usb());
        s.handle_key(&ctx, typed('w'));
        assert_eq!(s.pending.saves.len(), 1);

        let req = *s.pending.saves.keys().next().expect("one pending save");
        s.handle_simulation_library_entry_saved(req, &slug).unwrap();
        assert!(s.pending.saves.is_empty());
        assert!(!s.library.get(&slug).unwrap().buffer.dirty());
    }

    #[test]
    fn settle_saved_reports_foreign_req() {
        let (mut s, slug) = library_only("Velvia Warm");
        if let Some(lib) = s.library.get_mut(&slug) {
            lib.buffer.working.canonical = named_sim("Velvia Cool");
        }

        let foreign = ReqIdGen::new().next();
        let result = s.handle_simulation_library_entry_saved(foreign, &slug);
        assert!(matches!(result, Err(LibraryAnomaly::UnexpectedSave { .. })));
        assert!(s.library.get(&slug).unwrap().buffer.dirty());
    }

    #[test]
    fn rename_opens_for_library_not_slot() {
        let (mut s, _slug) = library_only("Velvia Warm");
        s.handle_key(ctx(), typed('r'));
        assert!(s.rename.is_some());

        let mut slot = multi_slot_setup(&[(CustomSetting::C1, Some("a"))]);
        slot.handle_key(ctx(), typed('r'));
        assert!(slot.rename.is_none());
    }

    #[test]
    fn delete_opens_confirm_for_library() {
        let (mut s, _slug) = library_only("Velvia Warm");
        s.handle_key(ctx(), key_press(KeyCode::Char('D')));
        assert!(s.confirm.is_some());
    }

    #[test]
    fn apply_opens_picker_then_confirms_on_matching_camera() {
        let mut s = multi_slot_setup(&[(CustomSetting::C1, Some("a"))]);
        let (lib, slug) = library_only("Velvia Warm");
        s.library = lib.library;
        s.list.set_selection(SimulationCursor::Library(slug));
        let ctx = connected_ctx(lib_usb());
        s.handle_key(&ctx, typed('a'));
        assert!(s.apply.is_some());
        s.handle_key(&ctx, key_press(KeyCode::Enter));
        assert!(s.apply.is_none());
        assert!(s.confirm.is_some());
    }

    #[test]
    fn apply_skipped_on_camera_mismatch() {
        let mut s = multi_slot_setup(&[(CustomSetting::C1, Some("a"))]);
        let (lib, slug) = library_only("Velvia Warm");
        s.library = lib.library;
        s.list.set_selection(SimulationCursor::Library(slug));
        let ctx = connected_ctx(UsbId {
            vendor: 0xFFFF,
            product: 0xFFFF,
        });
        s.handle_key(&ctx, typed('a'));
        assert!(s.apply.is_none());
    }

    #[test]
    fn apply_excludes_unloaded_slots() {
        let mut s = multi_slot_setup(&[(CustomSetting::C1, Some("a"))]);
        if let Some(entry) = s.slots.get_mut(CustomSetting::C1) {
            *entry = SlotEntry::Loading;
        }
        let (lib, slug) = library_only("Velvia Warm");
        s.library = lib.library;
        s.list.set_selection(SimulationCursor::Library(slug));
        let ctx = connected_ctx(lib_usb());
        s.handle_key(&ctx, typed('a'));
        assert!(s.apply.is_none());
        let posted = s.drain_status();
        assert_eq!(posted.len(), 1);
        assert_eq!(posted[0].severity, Severity::Error);
    }

    #[test]
    fn capturing_input_true_while_rename_open() {
        let (mut s, _slug) = library_only("Velvia Warm");
        assert!(!s.capturing_input());
        s.handle_key(ctx(), typed('r'));
        assert!(s.capturing_input());
        s.handle_key(ctx(), key_press(KeyCode::Esc));
        assert!(!s.capturing_input());
    }

    #[test]
    fn default_library_name_uses_slot_name_then_disambiguates() {
        let sim = named_sim("Velvia Warm");
        let first =
            SimulationTabState::default_library_name(&sim, &SimulationLibrarySnapshot::default());
        assert_eq!(first, "Velvia Warm");

        let slug = Slug::try_from(first.as_str()).unwrap();
        let snapshot = snapshot_with(vec![(slug, sample_entry("Velvia Warm", sim.clone()))]);
        let second = SimulationTabState::default_library_name(&sim, &snapshot);
        assert_ne!(second, first);
    }

    #[test]
    fn default_library_name_falls_back_when_slot_unnamed() {
        let name = SimulationTabState::default_library_name(
            &SimulationBase::default(),
            &SimulationLibrarySnapshot::default(),
        );
        assert_eq!(name, "simulation");
    }

    #[test]
    fn rename_to_same_slug_registers_save_not_anomaly() {
        let (mut s, slug) = library_only("Velvia Warm");
        s.handle_key(ctx(), typed('r'));
        assert!(s.rename.is_some());
        s.handle_key(ctx(), key_press(KeyCode::Enter));
        assert_eq!(s.pending.saves.len(), 1);
        let req = *s.pending.saves.keys().next().expect("one pending save");
        assert!(s.handle_simulation_library_entry_saved(req, &slug).is_ok());
        assert!(s.pending.saves.is_empty());
    }

    #[test]
    fn second_save_to_dirty_entry_is_refused() {
        let (mut s, slug) = library_only("Velvia Warm");
        if let Some(lib) = s.library.get_mut(&slug) {
            lib.buffer.working.canonical = named_sim("Velvia Cool");
        }
        let ctx = connected_ctx(lib_usb());
        s.handle_key(&ctx, typed('w'));
        assert_eq!(s.pending.saves.len(), 1);
        let _ = s.drain_status();

        s.handle_key(&ctx, typed('w'));
        assert_eq!(s.pending.saves.len(), 1);
        let posted = s.drain_status();
        assert_eq!(posted.len(), 1);
        assert_eq!(posted[0].severity, Severity::Error);
    }

    #[test]
    fn second_save_to_library_for_slot_is_refused() {
        let mut s = loaded_slot(&seeded_string_base());
        s.list
            .set_selection(SimulationCursor::Slot(CustomSetting::C1));
        let ctx = connected_ctx(lib_usb());
        s.handle_key(&ctx, typed('s'));
        assert_eq!(s.pending.adds.len(), 1);
        let _ = s.drain_status();

        s.handle_key(&ctx, typed('s'));
        assert_eq!(s.pending.adds.len(), 1);
        let posted = s.drain_status();
        assert_eq!(posted.len(), 1);
        assert_eq!(posted[0].severity, Severity::Error);
    }

    #[test]
    fn library_add_settles_and_clears_pending() {
        let mut s = loaded_slot(&seeded_string_base());
        s.list
            .set_selection(SimulationCursor::Slot(CustomSetting::C1));
        let ctx = connected_ctx(lib_usb());
        s.handle_key(&ctx, typed('s'));
        let req = *s.pending.adds.keys().next().expect("one pending add");
        s.handle_simulation_library_entry_added(req, &Slug::try_from("velvia-warm").unwrap());
        assert!(s.pending.adds.is_empty());
    }

    #[test]
    fn library_op_failure_clears_pending_add() {
        let mut s = loaded_slot(&seeded_string_base());
        s.list
            .set_selection(SimulationCursor::Slot(CustomSetting::C1));
        let ctx = connected_ctx(lib_usb());
        s.handle_key(&ctx, typed('s'));
        let req = *s.pending.adds.keys().next().expect("one pending add");
        s.handle_simulation_library_op_failed(req);
        assert!(s.pending.adds.is_empty());
    }

    #[test]
    fn disconnect_cancels_apply_modal() {
        let mut s = multi_slot_setup(&[(CustomSetting::C1, Some("a"))]);
        let (lib, slug) = library_only("Velvia Warm");
        s.library = lib.library;
        s.list.set_selection(SimulationCursor::Library(slug));
        let ctx = connected_ctx(lib_usb());
        s.handle_key(&ctx, typed('a'));
        assert!(s.apply.is_some());

        s.cancel_device_actions();
        assert!(s.apply.is_none());
    }

    #[test]
    fn disconnect_cancels_apply_confirm_but_keeps_delete() {
        let mut s = multi_slot_setup(&[(CustomSetting::C1, Some("a"))]);
        let (lib, slug) = library_only("Velvia Warm");
        s.library = lib.library;
        s.list.set_selection(SimulationCursor::Library(slug));
        let connected = connected_ctx(lib_usb());
        s.handle_key(&connected, typed('a'));
        s.handle_key(&connected, key_press(KeyCode::Enter));
        assert!(matches!(
            s.confirm,
            Some(PendingConfirm {
                action: ConfirmAction::Apply { .. },
                ..
            })
        ));
        s.cancel_device_actions();
        assert!(s.confirm.is_none());

        let (mut s2, _slug) = library_only("Velvia Warm");
        s2.handle_key(ctx(), key_press(KeyCode::Char('D')));
        assert!(matches!(
            s2.confirm,
            Some(PendingConfirm {
                action: ConfirmAction::Delete(_),
                ..
            })
        ));
        s2.cancel_device_actions();
        assert!(s2.confirm.is_some());
    }
}
