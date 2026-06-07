use crossterm::event::{KeyEvent, KeyEventKind};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::ui::modals::{ModalEffect, ModalHandler, ModalOutcome, centered};

pub struct FatalModal {
    title: &'static str,
    body: &'static str,
}

impl FatalModal {
    #[must_use]
    pub const fn no_device() -> Self {
        Self {
            title: "No camera",
            body: "No supported camera connected. Press any key to quit.",
        }
    }

    #[must_use]
    pub const fn disconnect() -> Self {
        Self {
            title: "Disconnected",
            body: "Camera disconnected. Press any key to quit.",
        }
    }
}

impl ModalHandler for FatalModal {
    fn on_key(&mut self, key: KeyEvent) -> ModalOutcome {
        if key.kind != KeyEventKind::Press {
            return ModalOutcome::Continue;
        }
        ModalOutcome::Effect(ModalEffect::Quit)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let popup = centered(60, 20, area);
        frame.render_widget(Clear, popup);
        let block = Block::default().borders(Borders::ALL).title(self.title);
        let para = Paragraph::new(self.body)
            .alignment(Alignment::Center)
            .block(block);
        frame.render_widget(para, popup);
    }
}
