use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{tui::App, ui::widgets::SEPARATOR};

const INFO_TTL: Duration = Duration::from_secs(5);
const ERROR_TTL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Error,
}

impl Severity {
    const fn ttl(self) -> Duration {
        match self {
            Self::Info => INFO_TTL,
            Self::Error => ERROR_TTL,
        }
    }
}

#[derive(Debug, Clone)]
struct StatusMessage {
    text: String,
    severity: Severity,
}

#[derive(Debug, Default)]
pub struct StatusQueue {
    errors: VecDeque<StatusMessage>,
    infos: VecDeque<StatusMessage>,
    current_started: Option<Instant>,
}

impl StatusQueue {
    pub fn push_info(&mut self, text: impl Into<String>) {
        self.infos.push_back(StatusMessage {
            text: text.into(),
            severity: Severity::Info,
        });
    }

    pub fn push_error(&mut self, text: impl Into<String>) {
        self.errors.push_back(StatusMessage {
            text: text.into(),
            severity: Severity::Error,
        });
    }

    fn head(&self) -> Option<&StatusMessage> {
        self.errors.front().or_else(|| self.infos.front())
    }

    fn advance(&mut self) {
        if self.errors.pop_front().is_none() {
            self.infos.pop_front();
        }
        self.current_started = None;
    }

    fn tick(&mut self) -> Option<&StatusMessage> {
        if let (Some(start), Some(msg)) = (self.current_started, self.head())
            && start.elapsed() >= msg.severity.ttl()
        {
            self.advance();
        }
        if self.current_started.is_none() && self.head().is_some() {
            self.current_started = Some(Instant::now());
        }
        self.head()
    }
}

pub struct Status;

impl Status {
    pub fn draw(app: &mut App, frame: &mut Frame, area: Rect) {
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

        if let Some(msg) = app.status.tick() {
            spans.push(Span::raw(SEPARATOR));
            let style = match msg.severity {
                Severity::Error => Style::default().fg(Color::Red),
                Severity::Info => Style::default(),
            };
            spans.push(Span::styled(msg.text.clone(), style));
        }

        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_queue_has_no_head() {
        let mut q = StatusQueue::default();
        assert!(q.tick().is_none());
    }

    #[test]
    fn errors_take_priority_over_infos() {
        let mut q = StatusQueue::default();
        q.push_info("info");
        q.push_error("error");
        let msg = q.tick().expect("head");
        assert_eq!(msg.text, "error");
        assert_eq!(msg.severity, Severity::Error);
    }

    #[test]
    fn fifo_within_same_severity() {
        let mut q = StatusQueue::default();
        q.push_info("first");
        q.push_info("second");
        assert_eq!(q.tick().expect("head").text, "first");
    }

    #[test]
    fn advance_drops_head() {
        let mut q = StatusQueue::default();
        q.push_error("a");
        q.push_error("b");
        q.tick();
        q.advance();
        assert_eq!(q.tick().expect("head").text, "b");
    }

    #[test]
    fn newly_pushed_error_jumps_in_front_of_pending_info() {
        let mut q = StatusQueue::default();
        q.push_info("info");
        q.tick();
        q.push_error("error");
        assert_eq!(q.tick().expect("head").text, "error");
    }
}
