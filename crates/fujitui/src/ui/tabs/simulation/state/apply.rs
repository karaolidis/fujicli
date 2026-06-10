use crossterm::event::KeyEvent;
use fujicore::generated::options::CustomSetting;
use ratatui::{Frame, layout::Rect};

use crate::{
    border_title,
    ui::widgets::{SelectionOutcome, SelectionState},
    workers::fs::slug::Slug,
};

pub(super) enum ApplyOutcome {
    Pending,
    Cancelled,
    Picked(CustomSetting),
}

#[derive(Debug)]
pub(super) struct ApplyState {
    slug: Slug,
    entry_name: String,
    selection: SelectionState<CustomSetting>,
}

impl ApplyState {
    pub(super) fn new(slug: Slug, entry_name: String, slots: Vec<(CustomSetting, String)>) -> Self {
        let title = border_title!(1, "Apply {entry_name} to slot");
        let items = slots
            .into_iter()
            .map(|(slot, label)| (label, slot))
            .collect();
        Self {
            slug,
            entry_name,
            selection: SelectionState::new(title, items),
        }
    }

    pub(super) fn into_parts(self) -> (Slug, String) {
        (self.slug, self.entry_name)
    }

    pub(super) fn handle_key(&mut self, key: KeyEvent) -> ApplyOutcome {
        match self.selection.handle_key(key) {
            SelectionOutcome::Pending => ApplyOutcome::Pending,
            SelectionOutcome::Cancelled => ApplyOutcome::Cancelled,
            SelectionOutcome::Picked(slot) => ApplyOutcome::Picked(slot),
        }
    }

    pub(super) fn draw(&mut self, frame: &mut Frame, area: Rect) {
        self.selection.draw(frame, area);
    }
}
