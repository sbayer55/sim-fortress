//! Vertically scrollable panel bodies.
//!
//! A body is drawn row-by-row into a tall off-screen [`Buffer`] whose `x`/`y`
//! match the real inner area, so absolute-coordinate drawing keeps working;
//! the window at `offset` is then copied onto the frame and an overflow
//! indicator (`↑n ↓m`) is written into the panel's bottom border.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::Frame;

use crate::{glyphs, theme};

/// Height of the off-screen canvas: the hard cap on rows a body may draw.
pub const CANVAS_H: u16 = 160;

/// What the blit measured: rows the body used and the offset actually shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Overflow {
    pub content: u16,
    pub offset: u16,
}

impl Overflow {
    /// Largest offset that still shows a full window of `visible` rows.
    pub const fn max_offset(content: u16, visible: u16) -> u16 {
        content.saturating_sub(visible)
    }
}

/// Draw `body` off-screen and blit the window at `offset` into `inner`.
///
/// The canvas is as wide as `inner` and [`CANVAS_H`] tall; `offset` is clamped
/// to the content. `body` returns the number of rows it used. `panel_area` is
/// the bordered panel around `inner`, whose bottom border receives the
/// indicator.
pub fn draw(f: &mut Frame<'_>, panel_area: Rect, inner: Rect, offset: u16, body: impl FnOnce(&mut Buffer, Rect) -> u16) -> Overflow {
    let canvas = Rect::new(inner.x, inner.y, inner.width, CANVAS_H);
    let mut buf = Buffer::empty(canvas);
    super::util::fill(&mut buf, canvas, Style::default().bg(theme::PANEL_BG));
    let content = body(&mut buf, canvas).min(CANVAS_H);
    let max = Overflow::max_offset(content, inner.height);
    let offset = offset.min(max);

    let target = f.buffer_mut();
    for y in 0..inner.height {
        for x in 0..inner.width {
            let src = buf.cell((inner.x + x, inner.y + y + offset)).cloned();
            if let (Some(src), Some(dst)) = (src, target.cell_mut((inner.x + x, inner.y + y))) {
                *dst = src;
            }
        }
    }

    if let Some(text) = foot(offset, content, inner.height) {
        super::panel::draw_foot(target, panel_area, &text);
    }
    Overflow { content, offset }
}

/// The Panel Foot for a window of `visible` rows at `offset` over `content`
/// rows: `↑n ↓m`, a zero part omitted, `None` when everything fits.
pub fn foot(offset: u16, content: u16, visible: u16) -> Option<String> {
    if content <= visible {
        return None;
    }
    let below = Overflow::max_offset(content, visible).saturating_sub(offset);
    let mut parts = Vec::new();
    if offset > 0 {
        parts.push(format!("{}{offset}", glyphs::UP));
    }
    if below > 0 {
        parts.push(format!("{}{below}", glyphs::DOWN));
    }
    Some(parts.join(" "))
}
