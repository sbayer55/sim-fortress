//! A one-row rule with an optional label: `─ Clock ───`. See `docs/components/divider.md`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::component::Component;
use crate::{glyphs, theme};

/// A horizontal rule spanning its row, with ` Text ` from column 1.
#[derive(Clone, Debug)]
pub struct Divider<'a> {
    text: Option<Cow<'a, str>>,
}

impl<'a> Divider<'a> {
    pub fn new(text: impl Into<Cow<'a, str>>) -> Self {
        Self { text: Some(text.into()) }
    }

    /// Rule only, no text.
    pub const fn rule() -> Self {
        Self { text: None }
    }
}

impl Component for Divider<'_> {
    fn height(&self, _width: u16) -> u16 {
        1
    }

    fn min_width(&self) -> u16 {
        3
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let w = crate::cast!(area.width => usize);
        let rule: String = std::iter::repeat_n(glyphs::H_LINE, w).collect();
        buf.set_stringn(area.x, area.y, &rule, w, theme::border());
        if let Some(text) = &self.text {
            // Cut at width − 2 so the first and last cell stay rule glyphs.
            buf.set_stringn(area.x + 1, area.y, format!(" {text} "), w.saturating_sub(2), theme::label());
        }
    }
}
