use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;
use crate::sim::Params;

fn rows_of(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height).map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect()
}

#[test]
fn sidebar_rows_and_footer() {
    let roster = Params::default().species;
    let legend = Legend::new(&[('a', theme::GOOD, "one"), ('b', theme::BAD, "two"), ('c', theme::DIM, "three")]).label_w(5).species(&roster);
    let species = roster.ids().count();
    assert_eq!(legend.height(20), 2 + crate::cast!(species.div_ceil(3) => u16) + 1);
    assert_eq!(legend.min_width(), 1 + 2 * 7);
    let area = Rect::new(0, 0, 16, 2);
    let mut buf = Buffer::empty(area);
    legend.render(&mut buf, area);
    assert_eq!(rows_of(&buf), vec![" a one  b two   ", " c three        "]);
    assert_eq!(buf[(1, 0)].fg, theme::GOOD);
}

#[test]
fn help_rows_carry_notes_and_bold_glyphs() {
    let legend = Legend::new(&[('a', theme::GOOD, "one")]).columns(1).label_w(4).notes(&["a note"]);
    let area = Rect::new(0, 0, 14, 1);
    let mut buf = Buffer::empty(area);
    legend.render(&mut buf, area);
    assert_eq!(rows_of(&buf), vec![" a one a note "]);
    assert!(buf[(1, 0)].modifier.contains(Modifier::BOLD));
    assert_eq!(buf[(3, 0)].fg, theme::TEXT);
}

#[test]
fn creatures_table_has_a_header_and_footer() {
    let roster = Params::default().species;
    let legend = Legend::creatures(&roster);
    let n = crate::cast!(roster.ids().count() => u16);
    assert_eq!(legend.height(40), n + 2);
    let area = Rect::new(0, 0, 30, 2);
    let mut buf = Buffer::empty(area);
    legend.render(&mut buf, area);
    assert!(rows_of(&buf)[0].starts_with(" ad jv  species role"));
    assert_eq!(buf[(1, 0)].fg, theme::ACCENT);
}
