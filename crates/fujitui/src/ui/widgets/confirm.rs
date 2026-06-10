use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::ui::{danger, success};

#[derive(Debug, Clone)]
pub struct ConfirmState {
    pub title: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmOutcome {
    Pending,
    Confirmed,
    Cancelled,
}

impl ConfirmState {
    pub const fn handle_key(key: KeyEvent) -> ConfirmOutcome {
        match key.code {
            KeyCode::Char('y' | 'Y') | KeyCode::Enter => ConfirmOutcome::Confirmed,
            KeyCode::Char('n' | 'N') | KeyCode::Esc => ConfirmOutcome::Cancelled,
            _ => ConfirmOutcome::Pending,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let [vert] = Layout::vertical([Constraint::Length(7)])
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
        let [msg_area, hint_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
        let msg = Paragraph::new(self.message.clone()).alignment(Alignment::Center);
        frame.render_widget(msg, msg_area);
        let hint = Paragraph::new(Line::from(vec![
            Span::raw("["),
            Span::styled("y", Style::default().fg(success())),
            Span::raw("]es / ["),
            Span::styled("n", Style::default().fg(danger())),
            Span::raw("]o"),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(hint, hint_area);
    }
}
