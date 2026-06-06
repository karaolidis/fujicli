use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    text::Line,
    widgets::Paragraph,
};

use crate::ui::tabs::{AppCtx, TabBehavior};

#[derive(Debug, Default)]
pub struct BackupTabState;

impl TabBehavior for BackupTabState {
    #[allow(clippy::unused_self)]
    fn render(&self, _ctx: &AppCtx, frame: &mut Frame, area: Rect) {
        let [centered] = Layout::vertical([Constraint::Length(1)])
            .flex(Flex::Center)
            .areas(area);
        let para = Paragraph::new(Line::from("Coming soon")).alignment(Alignment::Center);
        frame.render_widget(para, centered);
    }
}
