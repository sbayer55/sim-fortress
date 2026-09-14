//! S03 text styling helpers.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use crate::sim::SpeciesId;
use crate::ui::style::SpeciesStyle;
use crate::theme;

pub(super) fn sp(s: impl Into<String>, st: Style) -> Span<'static> {
    Span::styled(s.into(), st)
}

pub(super) fn species_style(id: SpeciesId) -> Style {
    Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)
}

pub(super) const fn trait_color(t: usize) -> Color {
    match t {
        0 => theme::INFO,
        1 => theme::DEER,
        2 => theme::ACCENT,
        3 => theme::WARN,
        4 => theme::BAD,
        5 => theme::VEGETATION,
        6 => theme::MAGENTA,
        7 => theme::LYNX,
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
