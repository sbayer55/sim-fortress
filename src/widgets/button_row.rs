//! A one-row set of bracketed buttons with one focused. See `docs/components/button-row.md`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::component::Component;
use crate::theme;

/// `[ text ]` buttons `gap` cells apart, centred as a group or starting at `indent`.
#[derive(Clone, Debug)]
pub struct ButtonRow<'a> {
    buttons: &'a [&'a str],
    focused: Option<usize>,
    primary: Option<usize>,
    gap: u16,
    /// `Some(indent)` for the Left variant.
    left: Option<u16>,
    hint: Option<Cow<'a, str>>,
}

impl<'a> ButtonRow<'a> {
    /// Each button text carries its own `[ ` and ` ]`.
    pub const fn new(buttons: &'a [&'a str]) -> Self {
        Self { buttons, focused: None, primary: None, gap: 4, left: None, hint: None }
    }

    /// The focused button, or `None` when focus is elsewhere.
    #[must_use]
    pub const fn focused(mut self, focused: Option<usize>) -> Self {
        self.focused = focused;
        self
    }

    /// The button that keeps a filled `theme::WARN` background when unfocused.
    #[must_use]
    pub const fn primary(mut self, primary: usize) -> Self {
        self.primary = Some(primary);
        self
    }

    #[must_use]
    pub const fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    /// Left variant: the group starts at `indent` instead of being centred.
    #[must_use]
    pub const fn left(mut self, indent: u16) -> Self {
        self.left = Some(indent);
        self
    }

    /// Dim text `gap` cells after the last button.
    #[must_use]
    pub fn hint(mut self, hint: impl Into<Cow<'a, str>>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    fn group_width(&self) -> u16 {
        let gaps = self.gap.saturating_mul(crate::cast!(self.buttons.len().saturating_sub(1) => u16));
        self.buttons.iter().map(|b| crate::cast!(b.chars().count() => u16)).fold(gaps, u16::saturating_add)
    }
}

impl Component for ButtonRow<'_> {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        let hint = self.hint.as_deref().map_or(0, |h| self.gap + crate::cast!(h.chars().count() => u16));
        self.left.unwrap_or(0) + self.group_width() + hint
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let mut x = area.x + self.left.unwrap_or_else(|| area.width.saturating_sub(self.group_width()).div_euclid(2));
        for (i, button) in self.buttons.iter().enumerate() {
            if x >= area.right() {
                return;
            }
            let style = if self.focused == Some(i) {
                theme::selected()
            } else if self.primary == Some(i) {
                Style::default().fg(theme::CURSOR_FG).bg(theme::WARN).add_modifier(Modifier::BOLD)
            } else {
                theme::text()
            };
            let w = crate::cast!(button.chars().count() => u16);
            buf.set_stringn(x, area.y, button, crate::cast!(area.right() - x => usize), style);
            x = x.saturating_add(w).saturating_add(self.gap);
        }
        if let Some(hint) = self.hint.as_deref() {
            if x < area.right() {
                buf.set_stringn(x, area.y, hint, crate::cast!(area.right() - x => usize), theme::dim_text());
            }
        }
    }
}
