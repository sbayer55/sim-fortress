//! Stepper and Text Field: a label, a boxed value and a hint on one row.
//! See `docs/components/stepper.md` and `text-field.md`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::component::Component;
use crate::{glyphs, theme};

#[cfg(test)]
mod tests;

/// The shared geometry: ` Label ` then a box `open value close` then ` hint`.
struct Boxed<'a> {
    label: &'a str,
    open: char,
    close: char,
    value: Cow<'a, str>,
    hint: &'a str,
    focused: bool,
    label_w: u16,
    box_w: u16,
}

impl Boxed<'_> {
    const fn min_width(&self) -> u16 {
        self.label_w + self.box_w + 2
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.height == 0 || area.width < self.min_width() {
            return;
        }
        let (x, y) = (area.x, area.y);
        let label_style = if self.focused { theme::label().add_modifier(Modifier::BOLD) } else { theme::text() };
        let edge_style = if self.focused { Style::default().fg(theme::KEY).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD) } else { theme::dim_text() };
        let well = if self.focused { theme::selected() } else { Style::default().fg(theme::TEXT_BRIGHT).bg(theme::BG) };
        let lw = crate::cast!(self.label_w => usize);
        buf.set_stringn(x + 1, y, format!("{:<lw$}", self.label), lw, label_style);
        let bx = x + 1 + self.label_w;
        buf.set_stringn(bx, y, self.open.to_string(), 1, edge_style);
        let vw = crate::cast!(self.box_w - 3 => usize);
        buf.set_stringn(bx + 1, y, format!(" {:<vw$}", self.value), vw + 1, well);
        buf.set_stringn(bx + self.box_w - 1, y, self.close.to_string(), 1, edge_style);
        let hx = bx + self.box_w + 1;
        if hx < area.right() {
            buf.set_stringn(hx, y, self.hint, crate::cast!(area.right() - hx => usize), theme::dim_text());
        }
    }
}

/// Which shape a Stepper takes.
#[derive(Clone, Debug)]
enum Shape<'a> {
    /// ` Label ◄ value ► hint`.
    Full { label: &'a str, hint: &'a str, typing: Option<&'a str>, label_w: u16, box_w: u16 },
    /// `◄  240 ►`: value right-aligned in `value_w` plus one blank.
    Compact { value_w: u16 },
    /// `◄ text   ► [←→]`: arrows bracket label and value together.
    Inline { text_w: u16, key: &'a str },
}

/// A value adjusted with `←` / `→`. The value is a pre-formatted string;
/// ranges, steps and typed entry stay with the screen.
#[derive(Clone, Debug)]
pub struct Stepper<'a> {
    value: Cow<'a, str>,
    shape: Shape<'a>,
    focused: bool,
}

impl<'a> Stepper<'a> {
    /// The full S09 row: `label_w` 20, `box_w` 26.
    pub fn new(label: &'a str, value: impl Into<Cow<'a, str>>) -> Self {
        Self { value: value.into(), shape: Shape::Full { label, hint: "", typing: None, label_w: 20, box_w: 26 }, focused: false }
    }

    /// Compact: no Label or Hint, value right-aligned in 5.
    pub fn compact(value: impl Into<Cow<'a, str>>) -> Self {
        Self { value: value.into(), shape: Shape::Compact { value_w: 5 }, focused: false }
    }

    /// Inline: the arrows bracket the whole text, then a key token.
    pub fn inline(text: impl Into<Cow<'a, str>>) -> Self {
        Self { value: text.into(), shape: Shape::Inline { text_w: 24, key: "" }, focused: false }
    }

    #[must_use]
    pub const fn hint(mut self, h: &'a str) -> Self {
        if let Shape::Full { hint, .. } = &mut self.shape {
            *hint = h;
        }
        self
    }

    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Typing variant: draws the buffer followed by `_` instead of the value.
    #[must_use]
    pub const fn typing(mut self, buffer: Option<&'a str>) -> Self {
        if let Shape::Full { typing, .. } = &mut self.shape {
            *typing = buffer;
        }
        self
    }

    #[must_use]
    pub const fn label_w(mut self, w: u16) -> Self {
        if let Shape::Full { label_w, .. } = &mut self.shape {
            *label_w = w;
        }
        self
    }

    #[must_use]
    pub const fn box_w(mut self, w: u16) -> Self {
        if let Shape::Full { box_w, .. } = &mut self.shape {
            *box_w = if w < 4 { 4 } else { w };
        }
        self
    }

    /// Compact: the value's right-aligned width.
    #[must_use]
    pub const fn value_w(mut self, w: u16) -> Self {
        if let Shape::Compact { value_w } = &mut self.shape {
            *value_w = w;
        }
        self
    }

    /// Inline: the key token after the arrows, e.g. `[←→]`.
    #[must_use]
    pub const fn key(mut self, k: &'a str) -> Self {
        if let Shape::Inline { key, .. } = &mut self.shape {
            *key = k;
        }
        self
    }

    /// Inline: the width the text is padded to inside the arrows.
    #[must_use]
    pub const fn text_w(mut self, w: u16) -> Self {
        if let Shape::Inline { text_w, .. } = &mut self.shape {
            *text_w = w;
        }
        self
    }
}

impl Component for Stepper<'_> {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        match self.shape {
            Shape::Full { label_w, box_w, .. } => label_w + box_w + 2,
            Shape::Compact { value_w } => value_w + 3,
            Shape::Inline { text_w, key, .. } => 3 + text_w + 1 + crate::cast!(key.chars().count() => u16),
        }
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let w = crate::cast!(area.width => usize);
        match self.shape {
            Shape::Full { label, hint, typing, label_w, box_w } => {
                let value = typing.map_or_else(|| Cow::Borrowed(self.value.as_ref()), |t| Cow::Owned(format!("{t}_")));
                Boxed { label, open: glyphs::REWIND, close: glyphs::PLAY, value, hint, focused: self.focused, label_w, box_w }.render(buf, area);
            }
            Shape::Compact { value_w } => {
                let vw = crate::cast!(value_w => usize);
                // The row around a focused Compact field is a selection bar; the arrows keep its background.
                let arrows = if self.focused { theme::key().bg(theme::SELECT_BG) } else { theme::dim_text() };
                let value = if self.focused { theme::selected() } else { theme::text() };
                buf.set_stringn(area.x, area.y, glyphs::REWIND.to_string(), w, arrows);
                buf.set_stringn(area.x + 1, area.y, format!("{:>vw$} ", self.value), w.saturating_sub(1), value);
                buf.set_stringn(area.x + value_w + 2, area.y, glyphs::PLAY.to_string(), w.saturating_sub(usize::from(value_w) + 2), arrows);
            }
            Shape::Inline { text_w, key } => {
                let tw = crate::cast!(text_w => usize);
                buf.set_stringn(area.x, area.y, format!("{} ", glyphs::REWIND), w, theme::key());
                buf.set_stringn(area.x + 2, area.y, format!("{:<tw$}", self.value), w.saturating_sub(2), theme::text());
                buf.set_stringn(area.x + 2 + text_w, area.y, format!("{} {key}", glyphs::PLAY), w.saturating_sub(usize::from(text_w) + 2), theme::key());
            }
        }
    }
}

/// A free-text field: ` Label [ text ] hint`. Editing stays with the screen.
#[derive(Clone, Debug)]
pub struct TextField<'a> {
    label: &'a str,
    text: Cow<'a, str>,
    hint: &'a str,
    focused: bool,
    label_w: u16,
    box_w: u16,
}

impl<'a> TextField<'a> {
    pub fn new(label: &'a str, text: impl Into<Cow<'a, str>>) -> Self {
        Self { label, text: text.into(), hint: "", focused: false, label_w: 20, box_w: 26 }
    }

    #[must_use]
    pub const fn hint(mut self, hint: &'a str) -> Self {
        self.hint = hint;
        self
    }

    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    #[must_use]
    pub const fn label_w(mut self, w: u16) -> Self {
        self.label_w = w;
        self
    }

    #[must_use]
    pub const fn box_w(mut self, w: u16) -> Self {
        self.box_w = if w < 4 { 4 } else { w };
        self
    }
}

impl Component for TextField<'_> {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        self.label_w + self.box_w + 2
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        Boxed {
            label: self.label,
            open: glyphs::BAR_L,
            close: glyphs::BAR_R,
            value: Cow::Borrowed(self.text.as_ref()),
            hint: self.hint,
            focused: self.focused,
            label_w: self.label_w,
            box_w: self.box_w,
        }
        .render(buf, area);
    }
}
