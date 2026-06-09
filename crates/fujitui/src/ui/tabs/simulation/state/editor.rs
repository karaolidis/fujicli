use std::ptr;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fujicore::{
    features::simulation::{
        Direction, EnumOps, Extreme, Magnitude, OptionDescriptor, OptionOps, SetOutcome,
        SimulationDescriptors,
    },
    generated::{options::OptionCategory, simulations::SimulationBase},
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::{
    border_title,
    ui::{border_style, tabs::Buffer},
};

use super::{
    CursorMove, DIRTY_MARKER, EditorTarget, FILTER_PROMPT, INDENT, SimulationState, TextInputState,
    draw_scrollbar, make_buffer_with_cursor,
};

pub(super) enum EditorOutcome {
    Continue,
    ExitToList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorAction {
    Bump(Direction),
    BigBump(Direction),
    Jump(Extreme),
}

#[derive(Debug)]
pub(super) struct InlineEdit {
    pub descriptor: &'static OptionDescriptor<SimulationBase>,
    pub status: InlineStatus,
    pub kind: InlineKind,
}

#[derive(Debug)]
pub(super) enum InlineKind {
    TextInput(TextInputState),
    Picker(PickerState),
}

#[derive(Debug)]
pub(super) struct PickerState {
    pub filter: String,
    pub cursor_row: usize,
    pub rows: Vec<PickerRow>,
    pub scroll: usize,
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

impl PickerState {
    fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        anchor_row: usize,
        fields_offset: usize,
        field_name: &str,
        dirty: bool,
        status: InlineStatus,
    ) {
        let inner_x = area.x + 1;
        let inner_y = area.y + 1;
        let inner_w = area.width.saturating_sub(2);
        let inner_h = area.height.saturating_sub(2);
        if inner_w == 0 || inner_h == 0 {
            return;
        }

        let visible = self.visible_rows();
        let rows = u16::try_from(visible.len().max(1)).unwrap_or(u16::MAX);
        let wanted = rows.saturating_add(3).min(14);

        let field_y =
            inner_y + u16::try_from(anchor_row.saturating_sub(fields_offset)).unwrap_or(0);
        let inner_bottom = inner_y + inner_h;
        let room_below = inner_bottom.saturating_sub(field_y);
        let room_above = field_y - inner_y + 1;
        let (popup_y, popup_h) = if room_below >= room_above {
            (field_y, wanted.min(room_below))
        } else {
            let h = wanted.min(room_above);
            ((field_y + 1).saturating_sub(h), h)
        };
        if popup_h < 3 {
            return;
        }

        let popup = Rect {
            x: inner_x,
            y: popup_y,
            width: inner_w,
            height: popup_h,
        };
        frame.render_widget(Clear, popup);
        let border = match status {
            InlineStatus::Rejected => Style::default().fg(Color::Red),
            InlineStatus::Idle => border_style(true),
        };
        let title = if dirty {
            Line::from(Span::styled(
                border_title!(1, "{DIRTY_MARKER} {field_name}"),
                Style::default().add_modifier(Modifier::ITALIC),
            ))
        } else {
            Line::from(Span::raw(border_title!(1, "{field_name}")))
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border)
            .title(title);
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let [filter_area, values_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);
        let mut filter_spans = vec![Span::styled(
            FILTER_PROMPT,
            Style::default().fg(Color::DarkGray),
        )];
        filter_spans.extend(make_buffer_with_cursor(
            &self.filter,
            self.filter.chars().count(),
            Style::default(),
        ));
        frame.render_widget(Paragraph::new(Line::from(filter_spans)), filter_area);

        if visible.is_empty() {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "(no matches)",
                    Style::default().fg(Color::DarkGray),
                ))),
                values_area,
            );
            return;
        }

        let items: Vec<ListItem<'static>> = visible
            .iter()
            .map(|row| ListItem::new(Line::from(Span::raw(row.label.to_owned()))))
            .collect();
        let mut state = ListState::default().with_offset(self.scroll);
        state.select(Some(self.cursor_row));
        let list =
            List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        frame.render_stateful_widget(list, values_area, &mut state);
        let new_offset = state.offset();
        draw_scrollbar(
            frame,
            Rect {
                x: popup.x,
                y: values_area.y,
                width: popup.width,
                height: values_area.height,
            },
            visible.len(),
            new_offset,
        );
        self.scroll = new_offset;
    }

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

    fn visible_rows(&self) -> Vec<&PickerRow> {
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
                        text.move_left();
                        EditModeOutcome::Continue
                    }
                    KeyCode::Right => {
                        text.move_right();
                        EditModeOutcome::Continue
                    }
                    KeyCode::Home => {
                        text.move_home();
                        EditModeOutcome::Continue
                    }
                    KeyCode::End => {
                        text.move_end();
                        EditModeOutcome::Continue
                    }
                    KeyCode::Backspace => {
                        if text.delete_before() {
                            *status = InlineStatus::Idle;
                        }
                        EditModeOutcome::Continue
                    }
                    KeyCode::Delete => {
                        if text.delete_after() {
                            *status = InlineStatus::Idle;
                        }
                        EditModeOutcome::Continue
                    }
                    KeyCode::Char(c) if !c.is_control() => {
                        if text.insert(c, max_len) {
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

#[derive(Debug, Default)]
pub(super) struct EditorState {
    field: usize,
    scroll: usize,
    edit: Option<InlineEdit>,
}

impl EditorState {
    #[cfg(test)]
    pub(super) const fn set_field(&mut self, field: usize) {
        self.field = field;
    }

    #[cfg(test)]
    const fn editing(&self) -> Option<&InlineEdit> {
        self.edit.as_ref()
    }

    pub(super) const fn is_editing(&self) -> bool {
        self.edit.is_some()
    }

    fn cursor(
        &self,
        descriptors: &SimulationDescriptors,
        canonical: &SimulationBase,
    ) -> Option<usize> {
        let count = descriptors.visible_fields(canonical).len();
        (count > 0).then(|| self.field.min(count - 1))
    }

    pub(super) fn handle_key(
        &mut self,
        key: KeyEvent,
        buffer: &mut Buffer<SimulationState>,
        descriptors: &'static SimulationDescriptors,
    ) -> EditorOutcome {
        if self.edit.is_some() {
            self.handle_edit_key(key, buffer, descriptors);
            return EditorOutcome::Continue;
        }
        let max = descriptors.visible_fields(&buffer.working.canonical).len();
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.step(CursorMove::Up, max),
            KeyCode::Down | KeyCode::Char('j') => self.step(CursorMove::Down, max),
            KeyCode::Esc => return EditorOutcome::ExitToList,
            KeyCode::Left => self.apply(
                {
                    let dir = Direction::Prev;
                    if shift {
                        EditorAction::BigBump(dir)
                    } else {
                        EditorAction::Bump(dir)
                    }
                },
                buffer,
                descriptors,
            ),
            KeyCode::Right => self.apply(
                {
                    let dir = Direction::Next;
                    if shift {
                        EditorAction::BigBump(dir)
                    } else {
                        EditorAction::Bump(dir)
                    }
                },
                buffer,
                descriptors,
            ),
            KeyCode::Home => self.apply(EditorAction::Jump(Extreme::Min), buffer, descriptors),
            KeyCode::End => self.apply(EditorAction::Jump(Extreme::Max), buffer, descriptors),
            KeyCode::Enter => self.enter_edit(buffer, descriptors),
            _ => {}
        }
        EditorOutcome::Continue
    }

    fn step(&mut self, dir: CursorMove, max: usize) {
        if max == 0 {
            return;
        }
        let current = self.field.min(max - 1);
        self.field = match dir {
            CursorMove::Up => current.saturating_sub(1),
            CursorMove::Down => (current + 1).min(max - 1),
        };
    }

    fn focused_descriptor(
        &self,
        descriptors: &SimulationDescriptors,
        canonical: &SimulationBase,
    ) -> Option<&'static OptionDescriptor<SimulationBase>> {
        let visible = descriptors.visible_fields(canonical);
        if visible.is_empty() {
            return None;
        }
        visible.get(self.field.min(visible.len() - 1)).copied()
    }

    fn apply(
        &self,
        action: EditorAction,
        buffer: &mut Buffer<SimulationState>,
        descriptors: &'static SimulationDescriptors,
    ) {
        let Some(desc) = self.focused_descriptor(descriptors, &buffer.working.canonical) else {
            return;
        };
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

    fn enter_edit(
        &mut self,
        buffer: &Buffer<SimulationState>,
        descriptors: &'static SimulationDescriptors,
    ) {
        let canonical = &buffer.working.canonical;
        let Some(desc) = self.focused_descriptor(descriptors, canonical) else {
            return;
        };
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
                    scroll: 0,
                }))
            }
            _ => None,
        };
        self.edit = kind.map(|kind| InlineEdit {
            descriptor: desc,
            status: InlineStatus::Idle,
            kind,
        });
    }

    fn handle_edit_key(
        &mut self,
        key: KeyEvent,
        buffer: &mut Buffer<SimulationState>,
        descriptors: &'static SimulationDescriptors,
    ) {
        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        match edit.handle_key(key) {
            EditModeOutcome::Continue => {}
            EditModeOutcome::Cancel => self.edit = None,
            EditModeOutcome::CommitText(text) => self.commit_text(&text, buffer, descriptors),
            EditModeOutcome::CommitPick(id) => self.commit_pick(id, buffer, descriptors),
        }
    }

    fn commit_text(
        &mut self,
        text: &str,
        buffer: &mut Buffer<SimulationState>,
        descriptors: &'static SimulationDescriptors,
    ) {
        let Some(desc) = self.edit.as_ref().map(|e| e.descriptor) else {
            return;
        };
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
        self.finish_commit(&outcome);
    }

    fn commit_pick(
        &mut self,
        id: &'static str,
        buffer: &mut Buffer<SimulationState>,
        descriptors: &'static SimulationDescriptors,
    ) {
        let Some(desc) = self.edit.as_ref().map(|e| e.descriptor) else {
            return;
        };
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
        self.finish_commit(&outcome);
    }

    fn finish_commit(&mut self, outcome: &SetOutcome) {
        match outcome {
            SetOutcome::Set => self.edit = None,
            SetOutcome::InvalidInput(_) | SetOutcome::Rejected => {
                if let Some(edit) = self.edit.as_mut() {
                    edit.status = InlineStatus::Rejected;
                }
            }
        }
    }

    pub(super) fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        active: bool,
        target: EditorTarget<'_>,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(active));
        match target {
            EditorTarget::None => {
                Self::draw_centered_message(
                    frame,
                    area,
                    block,
                    "(no entry selected)",
                    Color::DarkGray,
                );
            }
            EditorTarget::Loading { title } => Self::draw_centered_message(
                frame,
                area,
                block.title(border_title!(1, "{title}")),
                "loading...",
                Color::DarkGray,
            ),
            EditorTarget::Failed { title, error } => Self::draw_centered_message(
                frame,
                area,
                block.title(border_title!(1, "{title}")),
                &format!("fetch failed: {error}"),
                Color::Red,
            ),
            EditorTarget::Ready {
                title,
                working,
                fetched,
                descriptors,
                dirty,
            } => self.draw_fields(
                frame,
                area,
                block,
                &title,
                working,
                fetched,
                descriptors,
                dirty,
                active,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_fields(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        block: Block<'_>,
        title: &str,
        working: &SimulationState,
        fetched: &SimulationBase,
        descriptors: &'static SimulationDescriptors,
        dirty: bool,
        active: bool,
    ) {
        let cursor = if active {
            self.cursor(descriptors, &working.canonical)
        } else {
            None
        };
        let title = if dirty {
            Line::from(Span::styled(
                border_title!(1, "{DIRTY_MARKER} {title}"),
                Style::default().add_modifier(Modifier::ITALIC),
            ))
        } else {
            Line::from(Span::raw(border_title!(1, "{title}")))
        };
        let inner_width = area.width.saturating_sub(2);
        let (items, scroll_target) = self.make_field_items(
            descriptors,
            &working.canonical,
            fetched,
            cursor,
            inner_width,
        );
        let content_len = items.len();
        let list = List::new(items).block(block.title(title));
        let mut list_state = ListState::default().with_offset(self.scroll);
        list_state.select(scroll_target);
        frame.render_stateful_widget(list, area, &mut list_state);
        self.scroll = list_state.offset();
        draw_scrollbar(
            frame,
            Rect {
                x: area.x,
                y: area.y + 1,
                width: area.width,
                height: area.height.saturating_sub(2),
            },
            content_len,
            self.scroll,
        );

        if let Some(InlineEdit {
            descriptor,
            status,
            kind: InlineKind::Picker(picker),
        }) = self.edit.as_mut()
        {
            let field_dirty = !(descriptor.eq)(&working.canonical, fetched);
            picker.draw(
                frame,
                area,
                scroll_target.unwrap_or(0),
                self.scroll,
                descriptor.name,
                field_dirty,
                *status,
            );
        }
    }

    fn make_field_items(
        &self,
        descriptors: &SimulationDescriptors,
        canonical: &SimulationBase,
        fetched: &SimulationBase,
        cursor: Option<usize>,
        inner_width: u16,
    ) -> (Vec<ListItem<'static>>, Option<usize>) {
        let editing = self.edit.as_ref();
        let visible = descriptors.visible_fields(canonical);
        let mut items: Vec<ListItem<'static>> = Vec::new();
        let mut scroll_target = None;
        let mut current_category: Option<Option<OptionCategory>> = None;
        let mut first_group = true;

        for (field_idx, field) in visible.iter().enumerate() {
            if current_category != Some(field.category) {
                if !first_group {
                    items.push(ListItem::new(""));
                }
                if let Some(c) = field.category {
                    items.push(ListItem::new(Line::from(Span::styled(
                        c.to_string(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ))));
                }
                current_category = Some(field.category);
                first_group = false;
            }
            let prefix = if field.category.is_some() { INDENT } else { "" };
            let editing_here = editing.is_some_and(|e| ptr::eq(e.descriptor, *field));
            if editing_here {
                let edit = editing.expect("editing_here; editing is Some");
                scroll_target = Some(items.len());
                match &edit.kind {
                    InlineKind::TextInput(text) => {
                        items.push(Self::make_text_input_item(
                            prefix,
                            field.name,
                            text,
                            edit.status,
                            inner_width,
                        ));
                    }
                    InlineKind::Picker(_) => {
                        let value = (field.display)(canonical).unwrap_or_default();
                        let dirty = !(field.eq)(canonical, fetched);
                        items.push(Self::make_field_item(
                            prefix,
                            field.name,
                            value,
                            true,
                            dirty,
                            inner_width,
                        ));
                    }
                }
            } else {
                let value = (field.display)(canonical).expect("visible field has display");
                let dirty = !(field.eq)(canonical, fetched);
                if cursor == Some(field_idx) {
                    scroll_target = Some(items.len());
                }
                items.push(Self::make_field_item(
                    prefix,
                    field.name,
                    value,
                    cursor == Some(field_idx),
                    dirty,
                    inner_width,
                ));
            }
        }

        (items, scroll_target)
    }

    fn mirror_and_settle(
        state: &mut SimulationState,
        edited: &OptionDescriptor<SimulationBase>,
        descriptors: &SimulationDescriptors,
    ) {
        (edited.copy_from)(&mut state.shadow, &state.canonical);
        Self::settle(state, descriptors, Some(edited));
    }

    pub(super) fn settle(
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

    fn make_field_item(
        prefix: &'static str,
        name: &'static str,
        value: String,
        highlight: bool,
        dirty: bool,
        inner_width: u16,
    ) -> ListItem<'static> {
        let marker = if dirty {
            format!("{DIRTY_MARKER} ")
        } else {
            String::new()
        };
        let label_w = prefix.chars().count() + marker.chars().count() + name.chars().count();
        let value_w = value.chars().count();
        let gap = (inner_width as usize).saturating_sub(label_w + value_w);
        let dots_w = gap.saturating_sub(2);
        let dots: String = (0..dots_w)
            .map(|i| if i % 2 == 0 { '.' } else { ' ' })
            .collect();
        let mut text_style = Style::default();
        if highlight {
            text_style = text_style.add_modifier(Modifier::REVERSED);
        }
        if dirty {
            text_style = text_style.add_modifier(Modifier::ITALIC);
        }
        let dots_style = Style::default().fg(Color::DarkGray);
        ListItem::new(Line::from(vec![
            Span::raw(prefix),
            Span::styled(marker, text_style),
            Span::styled(name, text_style),
            Span::raw(" "),
            Span::styled(dots, dots_style),
            Span::raw(" "),
            Span::styled(value, text_style),
        ]))
    }

    fn make_text_input_item(
        prefix: &'static str,
        name: &'static str,
        text: &TextInputState,
        status: InlineStatus,
        inner_width: u16,
    ) -> ListItem<'static> {
        let label_w = prefix.chars().count() + name.chars().count();
        let buf_w = text.buffer.chars().count().max(text.cursor_col + 1);
        let gap = (inner_width as usize).saturating_sub(label_w + buf_w);
        let dots_w = gap.saturating_sub(2);
        let dots: String = (0..dots_w)
            .map(|i| if i % 2 == 0 { '.' } else { ' ' })
            .collect();
        let dots_style = Style::default().fg(Color::DarkGray);

        let mut spans = vec![
            Span::raw(prefix),
            Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(dots, dots_style),
            Span::raw(" "),
        ];
        let base = match status {
            InlineStatus::Idle => Style::default(),
            InlineStatus::Rejected => Style::default().fg(Color::Red),
        };
        spans.extend(make_buffer_with_cursor(&text.buffer, text.cursor_col, base));
        ListItem::new(Line::from(spans))
    }

    fn draw_centered_message(
        frame: &mut Frame,
        area: Rect,
        block: Block<'_>,
        text: &str,
        color: Color,
    ) {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let [centered] = Layout::vertical([Constraint::Length(1)])
            .flex(Flex::Center)
            .areas(inner);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                text.to_owned(),
                Style::default().fg(color),
            )))
            .alignment(Alignment::Center),
            centered,
        );
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyEventKind, KeyEventState};
    use fujicore::{
        features::simulation::{BumpError, VariantInfo},
        generated::{
            cameras::C_X_S20_SIMULATION,
            options::{Clarity, CustomSettingName, FilmSimulation, MonochromaticColorTemperature},
        },
    };

    use super::*;

    const DESC: &SimulationDescriptors = &C_X_S20_SIMULATION;

    fn buffer(base: SimulationBase) -> Buffer<SimulationState> {
        let shadow = DESC.new_shadow_from(&base);
        Buffer::from(SimulationState {
            canonical: base,
            shadow,
        })
    }

    fn focus(editor: &mut EditorState, buf: &Buffer<SimulationState>, name: &str) -> bool {
        match DESC
            .visible_fields(&buf.working.canonical)
            .iter()
            .position(|f| f.name == name)
        {
            Some(idx) => {
                editor.field = idx;
                true
            }
            None => false,
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    fn seeded_string_base() -> SimulationBase {
        SimulationBase {
            custom_setting_name: Some(CustomSettingName::default()),
            ..Default::default()
        }
    }

    fn picker(editor: &EditorState) -> &PickerState {
        match editor.editing() {
            Some(InlineEdit {
                kind: InlineKind::Picker(picker),
                ..
            }) => picker,
            _ => panic!("expected picker"),
        }
    }

    fn text(editor: &EditorState) -> &TextInputState {
        match editor.editing() {
            Some(InlineEdit {
                kind: InlineKind::TextInput(text),
                ..
            }) => text,
            _ => panic!("expected text input"),
        }
    }

    #[test]
    fn bump_changes_canonical_and_marks_dirty() {
        let mut buf = buffer(SimulationBase {
            clarity: Some(Clarity::default()),
            ..Default::default()
        });
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Clarity"));
        let before = buf.working.canonical.clarity;
        editor.handle_key(key(KeyCode::Right), &mut buf, DESC);
        assert_ne!(before, buf.working.canonical.clarity);
        assert!(buf.dirty());
    }

    #[test]
    fn bump_at_max_silent() {
        let at_max = Clarity::try_from(Clarity::MAX).unwrap();
        let mut buf = buffer(SimulationBase {
            clarity: Some(at_max),
            ..Default::default()
        });
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Clarity"));
        editor.handle_key(key(KeyCode::Right), &mut buf, DESC);
        assert_eq!(buf.working.canonical.clarity, Some(at_max));
        assert!(!buf.dirty());
    }

    #[test]
    fn enter_edit_on_string_starts_text_input() {
        let mut buf = buffer(seeded_string_base());
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Custom Setting Name"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        assert!(matches!(
            editor.editing(),
            Some(InlineEdit {
                kind: InlineKind::TextInput(_),
                ..
            })
        ));
    }

    #[test]
    fn enter_edit_on_enum_starts_picker_with_reachable_rows() {
        let mut buf = buffer(SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Film Simulation"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        assert!(!picker(&editor).rows.is_empty());
    }

    #[test]
    fn picker_compute_rows_filters_out_variants_set_by_id_rejects() {
        fn cycle_unused(
            _: &mut SimulationBase,
            _: Direction,
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
        let mut buf = buffer(SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Film Simulation"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        let velvia = picker(&editor)
            .rows
            .iter()
            .find(|r| r.label == "Velvia")
            .expect("Velvia present");
        assert_eq!(velvia.id, "velvia");
    }

    #[test]
    fn commit_pick_lands_via_canonical_id() {
        let mut buf = buffer(SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Film Simulation"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        editor.commit_pick("velvia", &mut buf, DESC);
        assert_eq!(
            buf.working.canonical.film_simulation,
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
            scroll: 0,
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
        let mut buf = buffer(seeded_string_base());
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Custom Setting Name"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        editor.handle_key(key(KeyCode::Char('A')), &mut buf, DESC);
        editor.handle_key(key(KeyCode::Char('B')), &mut buf, DESC);
        let text = text(&editor);
        assert!(text.buffer.ends_with("AB"));
        assert_eq!(text.cursor_col, text.buffer.chars().count());
    }

    #[test]
    fn text_input_backspace_deletes() {
        let mut buf = buffer(seeded_string_base());
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Custom Setting Name"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        editor.handle_key(key(KeyCode::Char('A')), &mut buf, DESC);
        editor.handle_key(key(KeyCode::Backspace), &mut buf, DESC);
        assert!(!text(&editor).buffer.ends_with('A'));
    }

    #[test]
    fn text_input_esc_cancels() {
        let mut buf = buffer(seeded_string_base());
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Custom Setting Name"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        editor.handle_key(key(KeyCode::Esc), &mut buf, DESC);
        assert!(editor.editing().is_none());
    }

    #[test]
    fn text_input_accepts_digits() {
        let mut buf = buffer(seeded_string_base());
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Custom Setting Name"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        for c in "123".chars() {
            editor.handle_key(key(KeyCode::Char(c)), &mut buf, DESC);
        }
        assert!(text(&editor).buffer.ends_with("123"));
    }

    #[test]
    fn picker_starts_on_currently_set_variant() {
        let mut buf = buffer(SimulationBase {
            film_simulation: Some(FilmSimulation::Velvia),
            ..Default::default()
        });
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Film Simulation"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        let picker = picker(&editor);
        assert!(picker.cursor_row > 0);
        assert_eq!(picker.rows[picker.cursor_row].label, "Velvia");
    }

    #[test]
    fn picker_arrows_walk_visible_rows() {
        let mut buf = buffer(SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Film Simulation"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        editor.handle_key(key(KeyCode::Down), &mut buf, DESC);
        assert_eq!(picker(&editor).cursor_row, 1);
        editor.handle_key(key(KeyCode::Up), &mut buf, DESC);
        assert_eq!(picker(&editor).cursor_row, 0);
    }

    #[test]
    fn picker_filter_typing_clamps_cursor() {
        let mut buf = buffer(SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Film Simulation"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        editor.handle_key(key(KeyCode::Down), &mut buf, DESC);
        editor.handle_key(key(KeyCode::Down), &mut buf, DESC);
        for c in "Velv".chars() {
            editor.handle_key(key(KeyCode::Char(c)), &mut buf, DESC);
        }
        let picker = picker(&editor);
        assert_eq!(picker.filter, "Velv");
        assert_eq!(picker.cursor_row, 0);
    }

    #[test]
    fn picker_enter_commits_when_pick_set() {
        let mut buf = buffer(SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Film Simulation"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        editor.handle_key(key(KeyCode::Down), &mut buf, DESC);
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        assert!(editor.editing().is_none());
        assert_ne!(
            buf.working.canonical.film_simulation,
            Some(FilmSimulation::Provia),
        );
        assert!(buf.dirty());
    }

    #[test]
    fn edit_mode_arrow_consumed_by_text_input_not_dispatch() {
        let mut buf = buffer(seeded_string_base());
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Custom Setting Name"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        editor.handle_key(key(KeyCode::Char('X')), &mut buf, DESC);
        let before = text(&editor).cursor_col;
        editor.handle_key(key(KeyCode::Left), &mut buf, DESC);
        let after = text(&editor).cursor_col;
        assert_eq!(after + 1, before);
        assert!(matches!(
            editor.editing(),
            Some(InlineEdit {
                kind: InlineKind::TextInput(_),
                ..
            })
        ));
    }

    #[test]
    fn slot_load_seeds_shadow_with_mono_defaults_when_film_is_provia() {
        let buf = buffer(SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        assert!(buf.working.shadow.monochromatic_color_temperature.is_some());
        assert_eq!(buf.working.canonical.monochromatic_color_temperature, None);
    }

    #[test]
    fn switching_to_monochrome_reveals_mono_fields_via_settle() {
        let mut buf = buffer(SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            ..Default::default()
        });
        let mut editor = EditorState::default();
        assert_eq!(buf.working.canonical.monochromatic_color_temperature, None);
        assert!(focus(&mut editor, &buf, "Film Simulation"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        editor.commit_pick("acros", &mut buf, DESC);
        assert_eq!(
            buf.working.canonical.film_simulation,
            Some(FilmSimulation::Acros)
        );
        assert!(
            buf.working
                .canonical
                .monochromatic_color_temperature
                .is_some()
        );
    }

    #[test]
    fn bump_mirrors_canonical_into_shadow() {
        let mut buf = buffer(SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            clarity: Some(Clarity::default()),
            ..Default::default()
        });
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Clarity"));
        let shadow_mono_before = buf.working.shadow.monochromatic_color_temperature;
        assert!(shadow_mono_before.is_some());
        editor.handle_key(key(KeyCode::Right), &mut buf, DESC);
        assert_eq!(buf.working.canonical.clarity, buf.working.shadow.clarity);
        assert_eq!(
            buf.working.shadow.monochromatic_color_temperature,
            shadow_mono_before,
        );
    }

    #[test]
    fn unrelated_edit_does_not_aggressively_fill_rule_allowed_fields() {
        let mut buf = buffer(SimulationBase {
            film_simulation: Some(FilmSimulation::Provia),
            clarity: Some(Clarity::default()),
            ..Default::default()
        });
        let mut editor = EditorState::default();
        let before = buf.working.canonical.clone();
        assert!(focus(&mut editor, &buf, "Clarity"));
        editor.handle_key(key(KeyCode::Right), &mut buf, DESC);
        let after = &buf.working.canonical;
        for desc in C_X_S20_SIMULATION.fields {
            if desc.name == "Clarity" {
                continue;
            }
            assert_eq!((desc.display)(after), (desc.display)(&before));
        }
    }

    #[test]
    fn round_trip_film_change_preserves_user_typed_mono_value() {
        let mut buf = buffer(SimulationBase {
            film_simulation: Some(FilmSimulation::Acros),
            monochromatic_color_temperature: Some(MonochromaticColorTemperature::default()),
            ..Default::default()
        });
        let mut editor = EditorState::default();
        assert!(focus(&mut editor, &buf, "Monochromatic Color Temperature"));
        for _ in 0..3 {
            editor.handle_key(key(KeyCode::Right), &mut buf, DESC);
        }
        let user_value = buf.working.canonical.monochromatic_color_temperature;
        assert_ne!(user_value, None);

        assert!(focus(&mut editor, &buf, "Film Simulation"));
        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        editor.commit_pick("provia", &mut buf, DESC);
        assert_eq!(buf.working.canonical.monochromatic_color_temperature, None);
        assert_eq!(
            buf.working.shadow.monochromatic_color_temperature,
            user_value
        );

        editor.handle_key(key(KeyCode::Enter), &mut buf, DESC);
        editor.commit_pick("acros", &mut buf, DESC);
        assert_eq!(
            buf.working.canonical.monochromatic_color_temperature,
            user_value,
        );
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
        EditorState::settle(&mut state, DESC, None);
        assert_eq!(state.canonical, before);
    }
}
