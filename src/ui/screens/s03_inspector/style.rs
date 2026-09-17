//! S03 text styling helpers.

use ratatui::style::Style;
use ratatui::text::Span;
use crate::theme;

pub(super) fn sp(s: impl Into<String>, st: Style) -> Span<'static> {
    Span::styled(s.into(), st)
}

// One palette for every screen: the S03 copy used to diverge (Res/Soc/Mat all
// in the sick colour), so the shared table is the only definition.
pub(super) use crate::ui::screens::common::trait_color;

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
