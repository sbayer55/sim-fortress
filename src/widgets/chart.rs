//! A time-series plot of half-block cells over labelled axes.
//! See `docs/components/chart.md`.
//!
//! Series are drawn with `▀ ▄` at half-row precision: each column averages
//! the days in its bin, odd halves draw `▄`, even halves `▀`, and zero draws
//! `▄` on the bottom row. Bands tint whole columns behind the series.

use std::fmt;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::component::Component;
use super::text::Text;
use crate::{glyphs, theme};

mod axes;
#[cfg(test)]
mod tests;

/// Cells left of the plot: a five-cell label and the axis column.
pub const LABEL_W: u16 = 6;

/// One line of the chart.
#[derive(Clone, Copy, Debug)]
pub struct Series<'a> {
    values: &'a [f32],
    color: Color,
}

impl<'a> Series<'a> {
    pub const fn new(values: &'a [f32]) -> Self {
        Self { values, color: theme::TEXT }
    }

    #[must_use]
    pub const fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

/// Columns whose bin holds a flagged day are tinted, with a label at the first.
#[derive(Clone, Copy, Debug)]
pub struct Band<'a> {
    days: &'a [bool],
    color: Color,
    label: Option<(char, &'a str)>,
}

impl<'a> Band<'a> {
    pub const fn new(days: &'a [bool]) -> Self {
        Self { days, color: theme::WARN, label: None }
    }

    #[must_use]
    pub const fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// `<glyph> <word>` on the top Plot row at the first tinted column + 1.
    #[must_use]
    pub const fn label(mut self, glyph: char, word: &'a str) -> Self {
        self.label = Some((glyph, word));
        self
    }

    fn tint(&self) -> Color {
        theme::lerp(theme::PANEL_BG, self.color, 0.22)
    }

    /// Whether column `c` of `cols` holds a flagged day.
    fn flagged(&self, c: usize, cols: usize) -> bool {
        let (a, b) = bin(self.days.len(), c, cols);
        self.days.get(a..b).is_some_and(|d| d.iter().any(|&f| f))
    }
}

/// Paint one plot column's background.
fn tint_column(buf: &mut Buffer, plot: Rect, x: u16, tint: Color) {
    for y in plot.top()..plot.bottom() {
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_char(' ');
            cell.set_style(Style::default().bg(tint));
        }
    }
}

/// The sample index range column `c` of `cols` averages, over `n` samples.
pub fn bin(n: usize, c: usize, cols: usize) -> (usize, usize) {
    let a = (c * n).div_euclid(cols.max(1));
    let b = (((c + 1) * n).div_euclid(cols.max(1))).max(a + 1).min(n);
    (a, b)
}

/// The mean of `v` over column `c`'s bin, 0 for an empty series.
pub fn mean_over(v: &[f32], c: usize, cols: usize) -> f32 {
    let (a, b) = bin(v.len(), c, cols);
    v.get(a..b).filter(|s| !s.is_empty()).map_or(0.0, |s| s.iter().sum::<f32>() / crate::cast!(s.len() => f32))
}

/// The series max rounded up to a multiple of `step`, never below `step`.
pub fn round_up(v: f32, step: f32) -> f32 {
    ((v / step).ceil() * step).max(step)
}

/// The label for sample `i`, e.g. `Y11 D124`.
pub type XLabel<'a> = &'a dyn Fn(usize) -> String;
/// The text of a y label for a value, e.g. `{v:.0}`.
pub type YLabel<'a> = &'a dyn Fn(f32) -> String;

/// Several series over one y scale, bands, reference rows and the axes.
pub struct Chart<'a> {
    series: Vec<Series<'a>>,
    bands: Vec<Band<'a>>,
    references: Vec<(f32, Color)>,
    y_step: f32,
    y_max: Option<f32>,
    y_label: Option<YLabel<'a>>,
    x_label: Option<XLabel<'a>>,
    legend: Option<Text<'a>>,
}

impl fmt::Debug for Chart<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Chart").field("series", &self.series.len()).field("bands", &self.bands.len()).field("y_step", &self.y_step).finish()
    }
}

impl Default for Chart<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Chart<'a> {
    pub const fn new() -> Self {
        Self { series: Vec::new(), bands: Vec::new(), references: Vec::new(), y_step: 100.0, y_max: None, y_label: None, x_label: None, legend: None }
    }

    #[must_use]
    pub fn series(mut self, series: Series<'a>) -> Self {
        self.series.push(series);
        self
    }

    #[must_use]
    pub fn band(mut self, band: Band<'a>) -> Self {
        self.bands.push(band);
        self
    }

    /// A dotted row at `value`, drawn behind the series on every other column.
    #[must_use]
    pub fn reference(mut self, value: f32, color: Color) -> Self {
        self.references.push((value, color));
        self
    }

    /// The y maximum is the series maximum rounded up to `step`.
    #[must_use]
    pub const fn y_step(mut self, step: f32) -> Self {
        self.y_step = step;
        self
    }

    /// A fixed y maximum instead of the rounded series maximum.
    #[must_use]
    pub const fn y_max(mut self, max: f32) -> Self {
        self.y_max = Some(max);
        self
    }

    #[must_use]
    pub const fn y_label(mut self, f: YLabel<'a>) -> Self {
        self.y_label = Some(f);
        self
    }

    /// Seven labels along the x axis, one per sample index the ticks land on.
    #[must_use]
    pub const fn x_label(mut self, f: XLabel<'a>) -> Self {
        self.x_label = Some(f);
        self
    }

    /// A row above the plot.
    #[must_use]
    pub fn legend(mut self, legend: Text<'a>) -> Self {
        self.legend = Some(legend);
        self
    }

    /// Samples in the window: the longest series.
    fn samples(&self) -> usize {
        self.series.iter().map(|s| s.values.len()).max().unwrap_or(0)
    }

    fn y_max_value(&self) -> f32 {
        self.y_max.unwrap_or_else(|| {
            let max = self.series.iter().flat_map(|s| s.values.iter().copied()).fold(0.0f32, f32::max);
            round_up(max, self.y_step)
        })
    }

    fn y_text(&self, v: f32) -> String {
        self.y_label.map_or_else(|| format!("{v:.0}"), |f| f(v))
    }

    fn x_text(&self, i: usize) -> String {
        self.x_label.map_or_else(String::new, |f| f(i))
    }

    /// The tinted columns, their labels, reference dots and series cells.
    fn plot_columns(&self, buf: &mut Buffer, plot: Rect, y_max: f32) {
        let cols = crate::cast!(plot.width => usize);
        let rows = crate::cast!(plot.height => usize);
        let n = self.samples();
        let mut labels: Vec<(u16, Band<'_>)> = Vec::new();
        for c in 0..cols {
            let x = plot.x + crate::cast!(c => u16);
            for band in self.bands.iter().filter(|band| n > 0 && band.flagged(c, cols)) {
                tint_column(buf, plot, x, band.tint());
                if !labels.iter().any(|(_, b)| std::ptr::eq(b.days, band.days)) {
                    labels.push((x, *band));
                }
            }
            self.reference_dots(buf, plot, x, c, rows, y_max);
            if n == 0 {
                continue;
            }
            for s in &self.series {
                series_cell(buf, plot, x, mean_over(s.values, c, cols), y_max, rows, s.color);
            }
        }
        for (x, band) in labels {
            if let Some((glyph, word)) = band.label {
                let text = format!("{glyph} {word}");
                buf.set_stringn(x + 1, plot.y, &text, text.chars().count() + 1, Style::default().fg(band.color).bg(band.tint()));
            }
        }
    }

    fn reference_dots(&self, buf: &mut Buffer, plot: Rect, x: u16, c: usize, rows: usize, y_max: f32) {
        if c % 2 != 0 {
            return;
        }
        for &(v, color) in &self.references {
            let r = rows - 1 - (crate::cast!(((v / y_max).clamp(0.0, 1.0) * (crate::cast!(rows => f32) - 1.0)).round() => usize)).min(rows - 1);
            if let Some(cell) = buf.cell_mut((x, plot.y + crate::cast!(r => u16))) {
                cell.set_char(glyphs::DOT);
                cell.set_style(Style::default().fg(theme::dim(color, 0.55)).bg(cell.bg));
            }
        }
    }
}

/// One series cell in column `x`: the value in half rows.
fn series_cell(buf: &mut Buffer, plot: Rect, x: u16, v: f32, y_max: f32, rows: usize, color: Color) {
    let halves = crate::cast!(((v / y_max).clamp(0.0, 1.0) * (crate::cast!(rows => f32) * 2.0)).round() => usize);
    if halves == 0 {
        // Flat zero: a lower half-block on the bottom row.
        if let Some(cell) = buf.cell_mut((x, plot.y + crate::cast!(rows => u16) - 1)) {
            cell.set_char(glyphs::HALF_LOWER);
            cell.set_style(Style::default().fg(color).bg(cell.bg));
        }
        return;
    }
    let r = rows - 1 - ((halves - 1).div_euclid(2)).min(rows - 1);
    let ch = if halves % 2 == 1 { glyphs::HALF_LOWER } else { glyphs::HALF_UPPER };
    if let Some(cell) = buf.cell_mut((x, plot.y + crate::cast!(r => u16))) {
        cell.set_char(ch);
        cell.set_style(Style::default().fg(color).bg(cell.bg).add_modifier(Modifier::BOLD));
    }
}

impl Component for Chart<'_> {
    /// The minimum; a chart takes whatever its container gives (use `Fill`).
    fn height(&self, _width: u16) -> u16 {
        4 + u16::from(self.legend.is_some())
    }

    fn min_width(&self) -> u16 {
        12
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        let mut area = area;
        if let Some(legend) = &self.legend {
            if area.height == 0 {
                return;
            }
            legend.render(buf, Rect { height: 1, ..area });
            area = Rect { y: area.y + 1, height: area.height - 1, ..area };
        }
        if area.height < 4 || area.width < 12 {
            return;
        }
        let plot = Rect::new(area.x + LABEL_W, area.y, area.width - LABEL_W - 1, area.height - 2);
        let y_max = self.y_max_value();
        self.plot_columns(buf, plot, y_max);
        axes::draw(buf, area, plot, self, y_max);
    }
}
