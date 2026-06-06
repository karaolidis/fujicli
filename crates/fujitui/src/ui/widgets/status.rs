use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{tui::App, ui::widgets::SEPARATOR};

#[derive(Debug, Default)]
pub struct Status;

impl Status {
    #[allow(clippy::unused_self)]
    pub fn render(&self, app: &App, frame: &mut Frame, area: Rect) {
        let mut spans: Vec<Span> = app.ctx.device_snapshot.as_ref().map_or_else(
            || vec![Span::raw("connecting...")],
            |snap| {
                vec![
                    Span::raw(snap.name),
                    Span::raw(SEPARATOR),
                    Span::raw(format!("{}%", snap.battery)),
                ]
            },
        );

        if let Some(msg) = &app.status_message {
            spans.push(Span::raw(SEPARATOR));
            spans.push(Span::raw(msg.as_str()));
        }

        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}
