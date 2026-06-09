use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem},
};

use crate::{
    ui::modals::{ModalEffect, ModalHandler, ModalOutcome, centered},
    workers::device::usb::DeviceCandidate,
};

pub struct DevicePickerModal {
    candidates: Vec<DeviceCandidate>,
    cursor: usize,
}

impl DevicePickerModal {
    #[must_use]
    pub const fn new(candidates: Vec<DeviceCandidate>) -> Self {
        Self {
            candidates,
            cursor: 0,
        }
    }
}

impl ModalHandler for DevicePickerModal {
    fn on_key(&mut self, key: KeyEvent) -> ModalOutcome {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                ModalOutcome::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.cursor + 1 < self.candidates.len() {
                    self.cursor += 1;
                }
                ModalOutcome::Continue
            }
            KeyCode::Enter => {
                let candidate = self.candidates.swap_remove(self.cursor);
                ModalOutcome::Effect(ModalEffect::SelectDevice(candidate))
            }
            KeyCode::Esc | KeyCode::Char('q') => ModalOutcome::Effect(ModalEffect::Quit),
            _ => ModalOutcome::Continue,
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let popup = centered(60, 40, area);
        frame.render_widget(Clear, popup);

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Select a camera");
        let items: Vec<ListItem> = self
            .candidates
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let line = format!("{} ({}) {}.{}", c.name, c.usb_id, c.bus, c.address);
                let style = if i == self.cursor {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(line, style)))
            })
            .collect();
        frame.render_widget(List::new(items).block(block), popup);
    }
}
