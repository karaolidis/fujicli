use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::ui::widgets::text_input::TextInputState;

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
        let mut order_dirty = false;
        let mut close = false;
        let mut clear = false;
        match key.code {
            KeyCode::Esc => {
                close = true;
                clear = !self.text.buffer.is_empty();
            }
            KeyCode::Enter => close = true,
            KeyCode::Backspace => {
                if self.text.buffer.is_empty() {
                    close = true;
                } else {
                    order_dirty = self.text.delete_before();
                }
            }
            KeyCode::Delete => order_dirty = self.text.delete_after(),
            KeyCode::Left => self.text.move_left(),
            KeyCode::Right => self.text.move_right(),
            KeyCode::Home => self.text.move_home(),
            KeyCode::End => self.text.move_end(),
            KeyCode::Char(c) if !c.is_control() => {
                order_dirty = self.text.insert(c);
            }
            _ => {}
        }
        if clear {
            self.text.clear();
            order_dirty = true;
        }
        if close {
            self.active = false;
            return FilterOutcome::Closed;
        }
        if order_dirty {
            FilterOutcome::ContentChanged
        } else {
            FilterOutcome::Idle
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let prompt = Span::styled(PROMPT, Style::default().fg(Color::DarkGray));
        let line = if self.active {
            let mut spans = vec![prompt];
            spans.extend(self.text.cursor_spans(Style::default()));
            Line::from(spans)
        } else {
            Line::from(vec![
                prompt,
                Span::styled(
                    self.text.buffer.clone(),
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        };
        frame.render_widget(Paragraph::new(line), area);
    }
}
