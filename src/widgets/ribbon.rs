//! A ribbon: one value per tick of a chase, plotted over the chase clock. See
//! `docs/components/ribbon.md`.
//!
//! The top row is the escape edge and the bottom row the kill edge: a value
//! near 1 sits low, near 0 sits high. A few stalk ticks on a grey ground lead
//! up to the rule where the clock started; the clock zone that follows ends at
//! `┤`, the clock expiring. The newest tick is `►` while the hunt is open; a
//! resolved hunt shows its outcome glyph on its coloured edge, and a ribbon
//! drawn `live(false)` is pulled toward the panel ground as a memory of the
//! last hunt.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::component::Component;
use crate::{glyphs, theme};

#[cfg(test)]
mod tests;

/// How much a resolved or idle ribbon is pulled toward the panel ground.
const IDLE_FADE: f32 = 0.45;

/// How a plotted hunt ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RibbonEnd {
    Kill,
    Escaped,
    TimedOut,
    Lost,
}

impl RibbonEnd {
    const fn glyph(self) -> char {
        match self {
            Self::Kill => glyphs::DEATH,
            Self::Escaped | Self::TimedOut => glyphs::RIGHT,
            Self::Lost => glyphs::DOT,
        }
    }

    const fn color(self) -> Color {
        match self {
            Self::Kill => theme::BAD,
            Self::Escaped | Self::TimedOut => theme::GOOD,
            Self::Lost => theme::DIM,
        }
    }

    /// The edge the final tick is pinned to.
    const fn value(self) -> f32 {
        match self {
            Self::Kill => 1.0,
            _ => 0.0,
        }
    }
}

/// The values of one hunt: stalk ticks before the clock, chase ticks after.
#[derive(Clone, Debug)]
pub struct Ribbon<'a> {
    stalk: &'a [f32],
    chase: &'a [f32],
    outcome: Option<RibbonEnd>,
    live: bool,
    caption: Option<Cow<'a, str>>,
    chase_max: usize,
    stalk_ticks: usize,
    seg: u16,
    rows: u16,
}

impl<'a> Ribbon<'a> {
    /// `stalk` and `chase` are values in 0..=1, oldest first; only the last
    /// `stalk_ticks` stalk values and the first `chase_max` chase values show.
    pub const fn new(stalk: &'a [f32], chase: &'a [f32]) -> Self {
        Self { stalk, chase, outcome: None, live: true, caption: None, chase_max: 30, stalk_ticks: 6, seg: 2, rows: 5 }
    }

    /// The outcome drawn at the final tick; it also pins that tick to its edge.
    #[must_use]
    pub const fn outcome(mut self, end: Option<RibbonEnd>) -> Self {
        self.outcome = end;
        self
    }

    /// `false` fades every colour toward the panel ground: the last hunt, remembered.
    #[must_use]
    pub const fn live(mut self, live: bool) -> Self {
        self.live = live;
        self
    }

    /// Text centred on the middle row of the clock zone.
    #[must_use]
    pub fn caption(mut self, text: impl Into<Cow<'a, str>>) -> Self {
        self.caption = Some(text.into());
        self
    }

    /// Ticks on the chase clock (default 30, `predation.chase_max_ticks`).
    #[must_use]
    pub const fn chase_max(mut self, ticks: usize) -> Self {
        self.chase_max = ticks;
        self
    }

    /// Stalk ticks shown before the clock rule (default 6).
    #[must_use]
    pub const fn stalk_ticks(mut self, ticks: usize) -> Self {
        self.stalk_ticks = ticks;
        self
    }

    /// Cells per tick (default 2: a mark and a connector).
    #[must_use]
    pub const fn seg(mut self, cells: u16) -> Self {
        self.seg = if cells == 0 { 1 } else { cells };
        self
    }

    /// Plot rows (default 5, minimum 2).
    #[must_use]
    pub const fn rows(mut self, rows: u16) -> Self {
        self.rows = if rows < 2 { 2 } else { rows };
        self
    }

    /// The row a value lands on: 0 is the escape edge, `rows − 1` the kill edge.
    pub fn row_of(&self, value: f32) -> u16 {
        crate::cast!((value.clamp(0.0, 1.0) * f32::from(self.rows - 1)).round() => u16)
    }

    fn fade(&self, c: Color) -> Color {
        if self.live {
            c
        } else {
            theme::lerp(c, theme::PANEL_BG, IDLE_FADE)
        }
    }

    /// Cells per tick as a usize, for geometry.
    fn pitch(&self) -> usize {
        usize::from(self.seg)
    }

    fn cell_x(area: Rect, x: usize) -> Option<u16> {
        let x = crate::cast!(x => u16);
        (x < area.width).then(|| area.x + x)
    }

    /// The grounds, both rules and the tick labels.
    fn draw_frame(&self, buf: &mut Buffer, area: Rect, zones: &Zones) {
        let (stalk_bg, chase_bg) = (self.fade(theme::lerp(theme::PANEL_BG, theme::DIM, 0.35)), self.fade(theme::lerp(theme::PANEL_BG, theme::INFO, 0.25)));
        for r in 0..self.rows {
            let Some(y) = (r < area.height).then(|| area.y + r) else { break };
            let edge = |base: Color, good: f32, bad: f32| {
                if r == 0 {
                    theme::lerp(base, self.fade(theme::GOOD), good)
                } else if r == self.rows - 1 {
                    theme::lerp(base, self.fade(theme::BAD), bad)
                } else {
                    base
                }
            };
            for x in 0..zones.rule {
                if let Some(cx) = Self::cell_x(area, x) {
                    buf[(cx, y)].set_char(' ').set_style(Style::default().bg(edge(stalk_bg, 0.15, 0.18)));
                }
            }
            for x in zones.clock..zones.end {
                if let Some(cx) = Self::cell_x(area, x) {
                    buf[(cx, y)].set_char(' ').set_style(Style::default().bg(edge(chase_bg, 0.18, 0.22)));
                }
            }
            if let Some(cx) = Self::cell_x(area, zones.rule) {
                buf[(cx, y)].set_char(glyphs::V_LINE).set_style(Style::default().fg(self.fade(theme::WARN)).bg(theme::PANEL_BG));
            }
            if let Some(cx) = Self::cell_x(area, zones.end) {
                buf[(cx, y)].set_char(glyphs::CLOCK_OUT).set_style(Style::default().fg(self.fade(theme::BAD)).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
            }
        }
        // Tick labels every ten ticks on the kill edge.
        let y = area.y + self.rows - 1;
        if y < area.bottom() {
            for k in (10..self.chase_max).step_by(10) {
                self.draw_label(buf, area, y, zones.clock + k * self.pitch(), &k.to_string());
            }
        }
    }

    fn draw_label(&self, buf: &mut Buffer, area: Rect, y: u16, x: usize, label: &str) {
        for (i, ch) in label.chars().enumerate() {
            let Some(lx) = Self::cell_x(area, x + i) else { return };
            let bg = buf[(lx, y)].bg;
            buf[(lx, y)].set_char(ch).set_style(Style::default().fg(self.fade(theme::DIM)).bg(bg));
        }
    }

    /// Every plotted tick as `(x, value)`, oldest first.
    fn points(&self, zones: &Zones) -> Vec<(usize, f32)> {
        let pitch = self.pitch();
        let shown = self.stalk.len().min(self.stalk_ticks);
        let mut pts: Vec<(usize, f32)> = self.stalk[self.stalk.len() - shown..]
            .iter()
            .enumerate()
            .map(|(i, &v)| (zones.rule - pitch * (shown - i), v))
            .collect();
        pts.extend(self.chase.iter().take(self.chase_max).enumerate().map(|(j, &v)| (zones.clock + pitch * j, v)));
        if let (Some(end), Some(last)) = (self.outcome, pts.last_mut()) {
            last.1 = end.value();
        }
        pts
    }

    fn draw_points(&self, buf: &mut Buffer, area: Rect, pts: &[(usize, f32)]) {
        let last = pts.len().saturating_sub(1);
        let mut prev_row: Option<u16> = None;
        for (i, &(x, v)) in pts.iter().enumerate() {
            let row = self.row_of(v);
            let Some(cx) = Self::cell_x(area, x) else { prev_row = Some(row); continue };
            let y = area.y + row;
            if y >= area.bottom() {
                break;
            }
            let fg = self.fade(theme::lerp(theme::GOOD, theme::BAD, v));
            let bg = buf[(cx, y)].bg;
            if let Some(pr) = prev_row {
                Self::draw_riser(buf, area, cx, pr, row, fg);
            }
            for dx in 1..self.pitch() {
                if let Some(nx) = Self::cell_x(area, x + dx) {
                    let nbg = buf[(nx, y)].bg;
                    buf[(nx, y)].set_char(glyphs::H_LINE).set_style(Style::default().fg(theme::lerp(fg, nbg, 0.5)).bg(nbg));
                }
            }
            let newest = i == last;
            match (newest, self.outcome) {
                (true, Some(end)) => {
                    let ground = theme::lerp(bg, self.fade(end.color()), 0.6);
                    buf[(cx, y)].set_char(end.glyph()).set_style(Style::default().fg(self.fade(theme::TEXT_BRIGHT)).bg(ground).add_modifier(Modifier::BOLD));
                }
                (true, None) if self.live => {
                    buf[(cx, y)].set_char(glyphs::PLAY).set_style(Style::default().fg(theme::TEXT_BRIGHT).bg(bg).add_modifier(Modifier::BOLD));
                }
                _ => {
                    buf[(cx, y)].set_char(glyphs::SQUARE).set_style(Style::default().fg(fg).bg(bg));
                }
            }
            prev_row = Some(row);
        }
    }

    /// The `│` cells between two rows a value jumped across, in the new tick's column.
    fn draw_riser(buf: &mut Buffer, area: Rect, cx: u16, from: u16, to: u16, fg: Color) {
        let (lo, hi) = (from.min(to), from.max(to));
        for rr in lo + 1..hi {
            let yy = area.y + rr;
            if yy < area.bottom() {
                let cbg = buf[(cx, yy)].bg;
                buf[(cx, yy)].set_char(glyphs::V_LINE).set_style(Style::default().fg(theme::lerp(fg, cbg, 0.4)).bg(cbg));
            }
        }
    }

    fn draw_caption(&self, buf: &mut Buffer, area: Rect, zones: &Zones) {
        let Some(text) = self.caption.as_deref() else { return };
        let text = format!(" {text} ");
        let n = text.chars().count();
        let width = zones.end - zones.clock;
        let start = zones.clock + width.saturating_sub(n).div_euclid(2);
        let y = area.y + self.rows.div_euclid(2);
        if y >= area.bottom() {
            return;
        }
        for (i, ch) in text.chars().enumerate().take(width) {
            if let Some(cx) = Self::cell_x(area, start + i) {
                let bg = buf[(cx, y)].bg;
                buf[(cx, y)].set_char(ch).set_style(Style::default().fg(self.fade(theme::TEXT)).bg(bg));
            }
        }
    }
}

/// Column offsets inside the area: the stalk zone is `0..rule`, the clock zone
/// `clock..end`, with the two rules at `rule` and `end`.
struct Zones {
    rule: usize,
    clock: usize,
    end: usize,
}

impl Component for Ribbon<'_> {
    fn height(&self, _width: u16) -> u16 {
        self.rows
    }

    fn min_width(&self) -> u16 {
        crate::cast!(self.pitch() * (self.stalk_ticks + self.chase_max) + 2 => u16)
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.height == 0 || area.width < 3 {
            return;
        }
        let rule = self.pitch() * self.stalk_ticks;
        let zones = Zones { rule, clock: rule + 1, end: rule + 1 + self.pitch() * self.chase_max };
        self.draw_frame(buf, area, &zones);
        let pts = self.points(&zones);
        self.draw_points(buf, area, &pts);
        self.draw_caption(buf, area, &zones);
    }
}
