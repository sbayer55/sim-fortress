//! A race chart: several running totals against each other, one point per
//! sample at a two-cell pitch, joined by step lines. See
//! `docs/components/race-chart.md`.
//!
//! The lead series is drawn last with bold `■` points; the others are dim `∙`
//! points. Between two samples of a series the connector cell carries `─`
//! when the level is unchanged, or a riser of `│` with `┘ ┌` (rising) or
//! `┐ └` (falling) corners.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::component::Component;
use crate::{glyphs, theme};

#[cfg(test)]
mod tests;

/// How far a rival's colour is pulled toward the background.
const RIVAL_DIM: f32 = 0.55;
/// Cells per sample along the x axis.
const PITCH: u16 = 2;

/// One line of the race.
#[derive(Clone, Copy, Debug)]
pub struct RaceSeries<'a> {
    values: &'a [f32],
    start: usize,
    color: Color,
    lead: bool,
}

impl<'a> RaceSeries<'a> {
    pub const fn new(values: &'a [f32]) -> Self {
        Self { values, start: 0, color: theme::TEXT, lead: false }
    }

    /// The sample index of the first value (a line that started later).
    #[must_use]
    pub const fn start(mut self, sample: usize) -> Self {
        self.start = sample;
        self
    }

    #[must_use]
    pub const fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// The lead line: bold `■` points, drawn on top of the others.
    #[must_use]
    pub const fn lead(mut self, on: bool) -> Self {
        self.lead = on;
        self
    }

    /// The sample index one past the last value.
    const fn end(&self) -> usize {
        self.start + self.values.len()
    }

    fn style(&self) -> Style {
        let color = if self.lead { self.color } else { theme::dim(self.color, RIVAL_DIM) };
        let s = Style::default().fg(color).bg(theme::PANEL_BG);
        if self.lead {
            s.add_modifier(Modifier::BOLD)
        } else {
            s
        }
    }

    const fn point(&self) -> char {
        if self.lead {
            glyphs::SQUARE
        } else {
            glyphs::TRAIL
        }
    }
}

/// A title row with the current values, a plot of `rows` rows over a `│`
/// axis, and a `└───` foot with every second sample's label.
pub struct RaceChart<'a> {
    title: Cow<'a, str>,
    series: Vec<RaceSeries<'a>>,
    rows: u16,
    first_label: usize,
    y_max: Option<f32>,
    fmt: Option<&'a dyn Fn(f32) -> String>,
}

impl std::fmt::Debug for RaceChart<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RaceChart").field("title", &self.title).field("series", &self.series.len()).field("rows", &self.rows).finish()
    }
}

impl<'a> RaceChart<'a> {
    pub fn new(title: impl Into<Cow<'a, str>>) -> Self {
        Self { title: title.into(), series: Vec::new(), rows: 5, first_label: 1, y_max: None, fmt: None }
    }

    #[must_use]
    pub fn series(mut self, s: RaceSeries<'a>) -> Self {
        self.series.push(s);
        self
    }

    /// Plot rows between the title and the foot (default 5).
    #[must_use]
    pub const fn rows(mut self, rows: u16) -> Self {
        self.rows = rows;
        self
    }

    /// The x label of sample 0 (default 1: years).
    #[must_use]
    pub const fn first_label(mut self, n: usize) -> Self {
        self.first_label = n;
        self
    }

    /// A fixed y maximum instead of the series maximum.
    #[must_use]
    pub const fn y_max(mut self, v: f32) -> Self {
        self.y_max = Some(v);
        self
    }

    /// How the title values and the y-max label are written (default `{v:.0}`).
    #[must_use]
    pub const fn value_fmt(mut self, f: &'a dyn Fn(f32) -> String) -> Self {
        self.fmt = Some(f);
        self
    }

    fn fmt_value(&self, v: f32) -> String {
        self.fmt.map_or_else(|| format!("{v:.0}"), |f| f(v))
    }

    /// Samples in the race: the furthest end over the series.
    fn samples(&self) -> usize {
        self.series.iter().map(RaceSeries::end).max().unwrap_or(0)
    }

    fn y_max_value(&self) -> f32 {
        let observed = self.series.iter().flat_map(|s| s.values.iter().copied()).fold(0.0_f32, f32::max);
        self.y_max.unwrap_or(observed).max(1.0)
    }

    fn title_row(&self, buf: &mut Buffer, area: Rect) {
        let mut x = area.x;
        let end = area.x + area.width;
        let put = |buf: &mut Buffer, x: &mut u16, s: &str, style: Style| {
            if *x < end {
                buf.set_stringn(*x, area.y, s, usize::from(end - *x), style);
                *x += crate::cast!(s.chars().count() => u16);
            }
        };
        put(buf, &mut x, &self.title, theme::label());
        for s in &self.series {
            let Some(&last) = s.values.last() else { continue };
            put(buf, &mut x, " ", Style::default().bg(theme::PANEL_BG));
            let text = format!("{}{}", s.point(), self.fmt_value(last));
            put(buf, &mut x, &text, s.style());
        }
    }

    fn axes(&self, buf: &mut Buffer, plot: Rect, first: usize, visible: usize) {
        let axis = theme::border();
        for j in 0..plot.height {
            buf.set_stringn(plot.x, plot.y + j, glyphs::V_LINE.to_string(), 1, axis);
        }
        let foot = plot.y + plot.height;
        let rule: String = std::iter::once(glyphs::BOX_BL).chain(std::iter::repeat_n(glyphs::H_LINE, usize::from(plot.width.saturating_sub(1)))).collect();
        buf.set_stringn(plot.x, foot, &rule, usize::from(plot.width), axis);
        for k in (0..visible).step_by(2) {
            let x = plot.x + 1 + crate::cast!(k => u16) * PITCH;
            let label = (self.first_label + first + k).to_string();
            buf.set_stringn(x, foot, &label, usize::from((plot.x + plot.width).saturating_sub(x)), theme::dim_text());
        }
        buf.set_stringn(plot.x + 1, plot.y, self.fmt_value(self.y_max_value()), usize::from(plot.width.saturating_sub(1)), theme::dim_text());
    }

    fn draw_series(buf: &mut Buffer, plot: Rect, s: &RaceSeries<'_>, first: usize, lvl: &dyn Fn(f32) -> u16) {
        let style = s.style();
        let right = plot.x + plot.width;
        let mut prev: Option<u16> = None;
        for (k, &v) in s.values.iter().enumerate() {
            let i = s.start + k;
            let cy = lvl(v);
            if i < first {
                prev = Some(cy);
                continue;
            }
            let cx = plot.x + 1 + crate::cast!(i - first => u16) * PITCH;
            if cx >= right {
                break;
            }
            if let Some(py) = prev.filter(|_| i > first || k > 0) {
                let jx = cx - 1;
                if jx > plot.x {
                    connector(buf, jx, py, cy, style);
                }
            }
            buf.set_stringn(cx, cy, s.point().to_string(), 1, style);
            prev = Some(cy);
        }
    }
}

/// The step between two levels in the column before a point.
fn connector(buf: &mut Buffer, x: u16, py: u16, cy: u16, style: Style) {
    let cell = |buf: &mut Buffer, y: u16, g: char| buf.set_stringn(x, y, g.to_string(), 1, style);
    if py == cy {
        cell(buf, cy, glyphs::H_LINE);
        return;
    }
    let (lo, hi) = (py.min(cy), py.max(cy));
    for y in lo + 1..hi {
        cell(buf, y, glyphs::V_LINE);
    }
    if py > cy {
        cell(buf, py, glyphs::BOX_BR);
        cell(buf, cy, glyphs::BOX_TL);
    } else {
        cell(buf, py, glyphs::BOX_TR);
        cell(buf, cy, glyphs::BOX_BL);
    }
}

impl Component for RaceChart<'_> {
    fn height(&self, _width: u16) -> u16 {
        self.rows + 2
    }

    fn min_width(&self) -> u16 {
        5
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        let area = area.intersection(*buf.area());
        if area.height < 3 || area.width < 3 {
            return;
        }
        crate::widgets::util::fill(buf, area, Style::default().bg(theme::PANEL_BG));
        let rows = self.rows.min(area.height - 2).max(1);
        self.title_row(buf, Rect::new(area.x, area.y, area.width, 1));
        let plot = Rect::new(area.x, area.y + 1, area.width, rows);
        let visible = usize::from(area.width.div_euclid(PITCH));
        let samples = self.samples();
        let first = samples.saturating_sub(visible);
        self.axes(buf, plot, first, samples.saturating_sub(first));
        let y_max = self.y_max_value();
        let top = plot.y;
        let span = f32::from(rows - 1);
        let lvl = move |v: f32| top + rows - 1 - crate::cast!((v / y_max * span).round().clamp(0.0, span) => u16);
        let (rivals, leads): (Vec<_>, Vec<_>) = self.series.iter().partition(|s| !s.lead);
        for s in rivals.into_iter().chain(leads) {
            Self::draw_series(buf, plot, s, first, &lvl);
        }
    }
}
