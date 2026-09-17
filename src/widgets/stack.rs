//! Vertical and horizontal stacks, and the blank `Spacer` between children.
//! See `docs/components/vstack.md`, `hstack.md` and `spacer.md`.

use std::fmt;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::component::{distribute, Component, Constraint};

#[cfg(test)]
mod tests;

/// Children top to bottom, each as tall as its constraint says.
pub struct VStack<'a> {
    children: Vec<(Constraint, &'a dyn Component)>,
    gap: u16,
}

impl fmt::Debug for VStack<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VStack").field("children", &self.children.len()).field("gap", &self.gap).finish()
    }
}

impl Default for VStack<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> VStack<'a> {
    pub const fn new() -> Self {
        Self { children: Vec::new(), gap: 0 }
    }

    /// Every box as an `Auto` child, in order.
    pub fn from_boxes(boxes: &'a [Box<dyn Component + 'a>]) -> Self {
        boxes.iter().fold(Self::new(), |s, b| s.child(b.as_ref()))
    }

    /// An `Auto` child: as tall as `child.height(width)`.
    #[must_use]
    pub fn child(self, child: &'a dyn Component) -> Self {
        self.child_with(Constraint::Auto, child)
    }

    #[must_use]
    pub fn child_with(mut self, constraint: Constraint, child: &'a dyn Component) -> Self {
        self.children.push((constraint, child));
        self
    }

    /// Blank rows between children (default 0).
    #[must_use]
    pub const fn gap(mut self, rows: u16) -> Self {
        self.gap = rows;
        self
    }

    fn heights(&self, area: Rect) -> Vec<u16> {
        let kinds: Vec<Constraint> = self.children.iter().map(|(k, _)| *k).collect();
        let natural: Vec<u16> = self.children.iter().map(|(_, c)| c.height(area.width)).collect();
        distribute(&kinds, &natural, area.height, self.gap)
    }

    fn gaps(&self) -> u16 {
        self.gap.saturating_mul(crate::cast!(self.children.len().saturating_sub(1) => u16))
    }
}

impl Component for VStack<'_> {
    /// The content height: `Auto` and `Fixed` children plus gaps. A `Fill`
    /// child counts 0 here; it takes whatever the area leaves at render time.
    fn height(&self, width: u16) -> u16 {
        self.children
            .iter()
            .map(|(k, c)| match *k {
                Constraint::Fixed(h) => h,
                Constraint::Auto | Constraint::Min(_) => c.height(width),
                Constraint::Fill(_) => 0,
            })
            .fold(self.gaps(), u16::saturating_add)
    }

    fn min_width(&self) -> u16 {
        self.children.iter().map(|(_, c)| c.min_width()).max().unwrap_or(0)
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        let mut y = area.y;
        for ((_, child), h) in self.children.iter().zip(self.heights(area)) {
            if y >= area.bottom() {
                break;
            }
            let slot = Rect::new(area.x, y, area.width, h).intersection(area);
            if !slot.is_empty() {
                child.render(buf, slot);
            }
            y = y.saturating_add(h).saturating_add(self.gap);
        }
    }
}

/// Children left to right, each as wide as its constraint says.
pub struct HStack<'a> {
    children: Vec<(Constraint, &'a dyn Component)>,
    gap: u16,
}

impl fmt::Debug for HStack<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HStack").field("children", &self.children.len()).field("gap", &self.gap).finish()
    }
}

impl Default for HStack<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> HStack<'a> {
    pub const fn new() -> Self {
        Self { children: Vec::new(), gap: 0 }
    }

    /// An `Auto` child: as wide as `child.min_width()`.
    #[must_use]
    pub fn child(self, child: &'a dyn Component) -> Self {
        self.child_with(Constraint::Auto, child)
    }

    #[must_use]
    pub fn child_with(mut self, constraint: Constraint, child: &'a dyn Component) -> Self {
        self.children.push((constraint, child));
        self
    }

    /// Blank columns between children (default 0).
    #[must_use]
    pub const fn gap(mut self, cols: u16) -> Self {
        self.gap = cols;
        self
    }

    fn widths(&self, total: u16) -> Vec<u16> {
        let kinds: Vec<Constraint> = self.children.iter().map(|(k, _)| *k).collect();
        let natural: Vec<u16> = self.children.iter().map(|(_, c)| c.min_width()).collect();
        distribute(&kinds, &natural, total, self.gap)
    }

    /// The area each child would get, without drawing. Children that fall
    /// past the right edge get an empty rect.
    pub fn areas(&self, area: Rect) -> Vec<Rect> {
        let mut x = area.x;
        self.widths(area.width)
            .into_iter()
            .map(|w| {
                let slot = Rect::new(x, area.y, w, area.height).intersection(area);
                x = x.saturating_add(w).saturating_add(self.gap);
                slot
            })
            .collect()
    }
}

impl Component for HStack<'_> {
    /// The tallest child at the width it would get.
    fn height(&self, width: u16) -> u16 {
        self.children.iter().zip(self.widths(width)).map(|((_, c), w)| c.height(w)).max().unwrap_or(0)
    }

    /// `Auto` and `Fixed` widths plus gaps; `Fill` children count 0.
    fn min_width(&self) -> u16 {
        let gaps = self.gap.saturating_mul(crate::cast!(self.children.len().saturating_sub(1) => u16));
        self.children
            .iter()
            .map(|(k, c)| match *k {
                Constraint::Fixed(w) => w,
                Constraint::Auto => c.min_width(),
                Constraint::Min(n) => c.min_width().max(n),
                Constraint::Fill(_) => 0,
            })
            .fold(gaps, u16::saturating_add)
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        for ((_, child), slot) in self.children.iter().zip(self.areas(area)) {
            if slot.is_empty() {
                continue;
            }
            child.render(buf, slot);
        }
    }
}

/// A blank block: `n` rows tall in a `VStack`, `n` columns wide in an `HStack`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spacer {
    rows: u16,
    cols: u16,
}

impl Spacer {
    pub const fn rows(n: u16) -> Self {
        Self { rows: n, cols: 0 }
    }

    pub const fn cols(n: u16) -> Self {
        Self { rows: 0, cols: n }
    }
}

impl Component for Spacer {
    fn height(&self, _width: u16) -> u16 {
        self.rows
    }

    fn min_width(&self) -> u16 {
        self.cols
    }

    fn render(&self, _buf: &mut Buffer, _area: Rect) {}
}
