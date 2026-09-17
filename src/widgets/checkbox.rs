//! A one-row on/off toggle with its key. See `docs/components/checkbox.md`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::component::Component;
use crate::theme;

/// ` [x] Label  [k]`.
#[derive(Clone, Debug)]
pub struct Checkbox<'a> {
    label: Cow<'a, str>,
    on: bool,
    key: Option<Cow<'a, str>>,
    label_w: u16,
}

impl<'a> Checkbox<'a> {
    pub fn new(label: impl Into<Cow<'a, str>>, on: bool) -> Self {
        Self { label: label.into(), on, key: None, label_w: 36 }
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
}

impl Component for Checkbox<'_> {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        5 + crate::cast!(self.label.chars().count() => u16)
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let w = crate::cast!(area.width => usize);
        let mark = if self.on { (" [x]", Style::default().fg(theme::GOOD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)) } else { (" [ ]", theme::dim_text()) };
        buf.set_stringn(area.x, area.y, mark.0, w, mark.1);
        if area.width <= 5 {
            return;
        }
        let lw = crate::cast!(self.label_w => usize);
        buf.set_stringn(area.x + 5, area.y, format!("{:<lw$}", self.label), w - 5, theme::text());
        if let Some(key) = self.key.as_deref() {
            let token = format!("[{key}]");
            let kx = area.x + 5 + self.label_w;
            // Dropped when it does not fit; the Label is never cut for it.
            if kx + crate::cast!(token.chars().count() => u16) <= area.right() {
                buf.set_stringn(kx, area.y, &token, crate::cast!(area.right() - kx => usize), theme::key());
            }
        }
    }
}
