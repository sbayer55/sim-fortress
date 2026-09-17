//! Range Bar: min, mean and max on a 0..1 scale. See `docs/components/range-bar.md`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::widgets::component::Component;
use crate::{glyphs, theme};

/// `[░░░▒▒█▒▒░░░]`: the span from min to max shaded, the mean a solid cell.
#[derive(Clone, Copy, Debug)]
pub struct RangeBar {
    min: f32,
    mean: f32,
    max: f32,
    color: Color,
    width: u16,
    text: bool,
}

impl RangeBar {
    /// Default width 13 (the S04 spread column).
    pub const fn new(min: f32, mean: f32, max: f32) -> Self {
        Self { min, mean, max, color: theme::TEXT, width: 13, text: false }
    }

    #[must_use]
    pub const fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Bar cells including the brackets; never below 3.
    #[must_use]
    pub fn width(mut self, w: u16) -> Self {
        self.width = w.max(3);
        self
    }

    /// Adds ` {lo:.2}..{hi:.2}` after the bar.
    #[must_use]
    pub const fn text(mut self) -> Self {
        self.text = true;
        self
    }

    /// The cell an input lands in: `0.0` is the first inner cell, `1.0` the last.
    fn cell(v: f32, inner: usize) -> usize {
        let last = inner.saturating_sub(1);
        (crate::cast!((v.clamp(0.0, 1.0) * crate::cast!(last => f32)).round() => usize)).min(last)
    }
}

impl Component for RangeBar {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        self.width + if self.text { 11 } else { 0 }
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.height == 0 || area.width < 3 {
            return;
        }
        let w = self.width.min(area.width);
        let inner = crate::cast!(w - 2 => usize);
        let (a, m, b) = (Self::cell(self.min, inner), Self::cell(self.mean, inner), Self::cell(self.max, inner));
        let glyph = |i: usize| {
            if i == m {
                glyphs::SHADE_4
            } else if i >= a && i <= b {
                glyphs::SHADE_2
            } else {
                glyphs::SHADE_1
            }
        };
        let frame: String = std::iter::once(glyphs::BAR_L).chain((0..inner).map(glyph)).chain(std::iter::once(glyphs::BAR_R)).collect();
        let dim = Style::default().fg(theme::DIM).bg(theme::PANEL_BG);
        buf.set_stringn(area.x, area.y, &frame, crate::cast!(w => usize), dim);
        let (lo, hi) = (a.min(m), b.max(m));
        let span: String = (lo..=hi).map(glyph).collect();
        let colored = Style::default().fg(self.color).bg(theme::PANEL_BG);
        buf.set_stringn(area.x + 1 + crate::cast!(lo => u16), area.y, &span, hi - lo + 1, colored);
        if self.text {
            let tx = area.x + w + 1;
            if tx < area.right() {
                let t = format!("{:.2}..{:.2}", self.min.clamp(0.0, 1.0), self.max.clamp(0.0, 1.0));
                buf.set_stringn(tx, area.y, &t, crate::cast!(area.right() - tx => usize), theme::dim_text());
            }
        }
    }
}
