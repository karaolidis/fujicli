use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::ui::muted;

const LOADING: &str = "loading...";

pub struct Loading;

impl Loading {
    pub fn draw(frame: &mut Frame, area: Rect) {
        let [centered] = Layout::vertical([Constraint::Length(1)])
            .flex(Flex::Center)
            .areas(area);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                LOADING,
                Style::default().fg(muted()),
            )))
            .alignment(Alignment::Center),
            centered,
        );
    }
}
