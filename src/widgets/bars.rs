//! Bars: labeled gauges, range bars, sparklines and histograms. The free
//! functions are thin wrappers over the components; screens move to the
//! structs section by section.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use super::component::Component;
use crate::{glyphs, theme};

mod labeled;
mod range;
mod sparkline;
#[cfg(test)]
mod tests;

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
pub fn histogram(buf: &mut Buffer, area: Rect, values: &[u16], color: Color, col_w: u16) {
    let max = f32::from(values.iter().copied().max().unwrap_or(1).max(1));
    let h = area.height;
    for (i, v) in values.iter().enumerate() {
        let x = area.x + crate::cast!(i => u16) * col_w;
        if x + col_w > area.right() {
            break;
        }
        // Two half-cells per row.
        let halves = crate::cast!(((f32::from(*v) / max) * (f32::from(h) * 2.0)).round() => u16);
        for r in 0..h {
            let y = area.bottom() - 1 - r;
            let level = halves.saturating_sub(r * 2);
            let ch = if level >= 2 {
                glyphs::FULL_BLOCK
            } else if level == 1 {
                glyphs::HALF_LOWER
            } else {
                continue;
            };
            let s: String = std::iter::repeat_n(ch, crate::cast!(col_w.saturating_sub(1).max(1) => usize)).collect();
            buf.set_stringn(x, y, &s, crate::cast!(col_w => usize), Style::default().fg(color).bg(theme::PANEL_BG));
        }
    }
}
