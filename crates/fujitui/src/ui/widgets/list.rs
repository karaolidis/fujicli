use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::{Block, Borders, List, ListItem, ListState},
};

#[cfg(test)]
use crate::ui::widgets::TextInputState;
use crate::{
    border_title,
    ui::{
        border_style,
        widgets::{FilterOutcome, FilterState, Scrollbar},
    },
};

#[derive(Debug, Clone, Copy)]
pub enum CursorMove {
    Up,
    Down,
}

pub trait Cursor: Clone + PartialEq {
    fn none() -> Self;
    fn rehome(&self, order: &[Self]) -> Self;
}

#[derive(Debug, Default)]
pub struct ListPane<C> {
    selection: C,
    filter: FilterState,
    scroll: usize,
}

impl<C: Cursor> ListPane<C> {
    pub const fn selection(&self) -> &C {
        &self.selection
    }

    pub fn set_selection(&mut self, selection: C) {
        self.selection = selection;
    }

    pub const fn filtering(&self) -> bool {
        self.filter.active()
    }

    pub const fn filter(&self) -> &FilterState {
        &self.filter
    }

    #[cfg(test)]
    pub(crate) const fn filter_text(&self) -> &TextInputState {
        self.filter.text()
    }

    pub fn step(&mut self, dir: CursorMove, order: &[C]) {
        if order.is_empty() {
            self.selection = C::none();
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

    pub fn ensure_valid(&mut self, order: &[C]) {
        if order.contains(&self.selection) {
            return;
        }
        self.selection = self.selection.rehome(order);
    }

    pub fn start_filter(&mut self) {
        self.filter.start();
    }

    pub fn handle_filter_key(&mut self, key: KeyEvent) -> bool {
        matches!(
            self.filter.handle_key(key),
            FilterOutcome::ContentChanged | FilterOutcome::Closed,
        )
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        active: bool,
        title: &str,
        items: Vec<ListItem<'static>>,
        selected: Option<usize>,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(active))
            .title(border_title!(1, "{title}"));
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
}
