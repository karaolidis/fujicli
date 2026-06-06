use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::Tab;

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Quit,
    NextTab,
    PrevTab,
    GotoTab(Tab),
}

#[must_use]
pub const fn map(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Tab | KeyCode::Char(']') => Some(Action::NextTab),
        KeyCode::BackTab | KeyCode::Char('[') => Some(Action::PrevTab),
        KeyCode::Char('1') => Some(Action::GotoTab(Tab::Simulation)),
        KeyCode::Char('2') => Some(Action::GotoTab(Tab::Render)),
        KeyCode::Char('3') => Some(Action::GotoTab(Tab::Backup)),
        _ => None,
    }
}
