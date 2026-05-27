use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    text::Line,
    widgets::Paragraph,
};

pub fn render(frame: &mut Frame, area: Rect) {
    let [centered] = Layout::vertical([Constraint::Length(1)])
        .flex(Flex::Center)
        .areas(area);
    let para = Paragraph::new(Line::from("Coming soon")).alignment(Alignment::Center);
    frame.render_widget(para, centered);
}
