//! Rows of aligned columns under a dim header, with a marker column, one
//! selected row and an optional totals row. See `docs/components/table.md`.

use std::borrow::Cow;
use std::fmt;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::columns::{Cell, Column, Columns, Resolved};
use super::component::{Align, Component};
use super::text::Text;
use crate::{glyphs, theme};

#[cfg(test)]
mod tests;

/// One table cell. Text cells take the row's style; the others keep their own.
pub enum TableCell<'a> {
    /// Plain text in the row style.
    Text(Cow<'a, str>),
    /// Own foreground; keeps the row background.
    Styled(Cow<'a, str>, Color),
    /// Secondary text in `theme::dim_text()`.
    Dim(Cow<'a, str>),
    /// A species glyph, bold in the roster colour.
    Glyph(char, Color),
    /// A Bare Bar, Sparkline, Trend Arrow or any other component.
    Widget(Box<dyn Component + 'a>),
    Blank,
}

impl fmt::Debug for TableCell<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(t) => f.debug_tuple("Text").field(t).finish(),
            Self::Styled(t, c) => f.debug_tuple("Styled").field(t).field(c).finish(),
            Self::Dim(t) => f.debug_tuple("Dim").field(t).finish(),
            Self::Glyph(g, c) => f.debug_tuple("Glyph").field(g).field(c).finish(),
            Self::Widget(_) => f.write_str("Widget"),
            Self::Blank => f.write_str("Blank"),
        }
    }
}

impl<'a> TableCell<'a> {
    pub fn text(t: impl Into<Cow<'a, str>>) -> Self {
        Self::Text(t.into())
    }

    pub fn styled(t: impl Into<Cow<'a, str>>, color: Color) -> Self {
        Self::Styled(t.into(), color)
    }

    pub fn dim(t: impl Into<Cow<'a, str>>) -> Self {
        Self::Dim(t.into())
    }

    pub fn widget(w: impl Component + 'a) -> Self {
        Self::Widget(Box::new(w))
    }

    fn min_width(&self) -> u16 {
        match self {
            Self::Text(t) | Self::Styled(t, _) | Self::Dim(t) => crate::cast!(t.chars().count() => u16),
            Self::Glyph(..) => 1,
            Self::Widget(w) => w.min_width(),
            Self::Blank => 0,
        }
    }

    /// The cell as a column cell under `style` (the row's text style and background).
    fn resolve(&self, style: RowStyle) -> Cell<'_> {
        match self {
            Self::Text(t) => Text::new(t.as_ref()).style(style.text).into(),
            Self::Styled(t, c) => Text::new(t.as_ref()).style(Style::default().fg(*c).bg(style.bg)).into(),
            Self::Dim(t) => Text::new(t.as_ref()).style(style.dim).into(),
            Self::Glyph(g, c) => Text::new(g.to_string()).style(Style::default().fg(*c).bg(style.bg).add_modifier(Modifier::BOLD)).into(),
            Self::Widget(w) => Cell::Widget(Box::new(w)),
            Self::Blank => Cell::Blank,
        }
    }
}

#[derive(Clone, Copy)]
struct RowStyle {
    text: Style,
    dim: Style,
    bg: Color,
}

impl RowStyle {
    fn of(selected: bool, absent: bool) -> Self {
        if selected {
            Self { text: theme::selected(), dim: theme::selected(), bg: theme::SELECT_BG }
        } else if absent {
            Self { text: theme::dim_text(), dim: theme::dim_text(), bg: theme::PANEL_BG }
        } else {
            Self { text: theme::text(), dim: theme::dim_text(), bg: theme::PANEL_BG }
        }
    }
}

/// One row: its cells under the columns, and optionally a tail that starts
/// at the column after the last cell and runs to the right edge.
pub struct TableRow<'a> {
    pub cells: Vec<TableCell<'a>>,
    pub absent: bool,
    tail: Option<Text<'a>>,
}

impl fmt::Debug for TableRow<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TableRow").field("cells", &self.cells).field("absent", &self.absent).field("tail", &self.tail).finish()
    }
}

impl<'a> TableRow<'a> {
    pub fn new(cells: impl IntoIterator<Item = TableCell<'a>>) -> Self {
        Self { cells: cells.into_iter().collect(), absent: false, tail: None }
    }

    /// A row whose subject is gone: drawn in `theme::dim_text()`.
    #[must_use]
    pub const fn absent(mut self, absent: bool) -> Self {
        self.absent = absent;
        self
    }

    /// Free text after the last cell, spanning the remaining columns.
    #[must_use]
    pub fn tail(mut self, text: Text<'a>) -> Self {
        self.tail = Some(text);
        self
    }
}

/// Rows on demand, so a screen never builds every row to draw a window.
pub trait RowSource {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Hand row `i` to `f`.
    fn with_row(&self, i: usize, f: &mut dyn FnMut(&TableRow<'_>));
}

impl RowSource for [TableRow<'_>] {
    fn len(&self) -> usize {
        self.len()
    }

    fn with_row(&self, i: usize, f: &mut dyn FnMut(&TableRow<'_>)) {
        if let Some(r) = self.get(i) {
            f(r);
        }
    }
}

impl RowSource for Vec<TableRow<'_>> {
    fn len(&self) -> usize {
        self.len()
    }

    fn with_row(&self, i: usize, f: &mut dyn FnMut(&TableRow<'_>)) {
        self.as_slice().with_row(i, f);
    }
}

/// Where a table's rows come from: a slice, or a source that builds them on demand.
#[derive(Clone, Copy)]
enum Rows<'a> {
    Slice(&'a [TableRow<'a>]),
    Source(&'a dyn RowSource),
}

impl Rows<'_> {
    fn len(self) -> usize {
        match self {
            Self::Slice(s) => s.len(),
            Self::Source(s) => s.len(),
        }
    }

    fn with_row(self, i: usize, f: &mut dyn FnMut(&TableRow<'_>)) {
        match self {
            Self::Slice(s) => s.with_row(i, f),
            Self::Source(s) => s.with_row(i, f),
        }
    }
}

/// A header row, a Marker column, rows from a source, one selected row and
/// an optional totals row.
pub struct Table<'a> {
    columns: Columns,
    rows: Rows<'a>,
    selected: Option<usize>,
    totals: Option<TableRow<'a>>,
    spaced: bool,
}

impl fmt::Debug for Table<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Table").field("columns", &self.columns).field("rows", &self.rows.len()).field("selected", &self.selected).finish()
    }
}

impl<'a> Table<'a> {
    /// `columns` excludes the implicit one-cell Marker column.
    pub fn new(columns: &[Column], rows: &'a [TableRow<'a>]) -> Self {
        Self { columns: Columns::new(columns), rows: Rows::Slice(rows), selected: None, totals: None, spaced: false }
    }

    /// Rows built on demand by a [`RowSource`].
    pub fn from_source(columns: &[Column], rows: &'a dyn RowSource) -> Self {
        Self { columns: Columns::new(columns), rows: Rows::Source(rows), selected: None, totals: None, spaced: false }
    }

    #[must_use]
    pub const fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    #[must_use]
    pub fn totals(mut self, totals: TableRow<'a>) -> Self {
        self.totals = Some(totals);
        self
    }

    /// A blank row after the Header and one before Totals.
    #[must_use]
    pub const fn spaced(mut self, spaced: bool) -> Self {
        self.spaced = spaced;
        self
    }

    /// `sorted by <column> ↓`, for the Panel Info slot.
    pub fn sort_info(column: &str) -> String {
        format!("sorted by {column} {}", glyphs::DOWN)
    }

    fn resolve(&self, area: Rect) -> Resolved {
        self.columns.resolve(area.x + 1, area.width.saturating_sub(1), |i| {
            let mut w = 0u16;
            for r in 0..self.rows.len() {
                self.rows.with_row(r, &mut |row| w = w.max(row.cells.get(i).map_or(0, TableCell::min_width)));
            }
            w
        })
    }

    fn draw_header(&self, buf: &mut Buffer, area: Rect, resolved: &[(u16, u16)]) {
        for (i, &(x, w)) in resolved.iter().enumerate() {
            let Some(col) = self.columns.get(i) else { break };
            let slot = Rect::new(x, area.y, w, 1).intersection(area);
            Text::new(col.title).style(theme::dim_text()).align(col.align).render(buf, slot);
        }
    }

    fn draw_row(&self, buf: &mut Buffer, y: u16, area: Rect, resolved: &[(u16, u16)], row: &TableRow<'_>, selected: bool, totals: bool) {
        let line = Rect::new(area.x, y, area.width, 1);
        let style = RowStyle::of(selected, row.absent);
        if selected {
            super::util::fill(buf, line, theme::selected());
            let marker = Style::default().fg(theme::KEY).bg(theme::SELECT_BG).add_modifier(Modifier::BOLD);
            buf.set_stringn(area.x, y, glyphs::PLAY.to_string(), 1, marker);
        }
        let mut cells: Vec<Cell<'_>> = row.cells.iter().map(|c| c.resolve(style)).collect();
        if totals {
            // The label is the first text cell.
            if let Some((i, TableCell::Text(t))) = row.cells.iter().enumerate().find(|(_, c)| matches!(c, TableCell::Text(_))) {
                if let Some(slot) = cells.get_mut(i) {
                    *slot = Text::new(t.as_ref()).style(theme::label()).into();
                }
            }
        }
        super::columns::draw_row(buf, y, area, &self.columns, resolved, &cells);
        if let (Some(tail), Some(&(x, _))) = (&row.tail, resolved.get(row.cells.len())) {
            let slot = Rect::new(x, y, area.right().saturating_sub(x), 1).intersection(area);
            tail.clone().or_align(Align::Left).render(buf, slot);
        }
    }
}

impl Component for Table<'_> {
    fn height(&self, _width: u16) -> u16 {
        let rows = crate::cast!(self.rows.len() => u16);
        let spaced = u16::from(self.spaced);
        1 + spaced + rows + self.totals.as_ref().map_or(0, |_| 1 + spaced)
    }

    fn min_width(&self) -> u16 {
        1 + self.columns.get(0).map_or(0, |c| match c.width {
            super::component::Constraint::Fixed(n) | super::component::Constraint::Min(n) => n,
            _ => 0,
        })
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.is_empty() {
            return;
        }
        let resolved = self.resolve(area);
        let mut y = area.y;
        self.draw_header(buf, area, &resolved);
        y += 1 + u16::from(self.spaced);
        for i in 0..self.rows.len() {
            if y >= area.bottom() {
                return;
            }
            self.rows.with_row(i, &mut |row| self.draw_row(buf, y, area, &resolved, row, self.selected == Some(i), false));
            y += 1;
        }
        if let Some(totals) = &self.totals {
            y += u16::from(self.spaced);
            if y < area.bottom() {
                self.draw_row(buf, y, area, &resolved, totals, false, true);
            }
        }
    }
}
