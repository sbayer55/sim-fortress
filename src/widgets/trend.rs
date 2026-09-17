//! Trend Arrow: `↑ ↔ ↓` over the last 30 days. See `docs/components/trend-arrow.md`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::component::Component;
use crate::{glyphs, theme};

/// The C3 trend rule over a daily count series: > +3 % ↑, < −3 % ↓, else ↔,
/// comparing the last value with the one up to 30 days earlier.
pub fn trend_arrow(counts: &[u16]) -> char {
    if counts.len() < 2 {
        return glyphs::FLAT;
    }
    let a = f32::from(counts.get(counts.len().saturating_sub(30).min(counts.len() - 1)).copied().unwrap_or(0));
    let b = f32::from(counts.last().copied().unwrap_or(0));
    let pct = if a > 0.0 { (b - a) / a * 100.0 } else if b > 0.0 { f32::INFINITY } else { 0.0 };
    if pct > 3.0 {
        glyphs::UP
    } else if pct < -3.0 {
        glyphs::DOWN
    } else {
        glyphs::FLAT
    }
}

/// `↑` good, `↓` bad, `↔` dim.
pub const fn arrow_color(a: char) -> Color {
    match a {
        glyphs::UP => theme::GOOD,
        glyphs::DOWN => theme::BAD,
        _ => theme::DIM,
    }
}

/// One coloured arrow cell, or `↓ declining` with `.word()`.
#[derive(Clone, Copy, Debug)]
pub struct TrendArrow {
    arrow: char,
    bold: bool,
    word: bool,
}

impl TrendArrow {
    pub fn new(counts: &[u16]) -> Self {
        Self { arrow: trend_arrow(counts), bold: false, word: false }
    }

    pub const fn arrow(self) -> char {
        self.arrow
    }

    pub const fn color(self) -> Color {
        arrow_color(self.arrow)
    }

    #[must_use]
    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// `↓ declining`, `↔ stable` or `↑ growing`, 11 cells.
    #[must_use]
    pub const fn word(mut self) -> Self {
        self.word = true;
        self
    }

    fn text(self) -> String {
        if !self.word {
            return self.arrow.to_string();
        }
        let word = match self.arrow {
            glyphs::UP => "growing",
            glyphs::DOWN => "declining",
            _ => "stable",
        };
        format!("{} {word}", self.arrow)
    }
}

impl Component for TrendArrow {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        if self.word {
            11
        } else {
            1
        }
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let mut style = Style::default().fg(self.color()).bg(theme::PANEL_BG);
        if self.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        buf.set_stringn(area.x, area.y, self.text(), crate::cast!(area.width => usize), style);
    }
}
