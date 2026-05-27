use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::tui::{App, common::SEPARATOR};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let mut spans: Vec<Span> = app.snapshot.as_ref().map_or_else(
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
