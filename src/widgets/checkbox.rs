//! A one-row on/off toggle with its key, or a focusable row of the S14
//! switcher. See `docs/components/checkbox.md`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::component::Component;
use crate::{glyphs, theme};

/// The three-cell mark: `[x]` / `[ ]`, or `(•)` / `( )` for a Radio row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Check,
    Radio,
}

/// `<indent> [x] Label   Value  ›` or ` [x] Label  [k]`.
#[derive(Clone, Debug)]
pub struct Checkbox<'a> {
    label: Cow<'a, str>,
    on: bool,
    key: Option<Cow<'a, str>>,
    /// The remembered sub-pick one blank after the Label pad (S14); takes the Key's place.
    value: Option<Cow<'a, str>>,
    /// The glyph after the Value slot that says a list opens to the right.
    cue: Option<char>,
    mark: Mark,
    /// The cursor row: the whole area in `theme::selected()`.
    focused: bool,
    /// Cannot be turned on: the whole row in `theme::dim_text()`.
    disabled: bool,
    label_w: u16,
    value_w: u16,
    indent: u16,
}

impl<'a> Checkbox<'a> {
    pub fn new(label: impl Into<Cow<'a, str>>, on: bool) -> Self {
        Self { label: label.into(), on, key: None, value: None, cue: None, mark: Mark::Check, focused: false, disabled: false, label_w: 36, value_w: 13, indent: 0 }
    }

    /// The letter that flips it, drawn as `[k]` after the Label pad.
    #[must_use]
    pub fn key(mut self, key: impl Into<Cow<'a, str>>) -> Self {
        self.key = Some(key.into());
        self
    }

    #[must_use]
    pub const fn label_w(mut self, w: u16) -> Self {
        self.label_w = w;
        self
    }

    /// Radio variant: `(•)` on, `( )` off.
    #[must_use]
    pub const fn radio(mut self, radio: bool) -> Self {
        self.mark = if radio { Mark::Radio } else { Mark::Check };
        self
    }

    /// Focused variant: the whole row in `theme::selected()`.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// A row that cannot be turned on, drawn in `theme::dim_text()`.
    #[must_use]
    pub const fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The remembered sub-pick, written one blank after the Label pad and cut to `value_w`.
    #[must_use]
    pub fn value(mut self, value: impl Into<Cow<'a, str>>) -> Self {
        self.value = Some(value.into());
        self
    }

    #[must_use]
    pub const fn value_w(mut self, w: u16) -> Self {
        self.value_w = w;
        self
    }

    /// A `»` directly after the Value slot: this row has a sub-pick list.
    #[must_use]
    pub const fn cue(mut self) -> Self {
        self.cue = Some(glyphs::CUE);
        self
    }

    /// Blank cells before the mark's own leading blank.
    #[must_use]
    pub const fn indent(mut self, cells: u16) -> Self {
        self.indent = cells;
        self
    }

    fn mark_text(&self) -> String {
        let (l, r) = match self.mark {
            Mark::Check => ('[', ']'),
            Mark::Radio => ('(', ')'),
        };
        let inner = match (self.mark, self.on) {
            (_, false) => ' ',
            (Mark::Check, true) => 'x',
            (Mark::Radio, true) => glyphs::BULLET,
        };
        format!("{l}{inner}{r}")
    }

    /// The background every cell of the row carries.
    const fn bg(&self) -> ratatui::style::Color {
        if self.focused {
            theme::SELECT_BG
        } else {
            theme::PANEL_BG
        }
    }

    fn mark_style(&self) -> Style {
        if self.disabled {
            theme::dim_text().bg(self.bg())
        } else if self.on {
            Style::default().fg(theme::GOOD).bg(self.bg()).add_modifier(Modifier::BOLD)
        } else {
            theme::dim_text().bg(self.bg())
        }
    }

    /// Bright and bold on the cursor row and when the row is on.
    fn label_style(&self) -> Style {
        if self.disabled {
            theme::dim_text().bg(self.bg())
        } else if self.focused || (self.on && self.value.is_some()) {
            Style::default().fg(theme::TEXT_BRIGHT).bg(self.bg()).add_modifier(Modifier::BOLD)
        } else {
            theme::text().bg(self.bg())
        }
    }

    fn value_style(&self) -> Style {
        if self.on && !self.disabled {
            theme::text().bg(self.bg())
        } else {
            theme::dim_text().bg(self.bg())
        }
    }

    fn cue_style(&self) -> Style {
        if self.focused {
            Style::default().fg(theme::ACCENT).bg(self.bg())
        } else {
            theme::dim_text().bg(self.bg())
        }
    }
}

impl Component for Checkbox<'_> {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        self.indent + 5 + crate::cast!(self.label.chars().count() => u16)
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let row = Rect { height: 1, ..area };
        if self.focused {
            super::util::fill(buf, row, theme::selected());
        }
        let w = crate::cast!(area.width => usize);
        let x0 = area.x.saturating_add(self.indent);
        if x0 >= area.right() {
            return;
        }
        let left = crate::cast!(area.right() - x0 => usize);
        buf.set_stringn(x0, area.y, format!(" {}", self.mark_text()), left, self.mark_style());
        if left <= 5 {
            return;
        }
        let lw = crate::cast!(self.label_w => usize);
        buf.set_stringn(x0 + 5, area.y, format!("{:<lw$}", self.label), left - 5, self.label_style());
        let kx = x0 + 5 + self.label_w;
        if let Some(key) = self.key.as_deref() {
            let token = format!("[{key}]");
            // Dropped when it does not fit; the Label is never cut for it.
            if kx + crate::cast!(token.chars().count() => u16) <= area.right() {
                buf.set_stringn(kx, area.y, &token, crate::cast!(area.right() - kx => usize), theme::key().bg(self.bg()));
            }
        }
        let vx = kx + 1;
        if let Some(value) = self.value.as_deref() {
            if vx < area.right() {
                let vw = usize::from(self.value_w).min(crate::cast!(area.right() - vx => usize));
                buf.set_stringn(vx, area.y, value, vw, self.value_style());
            }
        }
        if let Some(cue) = self.cue {
            let cx = vx + self.value_w;
            if cx < area.right() {
                buf.set_stringn(cx, area.y, cue.to_string(), w, self.cue_style());
            }
        }
    }
}
