use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use super::Scrollbar;
use crate::ui::{accent, danger, success};

const MARGIN: u16 = 3;

#[derive(Debug, Clone)]
pub struct SelectionState<T> {
    pub title: String,
    items: Vec<(String, T)>,
    cursor: usize,
    scroll: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionOutcome<T> {
    Pending,
    Cancelled,
    Picked(T),
}

impl<T: Clone> SelectionState<T> {
    pub const fn new(title: String, items: Vec<(String, T)>) -> Self {
        Self {
            title,
            items,
            cursor: 0,
            scroll: 0,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> SelectionOutcome<T> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => SelectionOutcome::Cancelled,
            KeyCode::Up | KeyCode::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                SelectionOutcome::Pending
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.items.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.items.len() - 1);
                }
                SelectionOutcome::Pending
            }
            KeyCode::Enter => self
                .items
                .get(self.cursor)
                .map_or(SelectionOutcome::Pending, |(_, value)| {
                    SelectionOutcome::Picked(value.clone())
                }),
            _ => SelectionOutcome::Pending,
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let rows = u16::try_from(self.items.len()).unwrap_or(u16::MAX);
        let height = rows.saturating_add(MARGIN).min(area.height);
        let [vert] = Layout::vertical([Constraint::Length(height)])
            .flex(Flex::Center)
            .areas(area);
        let [popup] = Layout::horizontal([Constraint::Percentage(50)])
            .flex(Flex::Center)
            .areas(vert);
        frame.render_widget(Clear, popup);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title.clone());
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let [list_area, hint_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);

        let items: Vec<ListItem<'static>> = self
            .items
            .iter()
            .map(|(label, _)| ListItem::new(Line::from(Span::raw(label.clone()))))
            .collect();
        let content_len = items.len();
        let mut state = ListState::default().with_offset(self.scroll);
        state.select(Some(self.cursor));
        let list =
            List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        frame.render_stateful_widget(list, list_area, &mut state);
        self.scroll = state.offset();
        Scrollbar::draw(
            frame,
            Rect {
                x: popup.x,
                y: list_area.y,
                width: popup.width,
                height: list_area.height,
            },
            content_len,
            self.scroll,
        );

        let hint = Paragraph::new(Line::from(vec![
            Span::styled("↑/↓", Style::default().fg(accent())),
            Span::raw(" navigate  "),
            Span::styled("enter", Style::default().fg(success())),
            Span::raw(" select  "),
            Span::styled("esc", Style::default().fg(danger())),
            Span::raw(" cancel"),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(hint, hint_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    fn three() -> SelectionState<usize> {
        SelectionState::new(
            "Pick".to_owned(),
            vec![
                ("a".to_owned(), 0),
                ("b".to_owned(), 1),
                ("c".to_owned(), 2),
            ],
        )
    }

    #[test]
    fn down_advances_and_clamps_at_end() {
        let mut s = three();
        assert_eq!(s.handle_key(key(KeyCode::Down)), SelectionOutcome::Pending);
        assert_eq!(
            s.handle_key(key(KeyCode::Char('j'))),
            SelectionOutcome::Pending
        );
        // at last item; further down stays put
        s.handle_key(key(KeyCode::Down));
        assert_eq!(
            s.handle_key(key(KeyCode::Enter)),
            SelectionOutcome::Picked(2)
        );
    }

    #[test]
    fn up_clamps_at_start() {
        let mut s = three();
        s.handle_key(key(KeyCode::Up));
        s.handle_key(key(KeyCode::Char('k')));
        assert_eq!(
            s.handle_key(key(KeyCode::Enter)),
            SelectionOutcome::Picked(0)
        );
    }

    #[test]
    fn esc_and_q_cancel() {
        let mut s = three();
        assert_eq!(s.handle_key(key(KeyCode::Esc)), SelectionOutcome::Cancelled);
        assert_eq!(
            s.handle_key(key(KeyCode::Char('q'))),
            SelectionOutcome::Cancelled
        );
    }

    #[test]
    fn enter_on_empty_is_pending() {
        let mut s = SelectionState::<usize>::new("Empty".to_owned(), Vec::new());
        assert_eq!(s.handle_key(key(KeyCode::Down)), SelectionOutcome::Pending);
        assert_eq!(s.handle_key(key(KeyCode::Enter)), SelectionOutcome::Pending);
    }
}
