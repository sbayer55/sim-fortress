//! A shared column spec that a block of rows draws against, so cells line up
//! down the block. See `docs/components/columns.md`.

use std::fmt;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::component::{distribute, Align, Component, Constraint};
use super::text::Text;
use crate::theme;

#[cfg(test)]
mod tests;

/// One column: its width constraint, default cell alignment and header title.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column {
    pub title: &'static str,
    pub width: Constraint,
    pub align: Align,
}

impl Column {
    pub const fn new(width: Constraint) -> Self {
        Self { title: "", width, align: Align::Left }
    }

    pub const fn titled(title: &'static str, width: Constraint) -> Self {
        Self { title, width, align: Align::Left }
    }

    #[must_use]
    pub const fn right(mut self) -> Self {
        self.align = Align::Right;
        self
    }

    #[must_use]
    pub const fn left(mut self) -> Self {
        self.align = Align::Left;
        self
    }
}

impl From<Constraint> for Column {
    fn from(width: Constraint) -> Self {
        Self::new(width)
    }
}

/// The resolved spec: one `(x offset, width)` per column that fits.
pub type Resolved = Vec<(u16, u16)>;

/// A column spec, resolved once per block before any row is drawn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Columns {
    cols: Vec<Column>,
}

impl Columns {
    pub fn new<C: Into<Column> + Copy>(cols: &[C]) -> Self {
        Self { cols: cols.iter().map(|&c| c.into()).collect() }
    }

    /// Set the default alignment of column `i`.
    #[must_use]
    pub fn align(mut self, i: usize, align: Align) -> Self {
        if let Some(c) = self.cols.get_mut(i) {
            c.align = align;
        }
        self
    }

    pub fn get(&self, i: usize) -> Option<&Column> {
        self.cols.get(i)
    }

    pub fn len(&self) -> usize {
        self.cols.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cols.is_empty()
    }

    /// The sum of the `Fixed` widths and the `Min` floors.
    pub fn min_width(&self) -> u16 {
        self.cols
            .iter()
            .map(|c| match c.width {
                Constraint::Fixed(n) | Constraint::Min(n) => n,
                Constraint::Auto | Constraint::Fill(_) => 0,
            })
            .fold(0, u16::saturating_add)
    }

    /// Resolve against `width`. `measured(i)` is the widest cell in column
    /// `i` over every row, used by `Min` columns. Whole trailing columns that
    /// do not fit are dropped, never squeezed.
    pub fn resolve(&self, x: u16, width: u16, measured: impl Fn(usize) -> u16) -> Resolved {
        let kinds: Vec<Constraint> = self.cols.iter().map(|c| c.width).collect();
        let natural: Vec<u16> = (0..self.cols.len()).map(&measured).collect();
        let mut out = Vec::with_capacity(kinds.len());
        let mut cx = x;
        for w in distribute(&kinds, &natural, width, 0) {
            if cx.saturating_add(w) > x.saturating_add(width) {
                break;
            }
            out.push((cx, w));
            cx = cx.saturating_add(w);
        }
        out
    }
}

/// One cell of a row.
pub enum Cell<'a> {
    /// Styled text; takes the column's alignment unless it sets its own.
    Text(Text<'a>),
    /// Any component, drawn in the cell's area.
    Widget(Box<dyn Component + 'a>),
    Blank,
}

impl fmt::Debug for Cell<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(t) => f.debug_tuple("Text").field(t).finish(),
            Self::Widget(_) => f.write_str("Widget"),
            Self::Blank => f.write_str("Blank"),
        }
    }
}

impl Cell<'_> {
    pub fn min_width(&self) -> u16 {
        match self {
            Self::Text(t) => t.min_width(),
            Self::Widget(w) => w.min_width(),
            Self::Blank => 0,
        }
    }

    fn render(&self, buf: &mut Buffer, area: Rect, align: Align) {
        match self {
            Self::Text(t) => t.clone().or_align(align).render(buf, area),
            Self::Widget(w) => w.render(buf, area),
            Self::Blank => {}
        }
    }
}

impl<'a> From<Text<'a>> for Cell<'a> {
    fn from(t: Text<'a>) -> Self {
        Self::Text(t)
    }
}

impl<'a> From<Box<dyn Component + 'a>> for Cell<'a> {
    fn from(w: Box<dyn Component + 'a>) -> Self {
        Self::Widget(w)
    }
}

macro_rules! widget_cells {
    ($($t:ty),* $(,)?) => {$(
        impl<'a> From<$t> for Cell<'a> {
            fn from(w: $t) -> Self {
                Self::Widget(Box::new(w))
            }
        }
    )*};
}
widget_cells!(
    super::stack::Spacer,
    super::bars::Bar,
    super::bars::LabeledBar<'a>,
    super::bars::RangeBar,
    super::bars::Sparkline,
    super::trend::TrendArrow,
    super::stack::HStack<'a>,
    super::field::Stepper<'a>,
    super::field::TextField<'a>,
    super::checkbox::Checkbox<'a>,
    super::key_hint::KeyHint<'a>,
);

/// A row of cells, or one component spanning the width.
pub struct Row<'a> {
    cells: Vec<Cell<'a>>,
    span: Option<Box<dyn Component + 'a>>,
}

impl fmt::Debug for Row<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Row").field("cells", &self.cells).field("span", &self.span.is_some()).finish()
    }
}

impl Default for Row<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Row<'a> {
    pub const fn new() -> Self {
        Self { cells: Vec::new(), span: None }
    }

    #[must_use]
    pub fn cell(mut self, cell: impl Into<Cell<'a>>) -> Self {
        self.cells.push(cell.into());
        self
    }

    /// A row that ignores the columns: a Divider, a totals line, a note.
    pub fn span(component: impl Component + 'a) -> Self {
        Self { cells: Vec::new(), span: Some(Box::new(component)) }
    }

    pub fn cells(&self) -> &[Cell<'a>] {
        &self.cells
    }
}

/// Rows drawn against one `Columns` spec; `Min` columns are measured over
/// every row first. One row may be selected and takes `theme::selected()`
/// across the whole width.
pub struct Block<'a> {
    cols: Columns,
    rows: Vec<Row<'a>>,
    selected: Option<usize>,
}

impl fmt::Debug for Block<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Block").field("cols", &self.cols).field("rows", &self.rows.len()).field("selected", &self.selected).finish()
    }
}

impl<'a> Block<'a> {
    pub const fn new(cols: Columns) -> Self {
        Self { cols, rows: Vec::new(), selected: None }
    }

    #[must_use]
    pub fn rows(mut self, rows: impl IntoIterator<Item = Row<'a>>) -> Self {
        self.rows.extend(rows);
        self
    }

    #[must_use]
    pub const fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    fn resolve(&self, area: Rect) -> Resolved {
        self.cols.resolve(area.x, area.width, |i| widest(self.rows.iter(), i))
    }
}

/// The widest cell `i` over `rows` (span rows are skipped).
pub fn widest<'r, 'a: 'r>(rows: impl Iterator<Item = &'r Row<'a>>, i: usize) -> u16 {
    rows.filter_map(|r| r.cells.get(i)).map(Cell::min_width).max().unwrap_or(0)
}

/// Draw one row's cells at the resolved offsets.
pub fn draw_row(buf: &mut Buffer, y: u16, area: Rect, cols: &Columns, resolved: &[(u16, u16)], cells: &[Cell<'_>]) {
    for ((i, cell), &(x, w)) in cells.iter().enumerate().zip(resolved) {
        let align = cols.get(i).map_or(Align::Left, |c| c.align);
        let slot = Rect::new(x, y, w, 1).intersection(area);
        if !slot.is_empty() {
            cell.render(buf, slot, align);
        }
    }
}

impl Component for Block<'_> {
    fn height(&self, _width: u16) -> u16 {
        crate::cast!(self.rows.len() => u16)
    }

    fn min_width(&self) -> u16 {
        self.cols.min_width()
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        let resolved = self.resolve(area);
        for (i, row) in self.rows.iter().enumerate() {
            let y = area.y.saturating_add(crate::cast!(i => u16));
            if y >= area.bottom() {
                break;
            }
            let line = Rect::new(area.x, y, area.width, 1);
            if self.selected == Some(i) {
                super::util::fill(buf, line, theme::selected());
            }
            match &row.span {
                Some(c) => c.render(buf, line),
                None => draw_row(buf, y, area, &self.cols, &resolved, &row.cells),
            }
        }
    }
}
