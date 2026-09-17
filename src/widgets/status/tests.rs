use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;

fn draw(w: u16, bar: &StatusBar<'_>) -> String {
    let area = Rect::new(0, 0, w, 1);
    let mut buf = Buffer::empty(area);
    bar.render(&mut buf, area);
    (0..w).map(|x| buf[(x, 0)].symbol().to_string()).collect()
}

const KEYS: &[(&str, &str)] = &[("k", "look"), ("Tab", "wide"), ("q", "quit")];

#[test]
fn pairs_then_right_with_one_trailing_cell() {
    assert_eq!(draw(40, &StatusBar::new(KEYS).right("hi")), " [k] look  [Tab] wide  [q] quit      hi ");
    assert_eq!(draw(34, &StatusBar::new(KEYS)), " [k] look  [Tab] wide  [q] quit   ");
}

#[test]
fn whole_pairs_drop_from_the_right_before_right_is_cut() {
    // 33 cells: room for two pairs (1 + 11 + 12 = 24) beside a 9-cell Right.
    assert_eq!(draw(33, &StatusBar::new(KEYS).right("year 12!")), " [k] look  [Tab] wide   year 12! ");
    // Nothing fits beside Right: Right alone, cut to the width.
    assert_eq!(draw(6, &StatusBar::new(KEYS).right("year 12!")), "year  ");
    assert_eq!(StatusBar::new(KEYS).right("year 12!").min_width(), 9 + 1 + 10);
}

#[test]
fn right_only_is_cut_at_its_tail() {
    assert_eq!(draw(5, &StatusBar::new(&[]).right("abcdefgh")), "abcd ");
}
