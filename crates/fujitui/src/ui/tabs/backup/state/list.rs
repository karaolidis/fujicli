use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::ListItem,
};
use time::format_description::well_known::Rfc3339;

use crate::{
    ui::{
        muted,
        tabs::AppCtx,
        widgets::{Cursor, ListPane},
    },
    workers::fs::{backup::BackupLibraryEntry, slug::Slug},
};

use super::{BackupCursor, COL_SEPARATOR, INDENT, RenameState};

pub(super) type BackupListPane = ListPane<BackupCursor>;

impl Cursor for BackupCursor {
    fn none() -> Self {
        Self::None
    }

    fn rehome(&self, order: &[Self]) -> Self {
        let first_or_none = || order.first().cloned().unwrap_or(Self::None);
        match self {
            Self::None => first_or_none(),
            Self::Entry(lost) => order
                .iter()
                .find(|c| matches!(c, Self::Entry(s) if s >= lost))
                .or_else(|| {
                    order
                        .iter()
                        .rev()
                        .find(|c| matches!(c, Self::Entry(s) if s < lost))
                })
                .cloned()
                .unwrap_or_else(first_or_none),
        }
    }
}

impl BackupListPane {
    pub(super) fn entries<'a>(
        &self,
        ctx: &'a AppCtx,
    ) -> impl Iterator<Item = (&'a Slug, &'a BackupLibraryEntry)> + 'a {
        let connected = ctx.device_snapshot.as_ref().map(|s| s.usb_id);
        let needle = self.filter().needle_lower();
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

    pub(super) fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        ctx: &AppCtx,
        rename: Option<&RenameState>,
    ) {
        let (items, selected) = self.make_list_items(ctx, rename);
        self.render(frame, area, true, "Backups", items, selected);
    }

    fn make_list_items(
        &self,
        ctx: &AppCtx,
        rename: Option<&RenameState>,
    ) -> (Vec<ListItem<'static>>, Option<usize>) {
        let filtering = !self.filter().buffer().is_empty();
        let mut out = Vec::new();
        let mut selected = None;
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
                let is_selected = matches!(self.selection(), BackupCursor::Entry(s) if s == slug);
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
            Style::default().fg(muted()),
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
            spans.push(Span::styled(suffix, Style::default().fg(muted())));
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
