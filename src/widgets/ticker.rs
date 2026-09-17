//! The one-line latest-event row under the map. See `docs/components/ticker.md`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::component::Component;
use crate::theme;

/// ` <Glyph> <Text>   (e: full log)` on `theme::BG`; a blank row for `None`.
#[derive(Clone, Debug)]
pub struct Ticker<'a> {
    event: Option<(char, Color, Cow<'a, str>)>,
    pointer: Cow<'a, str>,
}

impl<'a> Ticker<'a> {
    /// The event as `(glyph, colour, text)`; the caller resolves the kind.
    pub fn new(event: Option<(char, Color, &'a str)>) -> Self {
        Self { event: event.map(|(g, c, t)| (g, c, Cow::Borrowed(t))), pointer: Cow::Borrowed("(e: full log)") }
    }

    #[must_use]
    pub fn pointer(mut self, pointer: impl Into<Cow<'a, str>>) -> Self {
        self.pointer = pointer.into();
        self
    }
}

impl Component for Ticker<'_> {
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
        let row = Rect { height: 1, ..area };
        super::util::fill(buf, row, Style::default().bg(theme::BG));
        let Some((glyph, color, text)) = &self.event else { return };
        let w = crate::cast!(area.width => usize);
        buf.set_stringn(area.x, area.y, format!(" {glyph} "), w, Style::default().fg(*color).bg(theme::BG).add_modifier(Modifier::BOLD));
        if area.width < 3 {
            return;
        }
        buf.set_stringn(area.x + 3, area.y, text.as_ref(), w - 3, Style::default().fg(theme::TEXT).bg(theme::BG));
        // The Pointer goes before Text is cut: only drawn when both fit whole.
        let text_w = text.chars().count();
        let pointer = format!("   {}", self.pointer);
        if 3 + text_w + pointer.chars().count() <= w {
            let px = area.x + 3 + crate::cast!(text_w => u16);
            buf.set_stringn(px, area.y, &pointer, w - 3 - text_w, Style::default().fg(theme::DIM).bg(theme::BG));
        }
    }
}
