use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;
use crate::widgets::component::distribute;
use crate::widgets::text::Text;
use Constraint::{Auto, Fill, Fixed, Min};

fn row(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect()
}

#[test]
fn fill_splits_odd_cell_to_the_last_child() {
    assert_eq!(distribute(&[Fill(1), Fill(1)], &[0, 0], 41, 0), vec![20, 21]);
    assert_eq!(distribute(&[Fill(1), Fill(1), Fill(1)], &[0, 0, 0], 10, 0), vec![3, 3, 4]);
}

#[test]
fn fill_shares_by_weight_after_fixed_and_gaps() {
    assert_eq!(distribute(&[Fixed(4), Fill(1), Fill(3)], &[0, 0, 0], 20, 0), vec![4, 4, 12]);
    assert_eq!(distribute(&[Auto, Fill(1)], &[5, 0], 20, 2), vec![5, 13]);
}

#[test]
fn min_measures_but_never_below_its_floor() {
    assert_eq!(distribute(&[Min(5), Fill(1)], &[3, 0], 10, 0), vec![5, 5]);
    assert_eq!(distribute(&[Min(5), Fill(1)], &[6, 0], 10, 0), vec![6, 4]);
}

#[test]
fn fill_with_nothing_left_gives_zero() {
    assert_eq!(distribute(&[Fixed(30), Fill(1)], &[0, 0], 20, 0), vec![30, 0]);
}

#[test]
fn vstack_reports_content_height_and_clips_at_the_bottom() {
    let (a, b, c) = (Text::new("a"), Text::new("b"), Text::new("c"));
    let gap = Spacer::rows(1);
    let stack = VStack::new().child(&a).child(&gap).child(&b).child(&c);
    assert_eq!(stack.height(10), 4);
    let area = Rect::new(0, 0, 3, 2);
    let mut buf = Buffer::empty(area);
    stack.render(&mut buf, area);
    assert_eq!(row(&buf, 0), "a  ");
    assert_eq!(row(&buf, 1), "   ");
}

#[test]
fn vstack_gap_adds_blank_rows() {
    let (a, b) = (Text::new("a"), Text::new("b"));
    let stack = VStack::new().child(&a).child(&b).gap(2);
    assert_eq!(stack.height(3), 4);
    let area = Rect::new(0, 0, 1, 4);
    let mut buf = Buffer::empty(area);
    stack.render(&mut buf, area);
    assert_eq!((0..4).map(|y| row(&buf, y)).collect::<String>(), "a  b");
}

#[test]
fn vstack_from_boxes_matches_child_by_child() {
    let boxes: Vec<Box<dyn Component>> = vec![Box::new(Text::new("x")), Box::new(Spacer::rows(2))];
    assert_eq!(VStack::from_boxes(&boxes).height(5), 3);
}

#[test]
fn hstack_areas_and_min_width() {
    let (a, b) = (Text::new("ab"), Text::new("c"));
    let stack = HStack::new().child(&a).child_with(Fill(1), &b).child_with(Fixed(3), &a).gap(1);
    assert_eq!(stack.min_width(), 2 + 3 + 2);
    let areas = stack.areas(Rect::new(0, 0, 12, 1));
    assert_eq!(areas, vec![Rect::new(0, 0, 2, 1), Rect::new(3, 0, 5, 1), Rect::new(9, 0, 3, 1)]);
    let area = Rect::new(0, 0, 12, 1);
    let mut buf = Buffer::empty(area);
    stack.render(&mut buf, area);
    assert_eq!(row(&buf, 0), "ab c     ab ");
}

#[test]
fn hstack_child_past_the_edge_is_not_drawn() {
    let a = Text::new("abc");
    let stack = HStack::new().child_with(Fixed(4), &a).child_with(Fixed(4), &a);
    let area = Rect::new(0, 0, 4, 1);
    let mut buf = Buffer::empty(area);
    stack.render(&mut buf, area);
    assert_eq!(row(&buf, 0), "abc ");
}

#[test]
fn text_alignment_and_cut() {
    let area = Rect::new(0, 0, 6, 1);
    let mut buf = Buffer::empty(area);
    Text::new("ab").right().render(&mut buf, area);
    assert_eq!(row(&buf, 0), "    ab");
    Text::new("abcdefgh").right().render(&mut buf, area);
    assert_eq!(row(&buf, 0), "abcdef");
    let area = Rect::new(0, 0, 6, 1);
    let mut buf = Buffer::empty(area);
    Text::new("ab").center().render(&mut buf, area);
    assert_eq!(row(&buf, 0), "  ab  ");
    assert_eq!(Text::new("héllo").min_width(), 5);
}
