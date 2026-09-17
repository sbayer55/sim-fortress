//! A small vertical bar chart of `█`/`▄` stacks. See `docs/components/histogram.md`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::widgets::component::Component;
use crate::{glyphs, theme};

/// One column per bucket over an optional header, axis and footer.
#[derive(Clone, Debug)]
pub struct Histogram<'a> {
    buckets: &'a [u16],
    color: Color,
    col_w: u16,
    rows: u16,
    /// Name, min, mean, max.
    header: Option<(&'a str, f32, f32, f32)>,
    axis: bool,
    mark: Option<f32>,
    footer: bool,
    /// `┼` ticks with labels on the Axis, by bucket index.
    ticks: &'a [(usize, &'a str)],
}

impl<'a> Histogram<'a> {
    /// The Trait defaults: `col_w` 2, `rows` 4.
    pub const fn new(buckets: &'a [u16]) -> Self {
        Self { buckets, color: theme::TEXT, col_w: 2, rows: 4, header: None, axis: false, mark: None, footer: false, ticks: &[] }
    }

    #[must_use]
    pub const fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Cells per bucket, including the one-cell gutter; never below 1.
    #[must_use]
    pub const fn col_w(mut self, col_w: u16) -> Self {
        self.col_w = if col_w == 0 { 1 } else { col_w };
        self
    }

    /// Chart rows (the Columns), never below 1.
    #[must_use]
    pub const fn rows(mut self, rows: u16) -> Self {
        self.rows = if rows == 0 { 1 } else { rows };
        self
    }

    /// `Name .mm .mm .mm` above the Columns.
    #[must_use]
    pub const fn header(mut self, name: &'a str, min: f32, mean: f32, max: f32) -> Self {
        self.header = Some((name, min, mean, max));
        self
    }

    /// A `─` row under the Columns.
    #[must_use]
    pub const fn axis(mut self) -> Self {
        self.axis = true;
        self
    }

    /// A `┼` on the Axis at `mean` of the width.
    #[must_use]
    pub const fn mark(mut self, mean: f32) -> Self {
        self.mark = Some(mean);
        self.axis = true;
        self
    }

    /// `0  n=…  mode .mm  1` under the Axis.
    #[must_use]
    pub const fn footer(mut self) -> Self {
        self.footer = true;
        self
    }

    /// Group variant: `┼` ticks and labels on the Axis at these bucket indices.
    #[must_use]
    pub const fn ticks(mut self, ticks: &'a [(usize, &'a str)]) -> Self {
        self.ticks = ticks;
        self.axis = true;
        self
    }

    /// The Columns width: buckets × `col_w`.
    const fn width(&self) -> u16 {
        crate::cast!(self.buckets.len() => u16).saturating_mul(self.col_w)
    }

    fn draw_columns(&self, buf: &mut Buffer, area: Rect) {
        let max = f32::from(self.buckets.iter().copied().max().unwrap_or(1).max(1));
        let h = area.height;
        for (i, v) in self.buckets.iter().enumerate() {
            let x = area.x + crate::cast!(i => u16) * self.col_w;
            if x + self.col_w > area.right() {
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
                let s: String = std::iter::repeat_n(ch, crate::cast!(self.col_w.saturating_sub(1).max(1) => usize)).collect();
                buf.set_stringn(x, y, &s, crate::cast!(self.col_w => usize), Style::default().fg(self.color).bg(theme::PANEL_BG));
            }
        }
    }

    fn draw_header(&self, buf: &mut Buffer, x: u16, y: u16, width: usize) {
        let Some((name, min, mean, max)) = self.header else { return };
        let pct = |v: f32| crate::cast!((v * 100.0).round() => u32) % 100;
        buf.set_stringn(x, y, name, 11.min(width), Style::default().fg(self.color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
        if width > 11 {
            buf.set_stringn(x + 11, y, format!(".{:02} .{:02} .{:02}", pct(min), pct(mean), pct(max)), 14.min(width - 11), theme::dim_text());
        }
    }

    fn draw_axis(&self, buf: &mut Buffer, x: u16, y: u16, width: usize) {
        let line: String = std::iter::repeat_n(glyphs::H_LINE, width).collect();
        buf.set_stringn(x, y, &line, width, theme::border());
        if let Some(mean) = self.mark {
            let last = width.saturating_sub(1);
            let mx = (crate::cast!((mean.clamp(0.0, 1.0) * crate::cast!(last => f32)).round() => usize)).min(last);
            buf.set_stringn(x + crate::cast!(mx => u16), y, glyphs::CROSS.to_string(), 1, Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG));
        }
        for &(bucket, label) in self.ticks {
            let tx = crate::cast!(bucket => u16).saturating_mul(self.col_w);
            if usize::from(tx) >= width {
                continue;
            }
            buf.set_stringn(x + tx, y, glyphs::CROSS.to_string(), 1, theme::border());
            buf.set_stringn(x + tx + 1, y, label, width.saturating_sub(usize::from(tx) + 1), theme::dim_text());
        }
    }

    fn draw_footer(&self, buf: &mut Buffer, x: u16, y: u16, width: usize) {
        buf.set_stringn(x, y, "0", width, theme::dim_text());
        let n: u32 = self.buckets.iter().map(|&v| u32::from(v)).sum();
        let peak = self.buckets.iter().enumerate().max_by_key(|(_, v)| **v).map_or(0, |(i, _)| i);
        let mode = (crate::cast!(peak => f32) + 0.5) / crate::cast!(self.buckets.len().max(1) => f32);
        if width > 2 {
            buf.set_stringn(x + 2, y, format!("n={n}"), 7.min(width - 2), theme::dim_text());
        }
        if width > 11 {
            buf.set_stringn(x + 11, y, format!("mode .{:02}", crate::cast!((mode * 100.0).round() => u32) % 100), 11.min(width - 11), theme::dim_text());
        }
        if width > 1 {
            buf.set_stringn(x + crate::cast!(width - 1 => u16), y, "1", 1, theme::dim_text());
        }
    }
}

impl Component for Histogram<'_> {
    fn height(&self, _width: u16) -> u16 {
        self.rows + u16::from(self.header.is_some()) + u16::from(self.axis) + u16::from(self.footer)
    }

    fn min_width(&self) -> u16 {
        self.width()
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.is_empty() {
            return;
        }
        let width = crate::cast!(self.width().min(area.width) => usize);
        let mut y = area.y;
        if self.header.is_some() && y < area.bottom() {
            self.draw_header(buf, area.x, y, width);
            y += 1;
        }
        let rows = self.rows.min(area.bottom().saturating_sub(y));
        if rows > 0 {
            self.draw_columns(buf, Rect::new(area.x, y, area.width, rows));
        }
        y = y.saturating_add(self.rows);
        if self.axis && y < area.bottom() {
            self.draw_axis(buf, area.x, y, width);
            y += 1;
        }
        if self.footer && y < area.bottom() {
            self.draw_footer(buf, area.x, y, width);
        }
    }
}
