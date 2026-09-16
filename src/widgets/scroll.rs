//! Vertically scrollable panel bodies.
//!
//! A body is drawn row-by-row into a tall off-screen [`Buffer`] whose `x`/`y`
//! match the real inner area, so absolute-coordinate drawing keeps working;
//! the window at `offset` is then copied onto the frame and an overflow
//! indicator (`↑n ↓m`) is written into the panel's bottom border.

use std::fmt::Write as _;

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

    if content > inner.height {
        indicator(target, panel_area, offset, max - offset);
    }
    Overflow { content, offset }
}

/// ` ↑3 ↓12 ` right-aligned in the panel's bottom border.
fn indicator(buf: &mut Buffer, panel_area: Rect, above: u16, below: u16) {
    let mut text = String::from(" ");
    if above > 0 {
        let _ = write!(text, "{}{above} ", glyphs::UP);
    }
    if below > 0 {
        let _ = write!(text, "{}{below} ", glyphs::DOWN);
    }
    let w = crate::cast!(text.chars().count() => u16);
    if panel_area.height < 2 || panel_area.width < w + 2 {
        return;
    }
    let x = panel_area.right() - 1 - w;
    let y = panel_area.bottom() - 1;
    buf.set_stringn(x, y, &text, crate::cast!(w => usize), theme::dim_text());
}
