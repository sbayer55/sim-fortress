use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;
use crate::widgets::component::Constraint::{Fill, Fixed, Min};

fn rows_of(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height).map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect()
}

const COLS: [Column; 3] = [Column::titled("Name", Fixed(6)), Column::titled("N", Min(3)).right(), Column::new(Fill(1))];

fn rows() -> Vec<TableRow<'static>> {
    vec![
        TableRow::new([TableCell::text("Vole"), TableCell::text("7"), TableCell::dim("prey")]),
        TableRow::new([TableCell::text("Deer"), TableCell::text("4133"), TableCell::dim("prey")]).absent(true),
    ]
}

#[test]
fn header_marker_selection_and_measured_count() {
    let rows = rows();
    let table = Table::new(&COLS, &rows).selected(Some(0));
    assert_eq!(table.height(20), 3);
    assert_eq!(table.min_width(), 7);
    let area = Rect::new(0, 0, 16, 3);
    let mut buf = Buffer::empty(area);
    table.render(&mut buf, area);
    assert_eq!(rows_of(&buf), vec![" Name     N     ", "►Vole     7prey ", " Deer  4133prey "]);
    assert_eq!(buf[(0, 1)].fg, theme::KEY);
    assert_eq!(buf[(15, 1)].bg, theme::SELECT_BG);
    assert_eq!(buf[(1, 2)].fg, theme::DIM, "absent row is dim");
}

#[test]
fn spaced_totals_with_a_tail_and_sort_info() {
    let rows = rows();
    let totals = TableRow::new([TableCell::text("totals"), TableCell::text("4140")]).tail(Text::new(" ratio 1:2"));
    let table = Table::new(&COLS, &rows).totals(totals).spaced(true);
    assert_eq!(table.height(20), 6);
    let area = Rect::new(0, 0, 22, 6);
    let mut buf = Buffer::empty(area);
    table.render(&mut buf, area);
    assert_eq!(rows_of(&buf)[5], " totals4140 ratio 1:2 ");
    assert_eq!(buf[(1, 5)].fg, theme::ACCENT, "totals label");
    assert_eq!(Table::sort_info("count"), "sorted by count ↓");
}

#[test]
fn rows_past_the_bottom_are_not_drawn() {
    let rows = rows();
    let area = Rect::new(0, 0, 16, 2);
    let mut buf = Buffer::empty(area);
    Table::new(&COLS, &rows).render(&mut buf, area);
    assert_eq!(rows_of(&buf)[1], " Vole     7prey ");
}
