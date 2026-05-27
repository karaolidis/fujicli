use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Tabs,
};

use crate::tui::{Tab, common::SEPARATOR};

pub fn render(frame: &mut Frame, area: Rect, current: Tab) {
    let titles: Vec<&'static str> = Tab::ALL.iter().map(|t| t.label()).collect();
    let tabs = Tabs::new(titles)
        .select(current.index())
        .divider(SEPARATOR)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_widget(tabs, area);
}
