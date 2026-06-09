pub mod device;
pub mod fatal;
pub mod help;

use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
};

use crate::workers::device::usb::DeviceCandidate;

pub fn centered(pct_x: u16, pct_y: u16, area: Rect) -> Rect {
    let [vertical] = Layout::vertical([Constraint::Percentage(pct_y)])
        .flex(Flex::Center)
        .areas(area);
    let [horizontal] = Layout::horizontal([Constraint::Percentage(pct_x)])
        .flex(Flex::Center)
        .areas(vertical);
    horizontal
}

pub trait ModalHandler {
    fn on_key(&mut self, key: KeyEvent) -> ModalOutcome;
    fn render(&mut self, frame: &mut Frame, area: Rect);
}

pub enum ModalOutcome {
    Continue,
    Dismiss,
    Effect(ModalEffect),
}

pub enum ModalEffect {
    Quit,
    SelectDevice(DeviceCandidate),
}
