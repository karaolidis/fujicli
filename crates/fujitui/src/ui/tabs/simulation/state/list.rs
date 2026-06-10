use fujicore::generated::options::CustomSetting;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::ListItem,
};

use crate::{
    ui::{
        danger, muted, warning,
        widgets::{Cursor, ListPane},
    },
    workers::fs::slug::Slug,
};

use super::{
    DIRTY_MARKER, INDENT, RenameState, SimulationCursor,
    library::{SimulationLibraryBuffer, SimulationLibraryView},
    slots::{SlotEntry, Slots},
};

const COL_SEPARATOR: &str = " ";

pub(super) type SimulationListPane = ListPane<SimulationCursor>;

impl Cursor for SimulationCursor {
    fn none() -> Self {
        Self::None
    }

    fn rehome(&self, order: &[Self]) -> Self {
        let first_or_none = || order.first().cloned().unwrap_or(Self::None);
        match self {
            Self::Library(lost) => order
                .iter()
                .find(|c| matches!(c, Self::Library(s) if s >= lost))
                .or_else(|| {
                    order
                        .iter()
                        .rev()
                        .find(|c| matches!(c, Self::Library(s) if s < lost))
                })
                .cloned()
                .unwrap_or_else(first_or_none),
            Self::Slot(_) => order
                .iter()
                .find(|c| matches!(c, Self::Slot(_)))
                .cloned()
                .unwrap_or_else(first_or_none),
            Self::None => Self::None,
        }
    }
}

impl SimulationListPane {
    pub(super) fn simulation_slot_entries<'a>(
        &self,
        slots: &'a Slots,
    ) -> impl Iterator<Item = (CustomSetting, &'a SlotEntry)> + 'a {
        let needle = self.filter().needle_lower();
        slots.into_iter().filter(move |(_, entry)| {
            needle.is_empty()
                || entry
                    .name()
                    .is_some_and(|n| n.to_lowercase().contains(&needle))
        })
    }

    pub(super) fn library_entries<'a>(
        &self,
        library: &'a SimulationLibraryView,
    ) -> impl Iterator<Item = (&'a Slug, &'a SimulationLibraryBuffer)> + 'a {
        let needle = self.filter().needle_lower();
        library.into_iter().filter(move |(_, lib)| {
            needle.is_empty() || lib.entry.name.to_lowercase().contains(&needle)
        })
    }

    pub(super) fn order(
        &self,
        slots: &Slots,
        library: &SimulationLibraryView,
    ) -> Vec<SimulationCursor> {
        self.simulation_slot_entries(slots)
            .map(|(slot, _)| SimulationCursor::Slot(slot))
            .chain(
                self.library_entries(library)
                    .map(|(slug, _)| SimulationCursor::Library(slug.clone())),
            )
            .collect()
    }

    pub(super) fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        active: bool,
        slots: &Slots,
        library: &SimulationLibraryView,
        rename: Option<&RenameState>,
    ) {
        let (items, selected) = self.make_list_items(slots, library, rename);
        self.render(frame, area, active, "Simulations", items, selected);
    }

    fn make_list_items(
        &self,
        slots: &Slots,
        library: &SimulationLibraryView,
        rename: Option<&RenameState>,
    ) -> (Vec<ListItem<'static>>, Option<usize>) {
        let filtering = !self.filter().buffer().is_empty();
        let cursor = self.selection();
        let mut out = Vec::new();
        let mut selected = None;

        let slot_count = self.simulation_slot_entries(slots).count();
        out.push(Self::make_section_header(&format!("Slots ({slot_count})")));
        if slot_count == 0 {
            out.push(Self::make_placeholder(if filtering {
                "(no matches)"
            } else {
                "(no slots)"
            }));
        } else {
            for (slot, entry) in self.simulation_slot_entries(slots) {
                if matches!(cursor, SimulationCursor::Slot(s) if *s == slot) {
                    selected = Some(out.len());
                }
                out.push(Self::make_simulation_slot_item(slot, entry, cursor));
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
                out.push(Self::make_library_item(slug, lib, cursor, rename));
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
            Style::default().fg(muted()),
        )))
    }

    fn dirty_marker_span(selected: bool) -> Span<'static> {
        let mut style = Style::default().fg(warning());
        if selected {
            style = style.add_modifier(Modifier::REVERSED);
        }
        Span::styled(format!("{DIRTY_MARKER} "), style)
    }

    fn make_simulation_slot_item(
        slot: CustomSetting,
        entry: &SlotEntry,
        cursor: &SimulationCursor,
    ) -> ListItem<'static> {
        let selected = matches!(cursor, SimulationCursor::Slot(s) if *s == slot);
        let (label, style, dirty) = match entry {
            SlotEntry::Loading => (
                format!("{slot}{COL_SEPARATOR}(loading...)"),
                Style::default().fg(muted()),
                false,
            ),
            SlotEntry::Failed(_) => (
                format!("{slot}{COL_SEPARATOR}(failed)"),
                Style::default().fg(danger()),
                false,
            ),
            SlotEntry::Loaded(buf) => {
                let name = entry
                    .name()
                    .map_or_else(|| "(unnamed)".to_owned(), ToString::to_string);
                (
                    format!("{slot}{COL_SEPARATOR}{name}"),
                    Style::default(),
                    buf.dirty(),
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
        let mut spans = vec![Span::raw(INDENT)];
        if dirty {
            spans.push(Self::dirty_marker_span(selected));
        }
        spans.push(Span::styled(label, text_style));
        ListItem::new(Line::from(spans))
    }

    fn make_library_item(
        slug: &Slug,
        lib: &SimulationLibraryBuffer,
        cursor: &SimulationCursor,
        rename: Option<&RenameState>,
    ) -> ListItem<'static> {
        if let Some(rename) = rename.filter(|r| &r.slug == slug) {
            let mut spans = vec![Span::raw(INDENT)];
            spans.extend(rename.text.cursor_spans(Style::default()));
            return ListItem::new(Line::from(spans));
        }
        let selected = matches!(cursor, SimulationCursor::Library(s) if s == slug);
        let dirty = lib.buffer.dirty();
        let label = lib.entry.name.clone();
        let mut text_style = Style::default();
        if selected {
            text_style = text_style.add_modifier(Modifier::REVERSED);
        }
        if dirty {
            text_style = text_style.add_modifier(Modifier::ITALIC);
        }
        let mut spans = vec![Span::raw(INDENT)];
        if dirty {
            spans.push(Self::dirty_marker_span(selected));
        }
        spans.push(Span::styled(label, text_style));
        ListItem::new(Line::from(spans))
    }
}
