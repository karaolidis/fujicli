use crossterm::event::{KeyCode, KeyEvent};
use fujicore::generated::options::CustomSetting;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::{border_title, ui::border_style, workers::fs::library::Slug};

use super::{
    CursorMove, DIRTY_MARKER, FILTER_PROMPT, INDENT, SimulationCursor, TextInputState,
    draw_scrollbar,
    library::{Library, LibraryBuffer},
    make_buffer_with_cursor,
    slots::{SlotEntry, Slots},
};

const COL_SEPARATOR: &str = " ";

#[derive(Debug, Default)]
pub struct ListPane {
    selection: SimulationCursor,
    filter: TextInputState,
    filtering: bool,
    scroll: usize,
}

impl ListPane {
    pub(super) const fn selection(&self) -> &SimulationCursor {
        &self.selection
    }

    pub(super) const fn filtering(&self) -> bool {
        self.filtering
    }

    #[cfg(test)]
    pub(super) const fn filter(&self) -> &TextInputState {
        &self.filter
    }

    pub(super) fn slot_entries<'a>(
        &'a self,
        slots: &'a Slots,
    ) -> impl Iterator<Item = (CustomSetting, &'a SlotEntry)> + 'a {
        let needle = self.filter.buffer.to_lowercase();
        slots.into_iter().filter(move |(_, entry)| {
            needle.is_empty()
                || entry
                    .name()
                    .is_some_and(|n| n.to_lowercase().contains(&needle))
        })
    }

    pub(super) fn library_entries<'a>(
        &'a self,
        library: &'a Library,
    ) -> impl Iterator<Item = (&'a Slug, &'a LibraryBuffer)> + 'a {
        let needle = self.filter.buffer.to_lowercase();
        library.into_iter().filter(move |(_, lib)| {
            needle.is_empty() || lib.entry.name.to_lowercase().contains(&needle)
        })
    }

    pub(super) fn order(&self, slots: &Slots, library: &Library) -> Vec<SimulationCursor> {
        self.slot_entries(slots)
            .map(|(slot, _)| SimulationCursor::Slot(slot))
            .chain(
                self.library_entries(library)
                    .map(|(slug, _)| SimulationCursor::Library(slug.clone())),
            )
            .collect()
    }

    pub(super) fn step(&mut self, dir: CursorMove, order: &[SimulationCursor]) {
        if order.is_empty() {
            self.selection = SimulationCursor::None;
            return;
        }
        let current = order.iter().position(|c| c == &self.selection);
        let target = match (current, dir) {
            (None, _) => 0,
            (Some(i), CursorMove::Up) => i.saturating_sub(1),
            (Some(i), CursorMove::Down) => (i + 1).min(order.len() - 1),
        };
        self.selection = order[target].clone();
    }

    pub(super) fn reset(&mut self, order: &[SimulationCursor]) {
        self.selection = order.first().cloned().unwrap_or(SimulationCursor::None);
    }

    pub(super) fn settle_selection(&mut self, order: &[SimulationCursor]) {
        if matches!(self.selection, SimulationCursor::None) {
            self.reset(order);
        } else {
            self.ensure_valid(order);
        }
    }

    pub(super) fn ensure_valid(&mut self, order: &[SimulationCursor]) {
        if matches!(self.selection, SimulationCursor::None) || order.contains(&self.selection) {
            return;
        }
        let first_or_none = || order.first().cloned().unwrap_or(SimulationCursor::None);
        self.selection = match &self.selection {
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

    pub(super) fn start_filter(&mut self) {
        self.filtering = true;
        self.filter.cursor_col = self.filter.buffer.chars().count();
    }

    pub(super) fn handle_filter_key(&mut self, key: KeyEvent) -> bool {
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
                } else {
                    order_dirty = filter.delete_before();
                }
            }
            KeyCode::Delete => order_dirty = filter.delete_after(),
            KeyCode::Left => filter.move_left(),
            KeyCode::Right => filter.move_right(),
            KeyCode::Home => filter.move_home(),
            KeyCode::End => filter.move_end(),
            KeyCode::Char(c) if !c.is_control() => {
                order_dirty = filter.insert(c, usize::MAX);
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
        order_dirty
    }

    pub(super) fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        active: bool,
        slots: &Slots,
        library: &Library,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(active))
            .title(border_title!(1, "Simulations"));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let show_chip = self.filtering || !self.filter.buffer.is_empty();
        let list_area = if show_chip {
            let [chip_area, list_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);
            self.draw_filter(frame, chip_area);
            list_area
        } else {
            inner
        };

        let (items, selected) = self.make_list_items(slots, library);
        let content_len = items.len();
        let mut list_state = ListState::default().with_offset(self.scroll);
        list_state.select(selected);
        frame.render_stateful_widget(List::new(items), list_area, &mut list_state);
        self.scroll = list_state.offset();
        draw_scrollbar(
            frame,
            Rect {
                x: area.x,
                y: list_area.y,
                width: area.width,
                height: list_area.height,
            },
            content_len,
            self.scroll,
        );
    }

    fn draw_filter(&self, frame: &mut Frame, area: Rect) {
        let prompt = Span::styled(FILTER_PROMPT, Style::default().fg(Color::DarkGray));
        let line = if self.filtering {
            let mut spans = vec![prompt];
            spans.extend(make_buffer_with_cursor(
                &self.filter.buffer,
                self.filter.cursor_col,
                Style::default(),
            ));
            Line::from(spans)
        } else {
            Line::from(vec![
                prompt,
                Span::styled(
                    self.filter.buffer.clone(),
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        };
        frame.render_widget(Paragraph::new(line), area);
    }

    fn make_list_items(
        &self,
        slots: &Slots,
        library: &Library,
    ) -> (Vec<ListItem<'static>>, Option<usize>) {
        let filtering = !self.filter.buffer.is_empty();
        let cursor = &self.selection;
        let mut out = Vec::new();
        let mut selected = None;

        let slot_count = self.slot_entries(slots).count();
        out.push(Self::make_section_header(&format!("Slots ({slot_count})")));
        if slot_count == 0 {
            out.push(Self::make_placeholder(if filtering {
                "(no matches)"
            } else {
                "(no slots)"
            }));
        } else {
            for (slot, entry) in self.slot_entries(slots) {
                if matches!(cursor, SimulationCursor::Slot(s) if *s == slot) {
                    selected = Some(out.len());
                }
                out.push(Self::make_slot_item(slot, entry, cursor));
            }
        }

        let lib_count = self.library_entries(library).count();
        out.push(Self::make_section_header(&format!("Library ({lib_count})")));
        if lib_count == 0 {
            out.push(Self::make_placeholder(if filtering {
                "(no matches)"
            } else {
                "(no entries)"
            }));
        } else {
            for (slug, lib) in self.library_entries(library) {
                if matches!(cursor, SimulationCursor::Library(s) if s == slug) {
                    selected = Some(out.len());
                }
                out.push(Self::make_library_item(slug, lib, cursor));
            }
        }

        (out, selected)
    }

    fn make_section_header(label: &str) -> ListItem<'static> {
        ListItem::new(Line::from(Span::styled(
            label.to_owned(),
            Style::default().add_modifier(Modifier::BOLD),
        )))
    }

    fn make_placeholder(label: &str) -> ListItem<'static> {
        ListItem::new(Line::from(Span::styled(
            format!("{INDENT}{label}"),
            Style::default().fg(Color::DarkGray),
        )))
    }

    fn make_slot_item(
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

    fn make_library_item(
        slug: &Slug,
        lib: &LibraryBuffer,
        cursor: &SimulationCursor,
    ) -> ListItem<'static> {
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
}
