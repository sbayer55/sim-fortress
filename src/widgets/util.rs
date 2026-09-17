//! Small drawing helpers shared by screens.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::Frame;

use super::component::Component;
use super::text::Text;
use crate::theme;

/// Fill a rect with a background color.
pub fn fill(buf: &mut Buffer, area: Rect, style: Style) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(c) = buf.cell_mut((x, y)) {
                c.set_symbol(" ");
                c.set_style(style);
            }
        }
    }
}

/// Render a `Line` at a row inside `area`.
pub fn line(f: &mut Frame<'_>, area: Rect, row: u16, l: Line<'_>) {
    line_in(f.buffer_mut(), area, row, l);
}

/// [`line`] straight into a buffer (for off-screen canvases).
pub fn line_in(buf: &mut Buffer, area: Rect, row: u16, l: Line<'_>) {
    if row >= area.height {
        return;
    }
    Text::line(l).render(buf, Rect::new(area.x, area.y + row, area.width, 1));
}

/// Dim every cell in the area toward the background (for modal backdrops).
pub fn dim_area(buf: &mut Buffer, area: Rect, amount: f32) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(c) = buf.cell_mut((x, y)) {
                let fg = theme::dim(c.fg, amount);
                let bg = theme::dim(c.bg, amount);
                c.set_fg(fg);
                c.set_bg(bg);
            }
        }
    }
}

/// Centered rect of the given size inside `area`.
pub fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(area.x + (area.width - w).div_euclid(2), area.y + (area.height - h).div_euclid(2), w, h)
}

/// Format a 0..=1 value as a percentage string like " 82%".
pub fn pct(v: f32) -> String {
    format!("{:>3}%", crate::cast!((v.clamp(0.0, 1.0) * 100.0).round() => u32))
}
