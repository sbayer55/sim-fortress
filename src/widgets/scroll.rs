//! Vertically scrollable panel bodies. See `docs/components/scroll-region.md`.
//!
//! A body is drawn into a tall off-screen [`Buffer`] whose `x`/`y` match the
//! real inner area, so absolute-coordinate drawing keeps working; the window
//! at `offset` is then copied onto the target buffer. The Panel around the
//! region shows `↑n ↓m` in its Foot.

use std::fmt;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::Frame;

use super::component::Component;
use crate::{glyphs, theme};

#[cfg(test)]
mod tests;

/// Height of the off-screen canvas: the hard cap on rows a body may draw.
pub const CANVAS_H: u16 = 160;

/// What the blit measured: rows the body used, the offset actually shown and
/// the rows the window can show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Overflow {
    pub content: u16,
    pub offset: u16,
    pub visible: u16,
}

impl Overflow {
    /// Largest offset that still shows a full window of `visible` rows.
    pub const fn max_offset(content: u16, visible: u16) -> u16 {
        content.saturating_sub(visible)
    }

    /// The Panel Foot: `↑n ↓m`, a zero part omitted, `None` when everything fits.
    pub fn foot(self) -> Option<String> {
        foot(self.offset, self.content, self.visible)
    }
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

/// A body drawn off-screen and shown through a window at `offset`.
pub struct ScrollRegion<'a> {
    body: &'a dyn Component,
    offset: u16,
}

impl fmt::Debug for ScrollRegion<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScrollRegion").field("offset", &self.offset).finish()
    }
}

impl<'a> ScrollRegion<'a> {
    /// `body` is normally a `VStack`; its `height(width)` is the content height.
    pub const fn new(body: &'a dyn Component) -> Self {
        Self { body, offset: 0 }
    }

    #[must_use]
    pub const fn offset(mut self, offset: u16) -> Self {
        self.offset = offset;
        self
    }

    /// What a render into `inner` would show, without drawing: the content
    /// height and the clamped offset. Lets the caller set `Panel::foot`
    /// before the panel is drawn.
    pub fn overflow(&self, inner: Rect) -> Overflow {
        let content = self.body.height(inner.width).min(CANVAS_H);
        let offset = self.offset.min(Overflow::max_offset(content, inner.height));
        Overflow { content, offset, visible: inner.height }
    }

    /// Draw the body off-screen and blit the window into `inner`.
    pub fn render(&self, buf: &mut Buffer, inner: Rect) -> Overflow {
        let ov = self.overflow(inner);
        let canvas = Rect::new(inner.x, inner.y, inner.width, ov.content.max(inner.height));
        let mut off = Buffer::empty(canvas);
        super::util::fill(&mut off, canvas, Style::default().bg(theme::PANEL_BG));
        self.body.render(&mut off, Rect { height: ov.content, ..canvas });
        blit(buf, &off, inner, ov.offset);
        ov
    }
}

impl Component for ScrollRegion<'_> {
    /// The content height, capped at `CANVAS_H`; give it less and it scrolls.
    fn height(&self, width: u16) -> u16 {
        self.body.height(width).min(CANVAS_H)
    }

    fn min_width(&self) -> u16 {
        self.body.min_width()
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        Self::render(self, buf, area);
    }
}

/// Copy canvas rows `offset..` into `inner`.
fn blit(target: &mut Buffer, canvas: &Buffer, inner: Rect, offset: u16) {
    for y in 0..inner.height {
        for x in 0..inner.width {
            let src = canvas.cell((inner.x + x, inner.y + y + offset)).cloned();
            if let (Some(src), Some(dst)) = (src, target.cell_mut((inner.x + x, inner.y + y))) {
                *dst = src;
            }
        }
    }
}

/// Draw `body` off-screen and blit the window at `offset` into `inner`.
///
/// The closure form: `body` draws into a canvas `CANVAS_H` tall and returns
/// the number of rows it used, so the offset can only be clamped after
/// drawing. `panel_area` is the bordered panel around `inner`, whose bottom
/// border receives the Foot. Screens move to [`ScrollRegion`] over a stack.
pub fn draw(f: &mut Frame<'_>, panel_area: Rect, inner: Rect, offset: u16, body: impl FnOnce(&mut Buffer, Rect) -> u16) -> Overflow {
    let canvas = Rect::new(inner.x, inner.y, inner.width, CANVAS_H);
    let mut buf = Buffer::empty(canvas);
    super::util::fill(&mut buf, canvas, Style::default().bg(theme::PANEL_BG));
    let content = body(&mut buf, canvas).min(CANVAS_H);
    let offset = offset.min(Overflow::max_offset(content, inner.height));
    let target = f.buffer_mut();
    blit(target, &buf, inner, offset);
    let ov = Overflow { content, offset, visible: inner.height };
    if let Some(text) = ov.foot() {
        super::panel::draw_foot(target, panel_area, &text);
    }
    ov
}
