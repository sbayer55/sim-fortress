//! The Chart's y labels, axis column, x axis with seven ticks, and `now`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::Chart;
use crate::{glyphs, theme};

/// Y labels at 0, ¼, ½, ¾ and max with `┼`, `│` on the other plot rows,
/// the `─` axis with a `┼` under each of seven x labels, and `now` at the end.
pub(super) fn draw(buf: &mut Buffer, area: Rect, plot: Rect, chart: &Chart<'_>, y_max: f32) {
    let axis_y = plot.bottom();
    let cols = crate::cast!(plot.width => usize);
    let rows = crate::cast!(plot.height => usize);
    for k in 0..=4 {
        let y = axis_y - 1 - crate::cast!((crate::cast!((rows - 1) => f32) * crate::cast!(k => f32) / 4.0).round() => u16);
        let label = chart.y_text(y_max * crate::cast!(k => f32) / 4.0);
        buf.set_stringn(area.x, y, format!("{label:>5}"), 5, theme::dim_text());
        buf.set_stringn(plot.x - 1, y, glyphs::CROSS.to_string(), 1, theme::border());
    }
    for r in 0..rows {
        let y = plot.y + crate::cast!(r => u16);
        if buf.cell((plot.x - 1, y)).is_some_and(|c| c.symbol() != "┼") {
            buf.set_stringn(plot.x - 1, y, glyphs::V_LINE.to_string(), 1, theme::border());
        }
    }
    let axis: String = std::iter::repeat_n(glyphs::H_LINE, cols + 1).collect();
    buf.set_stringn(plot.x - 1, axis_y, &axis, cols + 1, theme::border());
    let n = chart.samples().max(1);
    for k in 0..=6 {
        let i = ((k * (n - 1)).div_euclid(6)).min(n - 1);
        let x = if k == 6 { plot.x + crate::cast!(cols => u16) - 1 } else { plot.x + crate::cast!(((i * cols).div_euclid(n)) => u16) };
        buf.set_stringn(x, axis_y, glyphs::CROSS.to_string(), 1, theme::border());
        let label = if chart.samples() == 0 { String::new() } else { chart.x_text(i) };
        if !label.is_empty() {
            let w = label.chars().count();
            let lx = crate::cast!((i32::from(x) - crate::cast!(w => i32).div_euclid(2)).max(i32::from(area.x)) => u16);
            let lx = lx.min(area.right().saturating_sub(crate::cast!(w => u16) + 1));
            buf.set_stringn(lx, axis_y + 1, &label, w, theme::dim_text());
        }
    }
    buf.set_stringn(plot.right() - 3, axis_y, "now", 3, Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG));
}
