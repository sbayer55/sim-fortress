//! Bars: labeled gauges, range bars, sparklines and histograms. The free
//! functions are thin wrappers over the components; screens move to the
//! structs section by section.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::component::Component;

mod histogram;
mod labeled;
mod range;
mod sparkline;
#[cfg(test)]
mod tests;

pub use histogram::Histogram;
pub use labeled::{vital_color, Bar, Inverted, LabeledBar};
pub use range::RangeBar;
pub use sparkline::Sparkline;

/// `label [████░░░░]  82%` on one row. `area.width` bounds the whole thing.
pub fn labeled(buf: &mut Buffer, area: Rect, row: u16, label: &str, value: f32, color: Color, label_w: u16, bar_w: u16) {
    if row >= area.height {
        return;
    }
    LabeledBar::new(label, value).color(color).label_and_bar(label_w, bar_w).render(buf, Rect::new(area.x, area.y + row, area.width, 1));
}

/// Just the `[████░░░░]` part, `w` cells wide including brackets.
pub fn bar(buf: &mut Buffer, x: u16, y: u16, w: u16, value: f32, color: Color) {
    Bar::new(value).color(color).render(buf, Rect::new(x, y, w, 1));
}

/// A range bar showing min..max as a shaded span with a marker at `mean`.
/// Used for trait distributions: `[░░░▒▒▒█▒▒░░░░]`.
pub fn range(buf: &mut Buffer, x: u16, y: u16, w: u16, min: f32, mean: f32, max: f32, color: Color) {
    RangeBar::new(min, mean, max).color(color).width(w).render(buf, Rect::new(x, y, w, 1));
}

/// One-row sparkline using `░▒▓█` intensity (CP437-safe).
pub fn sparkline(buf: &mut Buffer, x: u16, y: u16, w: u16, values: &[u16], color: Color) {
    Sparkline::new(values).color(color).render(buf, Rect::new(x, y, w, 1));
}

/// Vertical histogram in an area: `values` become columns of `▄`/`█` stacks.
/// Thin wrapper over the Bare [`Histogram`]; the area height is the chart height.
pub fn histogram(buf: &mut Buffer, area: Rect, values: &[u16], color: Color, col_w: u16) {
    Histogram::new(values).color(color).col_w(col_w).rows(area.height).render(buf, area);
}
