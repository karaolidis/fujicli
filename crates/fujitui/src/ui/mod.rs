pub mod actions;
pub mod modals;
pub mod tabs;
pub mod widgets;

pub use actions::Action;
use ratatui::style::{Color, Style};
pub use tabs::Tab;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keybind {
    pub keys: &'static str,
    pub action: &'static str,
}

#[macro_export]
macro_rules! border_title {
    ($pad:expr, $($arg:tt)*) => {{
        let inner = ::std::format!($($arg)*);
        let pad = " ".repeat($pad);
        ::std::format!("{pad}{inner}{pad}")
    }};
}

pub const fn danger() -> Color {
    Color::Red
}

pub const fn success() -> Color {
    Color::Green
}

pub const fn warning() -> Color {
    Color::Yellow
}

pub const fn accent() -> Color {
    Color::Cyan
}

pub const fn muted() -> Color {
    Color::DarkGray
}

pub fn border_style(active: bool) -> Style {
    if active {
        Style::default()
    } else {
        Style::default().fg(muted())
    }
}
