use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::Keybind;

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Quit,
    NextTab,
    PrevTab,
    GotoTab(usize),
    Help,
}

pub const KEYBINDS: &[Keybind] = &[
    Keybind {
        keys: "q",
        action: "Quit",
    },
    Keybind {
        keys: "Tab / ]",
        action: "Next tab",
    },
    Keybind {
        keys: "⇧Tab / [",
        action: "Previous tab",
    },
    Keybind {
        keys: "1-9",
        action: "Go to tab",
    },
    Keybind {
        keys: "?",
        action: "Toggle help",
    },
];

#[must_use]
pub fn map(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Tab | KeyCode::Char(']') => Some(Action::NextTab),
        KeyCode::BackTab | KeyCode::Char('[') => Some(Action::PrevTab),
        KeyCode::Char('?') => Some(Action::Help),
        KeyCode::Char(c @ '1'..='9') => c
            .to_digit(10)
            .and_then(|d| usize::try_from(d).ok())
            .map(|d| Action::GotoTab(d - 1)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyEvent, KeyModifiers};

    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn digits_map_to_zero_based_tab_index() {
        assert!(matches!(map(key('1')), Some(Action::GotoTab(0))));
        assert!(matches!(map(key('3')), Some(Action::GotoTab(2))));
        assert!(matches!(map(key('9')), Some(Action::GotoTab(8))));
    }

    #[test]
    fn zero_is_not_a_tab_shortcut() {
        assert!(map(key('0')).is_none());
    }

    #[test]
    fn question_mark_opens_help() {
        assert!(matches!(map(key('?')), Some(Action::Help)));
    }
}
