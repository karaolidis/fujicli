use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

#[derive(Debug, Clone)]
pub struct TextInputState {
    pub buffer: String,
    pub cursor_col: usize,
    pub max_len: usize,
}

impl Default for TextInputState {
    fn default() -> Self {
        Self {
            buffer: String::new(),
            cursor_col: 0,
            max_len: usize::MAX,
        }
    }
}

impl TextInputState {
    #[allow(dead_code)]
    #[must_use]
    pub fn new(buffer: String) -> Self {
        Self::new_with_max_len(buffer, usize::MAX)
    }

    #[must_use]
    pub fn new_with_max_len(buffer: String, max_len: usize) -> Self {
        let cursor_col = buffer.chars().count();
        Self {
            buffer,
            cursor_col,
            max_len,
        }
    }

    pub fn char_count(&self) -> usize {
        self.buffer.chars().count()
    }

    fn byte_at(&self, char_idx: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(char_idx)
            .map_or(self.buffer.len(), |(i, _)| i)
    }

    pub const fn move_left(&mut self) {
        self.cursor_col = self.cursor_col.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        if self.cursor_col < self.char_count() {
            self.cursor_col += 1;
        }
    }

    pub const fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_col = self.char_count();
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor_col = 0;
    }

    pub fn insert(&mut self, c: char) -> bool {
        if self.char_count() >= self.max_len {
            return false;
        }
        let pos = self.byte_at(self.cursor_col);
        self.buffer.insert(pos, c);
        self.cursor_col += 1;
        true
    }

    pub fn delete_before(&mut self) -> bool {
        if self.cursor_col == 0 {
            return false;
        }
        let start = self.byte_at(self.cursor_col - 1);
        let end = self.byte_at(self.cursor_col);
        self.buffer.drain(start..end);
        self.cursor_col -= 1;
        true
    }

    pub fn delete_after(&mut self) -> bool {
        if self.cursor_col >= self.char_count() {
            return false;
        }
        let start = self.byte_at(self.cursor_col);
        let end = self.byte_at(self.cursor_col + 1);
        self.buffer.drain(start..end);
        true
    }

    pub fn handle_edit_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Backspace => self.delete_before(),
            KeyCode::Delete => self.delete_after(),
            KeyCode::Char(c) if !c.is_control() => self.insert(c),
            KeyCode::Left => {
                self.move_left();
                false
            }
            KeyCode::Right => {
                self.move_right();
                false
            }
            KeyCode::Home => {
                self.move_home();
                false
            }
            KeyCode::End => {
                self.move_end();
                false
            }
            _ => false,
        }
    }

    #[must_use]
    pub fn cursor_spans(&self, base: Style) -> Vec<Span<'static>> {
        let chars: Vec<char> = self.buffer.chars().collect();
        let before: String = chars.iter().take(self.cursor_col).collect();
        let at: String = chars
            .get(self.cursor_col)
            .map_or_else(|| " ".to_owned(), ToString::to_string);
        let after: String = chars.iter().skip(self.cursor_col + 1).collect();
        let cursor_style = base.add_modifier(Modifier::REVERSED);
        vec![
            Span::styled(before, base),
            Span::styled(at, cursor_style),
            Span::styled(after, base),
        ]
    }
}
