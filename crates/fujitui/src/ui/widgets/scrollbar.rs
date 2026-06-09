use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Scrollbar as RatatuiScrollbar, ScrollbarOrientation, ScrollbarState},
};

const TRACK_SYMBOL: &str = "│";

pub struct Scrollbar;

impl Scrollbar {
    pub fn draw(frame: &mut Frame, track: Rect, content_len: usize, offset: usize) {
        let viewport = track.height as usize;
        if content_len <= viewport {
            return;
        }
        let mut state = ScrollbarState::new(content_len - viewport).position(offset);
        frame.render_stateful_widget(
            RatatuiScrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some(TRACK_SYMBOL)),
            track,
            &mut state,
        );
    }
}
