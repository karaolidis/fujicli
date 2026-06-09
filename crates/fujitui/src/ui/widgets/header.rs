use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Tabs,
};

use crate::{tui::App, ui::widgets::SEPARATOR};

pub struct Header;

impl Header {
    pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
        let available = app.available_tabs();
        let selected = available
            .iter()
            .position(|t| *t == app.active_tab)
            .unwrap_or(0);
        let titles: Vec<&'static str> = available.iter().map(|t| t.label()).collect();
        let tabs = Tabs::new(titles)
            .select(selected)
            .divider(SEPARATOR)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        frame.render_widget(tabs, area);
    }
}
