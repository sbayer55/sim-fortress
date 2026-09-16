//! S03 text styling helpers.

use ratatui::style::{Color, Style};
use ratatui::text::Span;
use crate::theme;

pub(super) fn sp(s: impl Into<String>, st: Style) -> Span<'static> {
    Span::styled(s.into(), st)
}

pub(super) const fn trait_color(t: usize) -> Color {
    match t {
        0 => theme::INFO,
        1 => theme::TAN,
        2 => theme::ACCENT,
        3 => theme::WARN,
        4 => theme::BAD,
        5 => theme::VEGETATION,
        6 => theme::MAGENTA,
        7 => theme::ROSE,
        _ => theme::SICK,
    }
}

pub(super) fn delta_style(d: f32) -> Style {
    let c = if d > 0.005 {
        theme::GOOD
    } else if d < -0.005 {
        theme::BAD
    } else {
        theme::DIM
    };
    Style::default().fg(c).bg(theme::PANEL_BG)
}
