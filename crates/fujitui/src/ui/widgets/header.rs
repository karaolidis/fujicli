use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Tabs,
};

use crate::{
    tui::App,
    ui::{Tab, widgets::SEPARATOR},
};

#[derive(Debug, Default)]
pub struct Header;

impl Header {
    #[allow(clippy::unused_self)]
    pub fn render(&self, app: &App, frame: &mut Frame, area: Rect) {
        let titles: Vec<&'static str> = Tab::ALL.iter().map(|t| t.label()).collect();
        let tabs = Tabs::new(titles)
            .select(app.active_tab.index())
            .divider(SEPARATOR)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        frame.render_widget(tabs, area);
    }
}
