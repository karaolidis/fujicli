use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding},
};

use crate::{
    border_title,
    ui::{
        Keybind, actions,
        modals::{ModalHandler, ModalOutcome},
        widgets::{SEPARATOR, Scrollbar},
    },
};

const COLUMN_GAP: &str = "  ";
const PAD_X: u16 = 2;
const PAD_Y: u16 = 1;

pub struct HelpModal {
    context_label: &'static str,
    context: &'static [Keybind],
    scroll: usize,
}

impl HelpModal {
    #[must_use]
    pub const fn new(context_label: &'static str, context: &'static [Keybind]) -> Self {
        Self {
            context_label,
            context,
            scroll: 0,
        }
    }

    fn key_width(&self) -> usize {
        actions::KEYBINDS
            .iter()
            .chain(self.context)
            .map(|bind| bind.keys.chars().count())
            .max()
            .unwrap_or(0)
    }

    fn content_width(&self) -> usize {
        let actions_width = actions::KEYBINDS
            .iter()
            .chain(self.context)
            .map(|bind| bind.action.chars().count())
            .max()
            .unwrap_or(0);
        self.key_width() + COLUMN_GAP.len() + actions_width
    }

    fn title(&self) -> String {
        if self.context.is_empty() {
            border_title!(1, "Keybindings")
        } else {
            border_title!(1, "Keybindings{SEPARATOR}{}", self.context_label)
        }
    }

    fn row(bind: &Keybind, key_width: usize) -> Line<'static> {
        let pad = key_width.saturating_sub(bind.keys.chars().count());
        Line::from(vec![
            Span::raw(" ".repeat(pad)),
            Span::styled(bind.keys, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(COLUMN_GAP),
            Span::raw(bind.action),
        ])
    }

    fn centered(width: u16, height: u16, area: Rect) -> Rect {
        let [vertical] = Layout::vertical([Constraint::Length(height)])
            .flex(Flex::Center)
            .areas(area);
        let [horizontal] = Layout::horizontal([Constraint::Length(width)])
            .flex(Flex::Center)
            .areas(vertical);
        horizontal
    }
}

impl ModalHandler for HelpModal {
    fn on_key(&mut self, key: KeyEvent) -> ModalOutcome {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?' | 'q') => return ModalOutcome::Dismiss,
            KeyCode::Down | KeyCode::Char('j') => self.scroll = self.scroll.saturating_add(1),
            KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            _ => {}
        }
        ModalOutcome::Continue
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let key_width = self.key_width();
        let title = self.title();

        let mut items: Vec<ListItem> = actions::KEYBINDS
            .iter()
            .map(|bind| ListItem::new(Self::row(bind, key_width)))
            .collect();
        if !self.context.is_empty() {
            items.push(ListItem::new(Line::default()));
            items.extend(
                self.context
                    .iter()
                    .map(|bind| ListItem::new(Self::row(bind, key_width))),
            );
        }
        let content_len = items.len();

        let inner_width =
            (self.content_width() + usize::from(PAD_X) * 2).max(title.chars().count());
        let width = u16::try_from(inner_width)
            .unwrap_or(u16::MAX)
            .saturating_add(2);
        let height = u16::try_from(content_len)
            .unwrap_or(u16::MAX)
            .saturating_add(PAD_Y * 2 + 2);
        let popup = Self::centered(width, height, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .padding(Padding::symmetric(PAD_X, PAD_Y))
            .title(title);
        let inner = block.inner(popup);
        self.scroll = self
            .scroll
            .min(content_len.saturating_sub(usize::from(inner.height)));

        frame.render_widget(Clear, popup);
        frame.render_widget(block, popup);

        let mut list_state = ListState::default().with_offset(self.scroll);
        frame.render_stateful_widget(List::new(items), inner, &mut list_state);

        Scrollbar::draw(
            frame,
            Rect {
                x: popup.x,
                y: inner.y,
                width: popup.width,
                height: inner.height,
            },
            content_len,
            self.scroll,
        );
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyModifiers;

    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn dismisses_on_close_keys() {
        let mut modal = HelpModal::new("Simulations", &[]);
        assert!(matches!(
            modal.on_key(key(KeyCode::Char('?'))),
            ModalOutcome::Dismiss
        ));
        assert!(matches!(
            modal.on_key(key(KeyCode::Char('q'))),
            ModalOutcome::Dismiss
        ));
        assert!(matches!(
            modal.on_key(key(KeyCode::Esc)),
            ModalOutcome::Dismiss
        ));
    }

    #[test]
    fn scroll_keys_keep_modal_open() {
        let mut modal = HelpModal::new("Simulations", &[]);
        assert!(matches!(
            modal.on_key(key(KeyCode::Char('j'))),
            ModalOutcome::Continue
        ));
        assert!(matches!(
            modal.on_key(key(KeyCode::Char('x'))),
            ModalOutcome::Continue
        ));
    }

    #[test]
    fn scroll_up_saturates_at_top() {
        let mut modal = HelpModal::new("Simulations", &[]);
        modal.on_key(key(KeyCode::Down));
        assert_eq!(modal.scroll, 1);
        modal.on_key(key(KeyCode::Up));
        modal.on_key(key(KeyCode::Up));
        assert_eq!(modal.scroll, 0);
    }
}
