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
                DIRTY_MARKER, Focused, INDENT, Pane, SimulationState, SimulationTabState, SlotEntry,
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

        match self.focused() {
            None => {
                centered_message(frame, area, block, "(no entry selected)", Color::DarkGray);
            }
            Some(Focused::Slot {
                slot,
                entry,
                descriptors,
            }) => {
                let title = slot.to_string();
                render_slot(frame, area, block, &title, entry, descriptors, cursor);
            }
            Some(Focused::Library { lib, descriptors }) => {
                render_library(
                    frame,
                    area,
                    block,
                    &lib.entry.name,
                    &lib.buffer.working,
                    descriptors,
                    lib.buffer.dirty(),
                    cursor,
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
) {
    match entry {
        SlotEntry::Loading => {
            centered_message(
                frame,
                area,
                block.title(border_title!(1, "{title}")),
                "loading...",
                Color::DarkGray,
            );
        }
        SlotEntry::Failed(err) => {
            centered_message(
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
                descriptors,
                buf.dirty(),
                cursor,
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
    descriptors: &'static SimulationDescriptors,
    dirty: bool,
    cursor: Option<usize>,
) {
    let title = if dirty {
        border_title!(1, "{DIRTY_MARKER} {title}")
    } else {
        border_title!(1, "{title}")
    };
    let inner_width = area.width.saturating_sub(2);
    let items = build_field_items(descriptors, &state.canonical, cursor, inner_width);
    let list = List::new(items).block(block.title(title));
    frame.render_widget(list, area);
}

fn build_field_items(
    descriptors: &SimulationDescriptors,
    canonical: &SimulationBase,
    cursor: Option<usize>,
    inner_width: u16,
) -> Vec<ListItem<'static>> {
    let mut items: Vec<ListItem<'static>> = Vec::new();
    let mut field_idx: usize = 0;
    let mut first_group = true;

    let mut order: Vec<Option<OptionCategory>> = Vec::new();
    for field in descriptors.fields {
        if !order.contains(&field.category) {
            order.push(field.category);
        }
    }

    for category in order {
        let prefix = if category.is_some() { INDENT } else { "" };
        let mut group_items: Vec<ListItem<'static>> = Vec::new();
        for field in descriptors.fields {
            if field.category != category {
                continue;
            }
            let Some(value) = (field.display)(canonical) else {
                continue;
            };
            group_items.push(field_item(
                prefix,
                field.name,
                value,
                cursor == Some(field_idx),
                inner_width,
            ));
            field_idx += 1;
        }
        if group_items.is_empty() {
            continue;
        }
        if !first_group {
            items.push(ListItem::new(""));
        }
        if let Some(c) = category {
            items.push(ListItem::new(Line::from(Span::styled(
                c.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ))));
        }
        items.extend(group_items);
        first_group = false;
    }

    items
}

fn field_item(
    prefix: &'static str,
    name: &'static str,
    value: String,
    highlight: bool,
    inner_width: u16,
) -> ListItem<'static> {
    let label_w = prefix.chars().count() + name.chars().count();
    let value_w = value.chars().count();
    let gap = (inner_width as usize).saturating_sub(label_w + value_w);
    let dots_w = gap.saturating_sub(2);
    let dots: String = (0..dots_w)
        .map(|i| if i % 2 == 0 { '.' } else { ' ' })
        .collect();
    let text_style = if highlight {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    let dots_style = Style::default().fg(Color::DarkGray);
    ListItem::new(Line::from(vec![
        Span::raw(prefix),
        Span::styled(name, text_style),
        Span::raw(" "),
        Span::styled(dots, dots_style),
        Span::raw(" "),
        Span::styled(value, text_style),
    ]))
}

fn centered_message(frame: &mut Frame, area: Rect, block: Block<'_>, text: &str, color: Color) {
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
