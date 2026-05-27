use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::tui::Tab;

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Quit,
    NextTab,
    PrevTab,
    GotoTab(Tab),
}

#[must_use]
pub fn map(key: KeyEvent) -> Option<Action> {
    if key.kind != KeyEventKind::Press {
        return None;
    }

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
