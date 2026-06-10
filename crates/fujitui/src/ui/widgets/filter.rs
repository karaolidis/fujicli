use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::ui::{muted, widgets::text_input::TextInputState};

const PROMPT: &str = "> ";

#[derive(Debug, Default)]
pub struct FilterState {
    text: TextInputState,
    active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOutcome {
    Idle,
    ContentChanged,
    Closed,
}

impl FilterState {
    pub const fn active(&self) -> bool {
        self.active
    }

    pub fn buffer(&self) -> &str {
        &self.text.buffer
    }

    pub fn needle_lower(&self) -> String {
        self.text.buffer.to_lowercase()
    }

    #[cfg(test)]
    pub(crate) const fn text(&self) -> &TextInputState {
        &self.text
    }

    pub fn start(&mut self) {
        self.active = true;
        self.text.move_end();
    }

    pub const fn show_chip(&self) -> bool {
        self.active || !self.text.buffer.is_empty()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> FilterOutcome {
        match key.code {
            KeyCode::Esc => {
                self.text.clear();
                self.active = false;
                return FilterOutcome::Closed;
            }
            KeyCode::Enter => {
                self.active = false;
                return FilterOutcome::Closed;
            }
            KeyCode::Backspace if self.text.buffer.is_empty() => {
                self.active = false;
                return FilterOutcome::Closed;
            }
            _ => {}
        }
        if self.text.handle_edit_key(key) {
            FilterOutcome::ContentChanged
        } else {
            FilterOutcome::Idle
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let prompt = Span::styled(PROMPT, Style::default().fg(muted()));
        let line = if self.active {
            let mut spans = vec![prompt];
            spans.extend(self.text.cursor_spans(Style::default()));
            Line::from(spans)
        } else {
            Line::from(vec![
                prompt,
                Span::styled(self.text.buffer.clone(), Style::default().fg(muted())),
            ])
        };
        frame.render_widget(Paragraph::new(line), area);
    }
}
