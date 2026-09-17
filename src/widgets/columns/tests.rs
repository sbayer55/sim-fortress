use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;
use crate::widgets::stack::Spacer;
use Constraint::{Fill, Fixed, Min};

fn rows_of(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height).map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect()
}

fn row(name: &'static str, count: u32) -> Row<'static> {
    Row::new().cell(Text::new(name)).cell(Text::new(count.to_string())).cell(Spacer::cols(1)).cell(Text::new("x"))
}

#[test]
fn min_column_measures_every_row_and_moves_later_cells() {
    let cols = Columns::new(&[Fixed(5), Min(3), Fixed(1), Fill(1)]).align(1, Align::Right);
    let area = Rect::new(0, 0, 14, 2);
    let mut buf = Buffer::empty(area);
    Block::new(cols.clone()).rows([row("Vole", 7), row("Deer", 4133)]).render(&mut buf, area);
    assert_eq!(rows_of(&buf), vec!["Vole    7 x   ", "Deer 4133 x   "]);
    assert_eq!(cols.min_width(), 9);
}

#[test]
fn trailing_columns_drop_whole_and_selected_row_fills_the_width() {
    let cols = Columns::new(&[Fixed(4), Fixed(4), Fixed(4)]);
    let area = Rect::new(0, 0, 10, 2);
    let mut buf = Buffer::empty(area);
    let rows = [Row::new().cell(Text::new("a")).cell(Text::new("b")).cell(Text::new("c")), Row::span(Text::new("spanning row"))];
    Block::new(cols).rows(rows).selected(Some(0)).render(&mut buf, area);
    assert_eq!(rows_of(&buf), vec!["a   b     ", "spanning r"]);
    assert_eq!(buf[(9, 0)].bg, theme::SELECT_BG);
    assert_ne!(buf[(9, 1)].bg, theme::SELECT_BG);
}

#[test]
fn resolve_reports_offsets_and_widths() {
    let cols = Columns::new(&[Fixed(3), Min(2), Fill(1), Fill(1)]);
    assert_eq!(cols.resolve(10, 20, |i| if i == 1 { 4 } else { 0 }), vec![(10, 3), (13, 4), (17, 6), (23, 7)]);
    assert_eq!(cols.resolve(0, 5, |_| 0), vec![(0, 3), (3, 2), (5, 0), (5, 0)], "zero-width Fill columns stay as empty slots");
}
