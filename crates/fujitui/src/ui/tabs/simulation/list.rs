use fujicore::generated::options::CustomSetting;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::{
    border_title,
    ui::{
        border_style,
        tabs::simulation::{
            COL_SEPARATOR, DIRTY_MARKER, INDENT, LibraryBuffer, Pane, SimulationCursor,
            SimulationTabState, SlotEntry,
        },
    },
    workers::fs::library::Slug,
};

impl SimulationTabState {
    pub(super) fn render_list(&self, frame: &mut Frame, area: Rect) {
        let active = self.pane() == Pane::List;
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(active))
            .title(border_title!(1, "Simulations"));
        let items = build_list_items(self);
        frame.render_widget(List::new(items).block(block), area);
    }
}

fn build_list_items(state: &SimulationTabState) -> Vec<ListItem<'static>> {
    let mut out = Vec::new();
    let slot_count = state.slot_entries().count();
    out.push(section_header(&format!("Slots ({slot_count})")));
    if slot_count == 0 {
        out.push(placeholder("(no slots)"));
    } else {
        for (slot, entry) in state.slot_entries() {
            out.push(slot_item(slot, entry, state.list_cursor()));
        }
    }
    let lib_count = state.library_entries().count();
    out.push(section_header(&format!("Library ({lib_count})")));
    if lib_count == 0 {
        out.push(placeholder("(no entries)"));
    } else {
        for (slug, lib) in state.library_entries() {
            out.push(library_item(slug, lib, state.list_cursor()));
        }
    }
    out
}

fn section_header(label: &str) -> ListItem<'static> {
    ListItem::new(Line::from(Span::styled(
        label.to_owned(),
        Style::default().add_modifier(Modifier::BOLD),
    )))
}

fn placeholder(label: &str) -> ListItem<'static> {
    ListItem::new(Line::from(Span::styled(
        format!("{INDENT}{label}"),
        Style::default().fg(Color::DarkGray),
    )))
}

fn slot_item(
    slot: CustomSetting,
    entry: &SlotEntry,
    cursor: &SimulationCursor,
) -> ListItem<'static> {
    let selected = matches!(cursor, SimulationCursor::Slot(s) if *s == slot);
    let (label, style) = match entry {
        SlotEntry::Loading => (
            format!("{slot}{COL_SEPARATOR}(loading...)"),
            Style::default().fg(Color::DarkGray),
        ),
        SlotEntry::Failed(_) => (
            format!("{slot}{COL_SEPARATOR}(failed)"),
            Style::default().fg(Color::Red),
        ),
        SlotEntry::Loaded(buf) => {
            let name = buf
                .working
                .canonical
                .custom_setting_name
                .as_ref()
                .map_or_else(|| "(unnamed)".to_owned(), ToString::to_string);
            let marker = if buf.dirty() {
                format!("{DIRTY_MARKER} ")
            } else {
                String::new()
            };
            (
                format!("{marker}{slot}{COL_SEPARATOR}{name}"),
                Style::default(),
            )
        }
    };
    let text_style = if selected {
        style.add_modifier(Modifier::REVERSED)
    } else {
        style
    };
    ListItem::new(Line::from(vec![
        Span::raw(INDENT),
        Span::styled(label, text_style),
    ]))
}

fn library_item(slug: &Slug, lib: &LibraryBuffer, cursor: &SimulationCursor) -> ListItem<'static> {
    let selected = matches!(cursor, SimulationCursor::Library(s) if s == slug);
    let marker = if lib.buffer.dirty() {
        format!("{DIRTY_MARKER} ")
    } else {
        String::new()
    };
    let label = format!("{marker}{}", lib.entry.name);
    let text_style = if selected {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    ListItem::new(Line::from(vec![
        Span::raw(INDENT),
        Span::styled(label, text_style),
    ]))
}
