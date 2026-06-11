use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    tui::App,
    ui::{danger, muted, warning, widgets::SEPARATOR},
    workers::device::DeviceHandle,
};

const INFO_TTL: Duration = Duration::from_secs(5);
const ERROR_TTL: Duration = Duration::from_secs(10);

pub const SPINNER_INTERVAL: Duration = Duration::from_millis(120);
const SPINNER_CELL: u16 = 2;

const DEVICE_GLYPH: &str = "●";
const DEVICE_SPINNER: [&str; 4] = ["◐", "◓", "◑", "◒"];

const FS_GLYPH: &str = "▤";
const FS_SPINNER: [&str; 4] = ["◰", "◳", "◲", "◱"];

fn spinner_frame(frames: &[&'static str], elapsed: Duration) -> &'static str {
    let idx = elapsed.as_millis() / SPINNER_INTERVAL.as_millis() % frames.len() as u128;
    frames[usize::try_from(idx).unwrap_or(0)]
}

const fn status_color(severity: Option<Severity>) -> Color {
    match severity {
        Some(Severity::Error) => danger(),
        Some(Severity::Info) => Color::Reset,
        None => muted(),
    }
}

pub const fn battery_color(percent: u32) -> Color {
    match percent {
        0..=15 => danger(),
        16..=30 => warning(),
        _ => Color::Reset,
    }
}

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
pub struct StatusMessage {
    pub severity: Severity,
    pub text: String,
}

impl StatusMessage {
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            text: text.into(),
        }
    }

    pub fn info(text: impl Into<String>) -> Self {
        Self {
            severity: Severity::Info,
            text: text.into(),
        }
    }
}

#[derive(Debug, Default)]
pub struct StatusQueue {
    errors: VecDeque<StatusMessage>,
    infos: VecDeque<StatusMessage>,
    current_started: Option<Instant>,
}

impl StatusQueue {
    pub fn push_info(&mut self, text: impl Into<String>) {
        self.infos.push_back(StatusMessage::info(text));
    }

    pub fn push_error(&mut self, text: impl Into<String>) {
        self.errors.push_back(StatusMessage::error(text));
    }

    pub fn push(&mut self, message: StatusMessage) {
        match message.severity {
            Severity::Info => self.infos.push_back(message),
            Severity::Error => self.errors.push_back(message),
        }
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
        let elapsed = app.started.elapsed();
        let device_busy = app.ctx.device.as_ref().is_some_and(DeviceHandle::is_busy);
        let fs_busy = app.ctx.fs.is_busy();
        let status = app.status.tick().map(|m| (m.severity, m.text.clone()));

        let [cam_area, content_area, fs_area] = Layout::horizontal([
            Constraint::Length(SPINNER_CELL),
            Constraint::Min(0),
            Constraint::Length(SPINNER_CELL),
        ])
        .areas(area);

        let severity = status.as_ref().map(|(severity, _)| *severity);
        let left = if device_busy {
            let color = match severity {
                Some(Severity::Error) => danger(),
                _ => Color::Reset,
            };
            Span::styled(
                spinner_frame(&DEVICE_SPINNER, elapsed),
                Style::default().fg(color),
            )
        } else {
            Span::styled(DEVICE_GLYPH, Style::default().fg(status_color(severity)))
        };
        frame.render_widget(Paragraph::new(Line::from(left)), cam_area);

        let mut spans: Vec<Span> = app.ctx.device_snapshot.as_ref().map_or_else(
            || vec![Span::raw("connecting...")],
            |snap| {
                vec![
                    Span::raw(snap.name),
                    Span::raw(SEPARATOR),
                    Span::styled(
                        format!("{}%", snap.battery),
                        Style::default().fg(battery_color(snap.battery)),
                    ),
                ]
            },
        );

        if let Some((severity, text)) = &status {
            spans.push(Span::raw(SEPARATOR));
            let style = match severity {
                Severity::Error => Style::default().fg(danger()),
                Severity::Info => Style::default(),
            };
            spans.push(Span::styled(text.clone(), style));
        }

        frame.render_widget(Paragraph::new(Line::from(spans)), content_area);

        let right = if fs_busy {
            Span::raw(spinner_frame(&FS_SPINNER, elapsed))
        } else {
            Span::styled(FS_GLYPH, Style::default().fg(muted()))
        };
        frame.render_widget(
            Paragraph::new(Line::from(right)).alignment(Alignment::Right),
            fs_area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_frame_advances_and_wraps() {
        assert_eq!(
            spinner_frame(&DEVICE_SPINNER, Duration::ZERO),
            DEVICE_SPINNER[0]
        );
        assert_eq!(
            spinner_frame(&DEVICE_SPINNER, Duration::from_millis(120)),
            DEVICE_SPINNER[1]
        );
        assert_eq!(
            spinner_frame(&DEVICE_SPINNER, Duration::from_millis(480)),
            DEVICE_SPINNER[0]
        );
    }

    #[test]
    fn status_color_reflects_queue_severity() {
        assert_eq!(status_color(Some(Severity::Error)), Color::Red);
        assert_eq!(status_color(Some(Severity::Info)), Color::Reset);
        assert_eq!(status_color(None), Color::DarkGray);
    }

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
