use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};
use time::format_description::well_known::Rfc3339;

use crate::{
    border_title,
    ui::{
        border_style,
        tabs::AppCtx,
        widgets::{FilterOutcome, FilterState, Scrollbar},
    },
    workers::fs::{backup::BackupLibraryEntry, slug::Slug},
};

use super::{BackupCursor, COL_SEPARATOR, CursorMove, INDENT, RenameState};

#[derive(Debug, Default)]
pub(super) struct ListPane {
    selection: BackupCursor,
    filter: FilterState,
    scroll: usize,
}

impl ListPane {
    pub(super) const fn selection(&self) -> &BackupCursor {
        &self.selection
    }

    pub(super) fn set_selection(&mut self, selection: BackupCursor) {
        self.selection = selection;
    }

    pub(super) const fn filtering(&self) -> bool {
        self.filter.active()
    }

    pub(super) fn entries<'a>(
        &'a self,
        ctx: &'a AppCtx,
    ) -> impl Iterator<Item = (&'a Slug, &'a BackupLibraryEntry)> + 'a {
        let connected = ctx.device_snapshot.as_ref().map(|s| s.usb_id);
        let needle = self.filter.needle_lower();
        ctx.backup_library_snapshot
            .entries
            .iter()
            .filter(move |(_, entry)| connected.is_some_and(|usb| entry.source_camera == usb))
            .filter(move |(_, entry)| {
                needle.is_empty() || entry.name.to_lowercase().contains(&needle)
            })
    }

    pub(super) fn order(&self, ctx: &AppCtx) -> Vec<BackupCursor> {
        self.entries(ctx)
            .map(|(slug, _)| BackupCursor::Entry(slug.clone()))
            .collect()
    }

    pub(super) fn step(&mut self, dir: CursorMove, order: &[BackupCursor]) {
        if order.is_empty() {
            self.selection = BackupCursor::None;
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

    pub(super) fn ensure_valid(&mut self, order: &[BackupCursor]) {
        if order.contains(&self.selection) {
            return;
        }
        let first_or_none = || order.first().cloned().unwrap_or(BackupCursor::None);
        self.selection = match &self.selection {
            BackupCursor::None => first_or_none(),
            BackupCursor::Entry(lost) => order
                .iter()
                .find(|c| matches!(c, BackupCursor::Entry(s) if s >= lost))
                .or_else(|| {
                    order
                        .iter()
                        .rev()
                        .find(|c| matches!(c, BackupCursor::Entry(s) if s < lost))
                })
                .cloned()
                .unwrap_or_else(first_or_none),
        };
    }

    pub(super) fn start_filter(&mut self) {
        self.filter.start();
    }

    pub(super) fn handle_filter_key(&mut self, key: KeyEvent) -> bool {
        matches!(
            self.filter.handle_key(key),
            FilterOutcome::ContentChanged | FilterOutcome::Closed,
        )
    }

    pub(super) fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        ctx: &AppCtx,
        rename: Option<&RenameState>,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(true))
            .title(border_title!(1, "Backups"));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let list_area = if self.filter.show_chip() {
            let [chip_area, list_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);
            self.filter.draw(frame, chip_area);
            list_area
        } else {
            inner
        };

        let (items, selected) = self.make_list_items(ctx, rename);
        let content_len = items.len();
        let mut list_state = ListState::default().with_offset(self.scroll);
        list_state.select(selected);
        frame.render_stateful_widget(List::new(items), list_area, &mut list_state);
        self.scroll = list_state.offset();
        Scrollbar::draw(
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

    fn make_list_items(
        &self,
        ctx: &AppCtx,
        rename: Option<&RenameState>,
    ) -> (Vec<ListItem<'static>>, Option<usize>) {
        let mut out = Vec::new();
        let mut selected = None;
        let filtering = !self.filter.buffer().is_empty();
        let connected = ctx.device_snapshot.as_ref().map(|s| s.usb_id);

        let visible: Vec<(&Slug, &BackupLibraryEntry)> = self.entries(ctx).collect();

        if connected.is_none() {
            out.push(Self::make_section_header("Backups (—)"));
            out.push(Self::make_placeholder("(no camera connected)"));
        } else if visible.is_empty() {
            out.push(Self::make_section_header("Backups (0)"));
            out.push(Self::make_placeholder(if filtering {
                "(no matches)"
            } else {
                "(no backups for this camera)"
            }));
        } else {
            out.push(Self::make_section_header(&format!(
                "Backups ({})",
                visible.len()
            )));
            for (slug, entry) in visible {
                let is_selected = matches!(&self.selection, BackupCursor::Entry(s) if s == slug);
                if is_selected {
                    selected = Some(out.len());
                }
                let inline = rename.filter(|r| r.slug == *slug);
                out.push(Self::make_backup_item(entry, is_selected, inline));
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

    fn make_backup_item(
        entry: &BackupLibraryEntry,
        selected: bool,
        rename: Option<&RenameState>,
    ) -> ListItem<'static> {
        let when = entry
            .created
            .format(&Rfc3339)
            .expect("Rfc3339 format should always succeed")
            .chars()
            .take(19)
            .collect::<String>();
        let suffix = format!("{COL_SEPARATOR}({when})");
        if let Some(rename) = rename {
            let mut spans = vec![Span::raw(INDENT)];
            spans.extend(rename.text.cursor_spans(Style::default()));
            spans.push(Span::styled(suffix, Style::default().fg(Color::DarkGray)));
            ListItem::new(Line::from(spans))
        } else {
            let base_style = if selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            let label = format!("{}{suffix}", entry.name);
            ListItem::new(Line::from(vec![
                Span::raw(INDENT),
                Span::styled(label, base_style),
            ]))
        }
    }
}
