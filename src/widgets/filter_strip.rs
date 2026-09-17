//! A one-row strip of toggle chips with a key hint. See `docs/components/filter-strip.md`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use super::component::Component;
use crate::theme;

/// ` * all ♥ births … [f] cycles`: glyph, name and gap per chip, hint at the right.
#[derive(Clone, Debug)]
pub struct FilterStrip<'a> {
    chips: &'a [(char, &'a str)],
    active: &'a [bool],
    colors: &'a [Color],
    hint: Option<Cow<'a, str>>,
    gap: u16,
}

impl<'a> FilterStrip<'a> {
    pub const fn new(chips: &'a [(char, &'a str)]) -> Self {
        Self { chips, active: &[], colors: &[], hint: None, gap: 1 }
    }

    /// One flag per chip; missing entries are inactive.
    #[must_use]
    pub const fn active(mut self, active: &'a [bool]) -> Self {
        self.active = active;
        self
    }

    /// One glyph colour per chip; missing entries are `theme::TEXT`.
    #[must_use]
    pub const fn colors(mut self, colors: &'a [Color]) -> Self {
        self.colors = colors;
        self
    }

    #[must_use]
    pub fn hint(mut self, hint: impl Into<Cow<'a, str>>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    #[must_use]
    pub const fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    /// The chip set: a leading blank, then glyph, blank, name, and `gap` between chips.
    fn chips_width(&self) -> u16 {
        let gaps = self.gap.saturating_mul(crate::cast!(self.chips.len().saturating_sub(1) => u16));
        self.chips.iter().map(|(_, name)| crate::cast!(name.chars().count() => u16) + 2).fold(1 + gaps, u16::saturating_add)
    }
}

impl Component for FilterStrip<'_> {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        self.chips_width()
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let mut x = area.x + 1;
        for (i, (glyph, name)) in self.chips.iter().enumerate() {
            if x >= area.right() {
                break;
            }
            let color = self.colors.get(i).copied().unwrap_or(theme::TEXT);
            let on = self.active.get(i).copied().unwrap_or(false);
            let rem = |x: u16| crate::cast!(area.right().saturating_sub(x) => usize);
            buf.set_stringn(x, area.y, glyph.to_string(), rem(x), Style::default().fg(color).bg(theme::PANEL_BG));
            let name_style = if on { theme::selected() } else { theme::text() };
            buf.set_stringn(x + 1, area.y, format!(" {name}"), rem(x + 1), name_style);
            x = x.saturating_add(crate::cast!(name.chars().count() => u16) + 2 + self.gap);
        }
        if let Some(hint) = self.hint.as_deref() {
            let w = crate::cast!(hint.chars().count() => u16);
            let last_chip_end = area.x + self.chips_width();
            // Right-aligned one blank before the border; dropped when fewer
            // than two blanks would separate it from the last chip.
            if area.width > w + 1 {
                let hx = area.right() - 1 - w;
                if hx >= last_chip_end + 2 {
                    buf.set_stringn(hx, area.y, hint, crate::cast!(w => usize), theme::dim_text());
                }
            }
        }
    }
}
