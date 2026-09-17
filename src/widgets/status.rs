//! Bottom status / key-hint bar. See `docs/components/status-bar.md`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use super::component::Component;
use super::key_hint::KeyHint;
use crate::theme;

#[cfg(test)]
mod tests;

/// `[key] label` pairs from the left and a right-aligned message. Whole pairs
/// are dropped from the right when Right would overlap them; Right is then
/// cut to the area width.
#[derive(Clone, Debug)]
pub struct StatusBar<'a> {
    keys: &'a [(&'a str, &'a str)],
    right: Cow<'a, str>,
    right_fg: Color,
    /// A dim note drawn just left of Right (C9: `AI: offline`); empty draws nothing.
    note: Cow<'a, str>,
}

impl<'a> StatusBar<'a> {
    pub const fn new(keys: &'a [(&'a str, &'a str)]) -> Self {
        Self { keys, right: Cow::Borrowed(""), right_fg: theme::ACCENT, note: Cow::Borrowed("") }
    }

    /// A dim note left of Right. Empty leaves the bar byte-identical.
    #[must_use]
    pub fn note(mut self, note: impl Into<Cow<'a, str>>) -> Self {
        self.note = note.into();
        self
    }

    #[must_use]
    pub fn right(mut self, right: impl Into<Cow<'a, str>>) -> Self {
        self.right = right.into();
        self
    }

    #[must_use]
    pub const fn right_color(mut self, color: Color) -> Self {
        self.right_fg = color;
        self
    }

    /// ` [k] label  `: the pair's cells including its two trailing blanks.
    fn pair_width((k, label): (&str, &str)) -> u16 {
        crate::cast!(k.chars().count() + label.chars().count() + 5 => u16)
    }

    fn right_width(&self) -> u16 {
        if self.right.is_empty() {
            0
        } else {
            crate::cast!(self.right.chars().count() => u16) + 1
        }
    }

    /// The note plus its two trailing cells, or 0.
    fn note_width(&self) -> u16 {
        if self.note.is_empty() {
            0
        } else {
            crate::cast!(self.note.chars().count() => u16) + 2
        }
    }

    /// How many pairs fit beside Right in `width` cells.
    fn shown(&self, width: u16) -> usize {
        let room = width.saturating_sub(self.right_width() + self.note_width());
        let mut used = 1u16; // the row's leading cell
        let mut n = 0;
        for &pair in self.keys {
            used = used.saturating_add(Self::pair_width(pair));
            if used > room {
                break;
            }
            n += 1;
        }
        n
    }
}

impl Component for StatusBar<'_> {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    /// Right plus its trailing cell plus the first pair.
    fn min_width(&self) -> u16 {
        self.right_width() + self.keys.first().map_or(0, |&p| 1 + Self::pair_width(p))
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let bg = Style::default().bg(theme::STATUS_BG);
        super::util::fill(buf, Rect { height: 1, ..area }, bg);
        let mut x = area.x + 1;
        for &(k, label) in self.keys.iter().take(self.shown(area.width)) {
            let w = Self::pair_width((k, label)) - 2;
            KeyHint::new(k, label).bg(theme::STATUS_BG).render(buf, Rect::new(x, area.y, w, 1));
            x += w + 2;
        }
        if !self.right.is_empty() {
            let w = self.right_width().min(area.width);
            let rx = area.right() - w;
            buf.set_stringn(rx, area.y, self.right.as_ref(), crate::cast!(w - 1 => usize), bg.fg(self.right_fg));
        }
        let nw = self.note_width();
        if nw > 0 && self.right_width() + nw <= area.width {
            let nx = area.right() - self.right_width() - nw;
            buf.set_stringn(nx, area.y, self.note.as_ref(), crate::cast!(nw - 2 => usize), bg.fg(theme::DIM));
        }
    }
}
