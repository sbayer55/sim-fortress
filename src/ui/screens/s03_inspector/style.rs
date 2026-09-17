//! S03 text styling helpers and the row builders the columns share.

use ratatui::style::{Color, Style};
use ratatui::text::Span;
use crate::theme;
use crate::widgets::{Component, Divider, Spacer, Text};

pub(super) fn sp(s: impl Into<String>, st: Style) -> Span<'static> {
    Span::styled(s.into(), st)
}

/// One row of styled spans.
pub(super) fn line(spans: Vec<Span<'static>>) -> Box<dyn Component + 'static> {
    Box::new(Text::spans(spans))
}

/// One row of one style.
pub(super) fn one(s: impl Into<String>, st: Style) -> Box<dyn Component + 'static> {
    line(vec![sp(s, st)])
}

/// A section Divider.
pub(super) fn section(title: &'static str) -> Box<dyn Component + 'static> {
    Box::new(Divider::new(title))
}

/// `n` blank rows.
pub(super) fn blank(n: u16) -> Box<dyn Component + 'static> {
    Box::new(Spacer::rows(n))
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
