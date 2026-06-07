use fujicore::generated::options::CustomSetting;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::{
    border_title,
    ui::{
        border_style,
        tabs::simulation::{
            COL_SEPARATOR, DIRTY_MARKER, FILTER_PROMPT, INDENT, LibraryBuffer, Pane,
            SimulationCursor, SimulationTabState, SlotEntry, TextInputState,
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
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let show_chip = self.filtering() || !self.filter().buffer.is_empty();
        let list_area = if show_chip {
            let [chip_area, list_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);
            render_filter(frame, chip_area, self.filter(), self.filtering());
            list_area
        } else {
            inner
        };

        let items = list_items(self);
        frame.render_widget(List::new(items), list_area);
    }
}

fn render_filter(frame: &mut Frame, area: Rect, filter: &TextInputState, filtering: bool) {
    let prompt = Span::styled(FILTER_PROMPT, Style::default().fg(Color::DarkGray));
    let line = if filtering {
        let chars: Vec<char> = filter.buffer.chars().collect();
        let cursor = filter.cursor_col;
        let before: String = chars.iter().take(cursor).collect();
        let at: String = chars
            .get(cursor)
            .map_or_else(|| " ".to_owned(), ToString::to_string);
        let after: String = chars.iter().skip(cursor + 1).collect();
        Line::from(vec![
            prompt,
            Span::raw(before),
            Span::styled(at, Style::default().add_modifier(Modifier::REVERSED)),
            Span::raw(after),
        ])
    } else {
        Line::from(vec![
            prompt,
            Span::styled(filter.buffer.clone(), Style::default().fg(Color::DarkGray)),
        ])
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn list_items(state: &SimulationTabState) -> Vec<ListItem<'static>> {
    let filtering = !state.filter().buffer.is_empty();
    let mut out = Vec::new();
    let slot_count = state.slot_entries().count();
    out.push(section_header(&format!("Slots ({slot_count})")));
    if slot_count == 0 {
        out.push(placeholder(if filtering {
            "(no matches)"
        } else {
            "(no slots)"
        }));
    } else {
        for (slot, entry) in state.slot_entries() {
            out.push(slot_item(slot, entry, state.list_cursor()));
        }
    }
    let lib_count = state.library_entries().count();
    out.push(section_header(&format!("Library ({lib_count})")));
    if lib_count == 0 {
        out.push(placeholder(if filtering {
            "(no matches)"
        } else {
            "(no entries)"
        }));
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
    let (label, style, dirty) = match entry {
        SlotEntry::Loading => (
            format!("{slot}{COL_SEPARATOR}(loading...)"),
            Style::default().fg(Color::DarkGray),
            false,
        ),
        SlotEntry::Failed(_) => (
            format!("{slot}{COL_SEPARATOR}(failed)"),
            Style::default().fg(Color::Red),
            false,
        ),
        SlotEntry::Loaded(buf) => {
            let name = entry
                .name()
                .map_or_else(|| "(unnamed)".to_owned(), ToString::to_string);
            let dirty = buf.dirty();
            let marker = if dirty {
                format!("{DIRTY_MARKER} ")
            } else {
                String::new()
            };
            (
                format!("{marker}{slot}{COL_SEPARATOR}{name}"),
                Style::default(),
                dirty,
            )
        }
    };
    let mut text_style = style;
    if selected {
        text_style = text_style.add_modifier(Modifier::REVERSED);
    }
    if dirty {
        text_style = text_style.add_modifier(Modifier::ITALIC);
    }
    ListItem::new(Line::from(vec![
        Span::raw(INDENT),
        Span::styled(label, text_style),
    ]))
}

fn library_item(slug: &Slug, lib: &LibraryBuffer, cursor: &SimulationCursor) -> ListItem<'static> {
    let selected = matches!(cursor, SimulationCursor::Library(s) if s == slug);
    let dirty = lib.buffer.dirty();
    let marker = if dirty {
        format!("{DIRTY_MARKER} ")
    } else {
        String::new()
    };
    let label = format!("{marker}{}", lib.entry.name);
    let mut text_style = Style::default();
    if selected {
        text_style = text_style.add_modifier(Modifier::REVERSED);
    }
    if dirty {
        text_style = text_style.add_modifier(Modifier::ITALIC);
    }
    ListItem::new(Line::from(vec![
        Span::raw(INDENT),
        Span::styled(label, text_style),
    ]))
}
