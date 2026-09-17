//! Bordered panel: double-line outer panels, single-line inner splits.
//! See `docs/components/panel.md`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Borders, Widget};
use ratatui::Frame;

use super::component::Component;
use super::divider::Divider;
use crate::theme;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq)]
#[derive(Debug)]
pub enum Kind {
    Outer,
    Inner,
    Focus,
}

/// A titled box that clears its area and hands back the inner rectangle.
#[derive(Clone, Debug)]
pub struct Panel<'a> {
    title: Option<Cow<'a, str>>,
    info: Option<Cow<'a, str>>,
    foot: Option<Cow<'a, str>>,
    kind: Kind,
}

impl<'a> Panel<'a> {
    pub fn new(title: impl Into<Cow<'a, str>>) -> Self {
        Self { title: Some(title.into()), info: None, foot: None, kind: Kind::Outer }
    }

    /// No Title: a continuous top edge.
    pub const fn untitled() -> Self {
        Self { title: None, info: None, foot: None, kind: Kind::Outer }
    }

    #[must_use]
    pub const fn kind(mut self, kind: Kind) -> Self {
        self.kind = kind;
        self
    }

    /// Dim, right-aligned text in the top edge. Dropped before Title is cut.
    #[must_use]
    pub fn info(mut self, info: impl Into<Cow<'a, str>>) -> Self {
        let info = info.into();
        self.info = if info.is_empty() { None } else { Some(info) };
        self
    }

    /// Dim, right-aligned text in the bottom edge; normally `↑n ↓m` from a Scroll Region.
    #[must_use]
    pub fn foot(mut self, foot: impl Into<Cow<'a, str>>) -> Self {
        let foot = foot.into();
        self.foot = if foot.is_empty() { None } else { Some(foot) };
        self
    }

    /// The content area a panel drawn at `area` leaves: `(w−2) × (h−2)` inside the border.
    pub fn inner(area: Rect) -> Rect {
        Block::default().borders(Borders::ALL).inner(area)
    }

    /// Fill, border, Title, Info and Foot; returns the inner area.
    pub fn render(&self, buf: &mut Buffer, area: Rect) -> Rect {
        let (border_type, border_style) = match self.kind {
            Kind::Outer => (BorderType::Double, theme::border()),
            Kind::Inner => (BorderType::Plain, theme::border()),
            Kind::Focus => (BorderType::Double, theme::border_focus()),
        };
        super::util::fill(buf, area, Style::default().bg(theme::PANEL_BG));
        Block::default().borders(Borders::ALL).border_type(border_type).border_style(border_style).render(area, buf);
        if area.height == 0 || area.width < 3 {
            return Self::inner(area);
        }
        let edge = crate::cast!(area.width - 2 => usize); // cells between the corners
        let title = self.title.as_deref().map(|t| format!(" {t} ")).unwrap_or_default();
        let title_w = title.chars().count().min(edge);
        buf.set_stringn(area.x + 1, area.y, &title, edge, border_style);
        let text = title.trim();
        buf.set_stringn(area.x + 2, area.y, text, edge.saturating_sub(1), theme::title());
        if let Some(info) = self.info.as_deref() {
            let info = format!(" {info} ");
            let info_w = info.chars().count();
            if title_w + info_w <= edge {
                let x = area.right() - 1 - crate::cast!(info_w => u16);
                buf.set_stringn(x, area.y, &info, info_w, border_style);
                buf.set_stringn(x + 1, area.y, info.trim(), info_w - 2, theme::dim_text());
            }
        }
        if let Some(foot) = self.foot.as_deref() {
            draw_foot(buf, area, foot);
        }
        Self::inner(area)
    }
}

impl Component for Panel<'_> {
    /// The two border rows; a panel is sized by its container (`Fixed` or `Fill`).
    fn height(&self, _width: u16) -> u16 {
        2
    }

    fn min_width(&self) -> u16 {
        3
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        Self::render(self, buf, area);
    }
}

/// ` text ` right-aligned in the bottom edge of `panel_area`, one border cell
/// kept after it; dropped when the edge is narrower than the text plus 2.
pub fn draw_foot(buf: &mut Buffer, panel_area: Rect, text: &str) {
    let text = format!(" {text} ");
    let w = crate::cast!(text.chars().count() => u16);
    if panel_area.height < 2 || panel_area.width < w + 2 {
        return;
    }
    let x = panel_area.right() - 1 - w;
    let y = panel_area.bottom() - 1;
    buf.set_stringn(x, y, &text, crate::cast!(w => usize), theme::dim_text());
}

/// Draw a titled panel and return the inner area.
pub fn draw(f: &mut Frame<'_>, area: Rect, title: &str, kind: Kind) -> Rect {
    draw_with_hint_in(f.buffer_mut(), area, title, "", kind)
}

/// [`draw`] straight into a buffer (for off-screen canvases).
pub fn draw_in(buf: &mut Buffer, area: Rect, title: &str, kind: Kind) -> Rect {
    draw_with_hint_in(buf, area, title, "", kind)
}

/// Draw a titled panel with a right-aligned hint in the top border.
pub fn draw_with_hint(f: &mut Frame<'_>, area: Rect, title: &str, hint: &str, kind: Kind) -> Rect {
    draw_with_hint_in(f.buffer_mut(), area, title, hint, kind)
}

/// [`draw_with_hint`] straight into a buffer. Thin wrapper over [`Panel`].
pub fn draw_with_hint_in(buf: &mut Buffer, area: Rect, title: &str, hint: &str, kind: Kind) -> Rect {
    let panel = if title.is_empty() { Panel::untitled() } else { Panel::new(title) };
    panel.kind(kind).info(hint).render(buf, area)
}

/// A single-row section title inside a panel: `── Title ────`. Thin wrapper over [`Divider`].
pub fn section(f: &mut Frame<'_>, area: Rect, row: u16, title: &str) {
    section_in(f.buffer_mut(), area, row, title);
}

/// [`section`] straight into a buffer.
pub fn section_in(buf: &mut Buffer, area: Rect, row: u16, title: &str) {
    if row >= area.height {
        return;
    }
    Divider::new(title).render(buf, Rect::new(area.x, area.y + row, area.width, 1));
}
