use std::ptr;

use fujicore::{
    features::simulation::SimulationDescriptors,
    generated::{options::OptionCategory, simulations::SimulationBase},
};

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::{
    border_title,
    ui::{
        border_style,
        tabs::{
            AppCtx,
            simulation::{
                DIRTY_MARKER, FILTER_PROMPT, Focused, INDENT, InlineEdit, InlineKind, InlineStatus,
                Pane, PickerState, SimulationState, SimulationTabState, SlotEntry, TextInputState,
            },
        },
    },
};

impl SimulationTabState {
    pub(super) fn render_editor(&self, _ctx: &AppCtx, frame: &mut Frame, area: Rect) {
        let active = self.pane() == Pane::Editor;
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(active));
        let cursor = if active { self.editor_cursor() } else { None };
        let editing = self.editing();

        match self.focused() {
            None => {
                render_centered_message(frame, area, block, "(no entry selected)", Color::DarkGray);
            }
            Some(Focused::Slot {
                slot,
                entry,
                descriptors,
            }) => {
                let title = slot.to_string();
                render_slot(
                    frame,
                    area,
                    block,
                    &title,
                    entry,
                    descriptors,
                    cursor,
                    editing,
                );
            }
            Some(Focused::Library { lib, descriptors }) => {
                render_library(
                    frame,
                    area,
                    block,
                    &lib.entry.name,
                    &lib.buffer.working,
                    &lib.buffer.fetched.canonical,
                    descriptors,
                    lib.buffer.dirty(),
                    cursor,
                    editing,
                );
            }
        }
    }
}

fn render_slot(
    frame: &mut Frame,
    area: Rect,
    block: Block<'_>,
    title: &str,
    entry: &SlotEntry,
    descriptors: &'static SimulationDescriptors,
    cursor: Option<usize>,
    editing: Option<&InlineEdit>,
) {
    match entry {
        SlotEntry::Loading => {
            render_centered_message(
                frame,
                area,
                block.title(border_title!(1, "{title}")),
                "loading...",
                Color::DarkGray,
            );
        }
        SlotEntry::Failed(err) => {
            render_centered_message(
                frame,
                area,
                block.title(border_title!(1, "{title}")),
                &format!("fetch failed: {err}"),
                Color::Red,
            );
        }
        SlotEntry::Loaded(buf) => {
            render_library(
                frame,
                area,
                block,
                title,
                &buf.working,
                &buf.fetched.canonical,
                descriptors,
                buf.dirty(),
                cursor,
                editing,
            );
        }
    }
}

fn render_library(
    frame: &mut Frame,
    area: Rect,
    block: Block<'_>,
    title: &str,
    state: &SimulationState,
    fetched: &SimulationBase,
    descriptors: &'static SimulationDescriptors,
    dirty: bool,
    cursor: Option<usize>,
    editing: Option<&InlineEdit>,
) {
    let title = if dirty {
        let text = border_title!(1, "{DIRTY_MARKER} {title}");
        Line::from(Span::styled(
            text,
            Style::default().add_modifier(Modifier::ITALIC),
        ))
    } else {
        Line::from(Span::raw(border_title!(1, "{title}")))
    };
    let inner_width = area.width.saturating_sub(2);
    let items = field_items(
        descriptors,
        &state.canonical,
        fetched,
        cursor,
        inner_width,
        editing,
    );
    let list = List::new(items).block(block.title(title));
    frame.render_widget(list, area);
}

fn field_items(
    descriptors: &SimulationDescriptors,
    canonical: &SimulationBase,
    fetched: &SimulationBase,
    cursor: Option<usize>,
    inner_width: u16,
    editing: Option<&InlineEdit>,
) -> Vec<ListItem<'static>> {
    let visible = descriptors.visible_fields(canonical);
    let mut items: Vec<ListItem<'static>> = Vec::new();
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
            let value = (field.display)(canonical).unwrap_or_default();
            match &edit.kind {
                InlineKind::TextInput(text) => append_text_input(
                    &mut items,
                    prefix,
                    field.name,
                    text,
                    edit.status,
                    inner_width,
                ),
                InlineKind::Picker(picker) => append_picker(
                    &mut items,
                    prefix,
                    field.name,
                    &value,
                    picker,
                    edit.status,
                    inner_width,
                ),
            }
        } else {
            let value = (field.display)(canonical).expect("visible field has display");
            let dirty = !(field.eq)(canonical, fetched);
            items.push(field_item(
                prefix,
                field.name,
                value,
                cursor == Some(field_idx),
                dirty,
                inner_width,
            ));
        }
    }

    items
}

fn field_item(
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

fn append_text_input(
    out: &mut Vec<ListItem<'static>>,
    prefix: &'static str,
    name: &'static str,
    text: &TextInputState,
    status: InlineStatus,
    inner_width: u16,
) {
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
    spans.extend(buffer_with_cursor(&text.buffer, text.cursor_col, status));
    out.push(ListItem::new(Line::from(spans)));
}

fn append_picker(
    out: &mut Vec<ListItem<'static>>,
    prefix: &'static str,
    name: &'static str,
    canonical_value: &str,
    picker: &PickerState,
    status: InlineStatus,
    inner_width: u16,
) {
    let label_w = prefix.chars().count() + name.chars().count();
    let value_w = canonical_value.chars().count();
    let gap = (inner_width as usize).saturating_sub(label_w + value_w);
    let dots_w = gap.saturating_sub(2);
    let dots: String = (0..dots_w)
        .map(|i| if i % 2 == 0 { '.' } else { ' ' })
        .collect();
    let dots_style = Style::default().fg(Color::DarkGray);

    let value_style = field_text_style(status);
    out.push(ListItem::new(Line::from(vec![
        Span::raw(prefix),
        Span::raw(name),
        Span::raw(" "),
        Span::styled(dots, dots_style),
        Span::raw(" "),
        Span::styled(canonical_value.to_owned(), value_style),
    ])));
    let inner_prefix = format!("{prefix}{INDENT}");
    let mut filter_spans = vec![
        Span::raw(inner_prefix.clone()),
        Span::styled(FILTER_PROMPT, Style::default().fg(Color::DarkGray)),
    ];
    filter_spans.extend(buffer_with_cursor(
        &picker.filter,
        picker.filter.chars().count(),
        InlineStatus::Idle,
    ));
    out.push(ListItem::new(Line::from(filter_spans)));
    let visible = picker.visible_rows();
    if visible.is_empty() {
        out.push(ListItem::new(Line::from(Span::styled(
            format!("{inner_prefix}(no matches)"),
            Style::default().fg(Color::DarkGray),
        ))));
    } else {
        for (i, row) in visible.iter().enumerate() {
            let style = if i == picker.cursor_row {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            out.push(ListItem::new(Line::from(vec![
                Span::raw(inner_prefix.clone()),
                Span::styled(row.label.to_owned(), style),
            ])));
        }
    }
}

fn field_text_style(status: InlineStatus) -> Style {
    match status {
        InlineStatus::Idle => Style::default(),
        InlineStatus::Rejected => Style::default().fg(Color::Red),
    }
}

fn buffer_with_cursor(buffer: &str, cursor_col: usize, status: InlineStatus) -> Vec<Span<'static>> {
    let chars: Vec<char> = buffer.chars().collect();
    let before: String = chars.iter().take(cursor_col).collect();
    let at: String = chars
        .get(cursor_col)
        .map_or_else(|| " ".to_owned(), ToString::to_string);
    let after: String = chars.iter().skip(cursor_col + 1).collect();
    let base = field_text_style(status);
    let cursor_style = base.add_modifier(Modifier::REVERSED);
    vec![
        Span::styled(before, base),
        Span::styled(at, cursor_style),
        Span::styled(after, base),
    ]
}

fn render_centered_message(
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
