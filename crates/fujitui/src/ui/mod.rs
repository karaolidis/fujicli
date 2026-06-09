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

pub fn border_style(active: bool) -> Style {
    if active {
        Style::default()
    } else {
        Style::default().fg(Color::DarkGray)
    }
}
