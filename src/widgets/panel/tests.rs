use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;

fn row(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect()
}

#[test]
fn info_is_dropped_before_title_is_cut() {
    let area = Rect::new(0, 0, 20, 3);
    let mut buf = Buffer::empty(area);
    Panel::new("A long title").info("hint").render(&mut buf, area);
    assert_eq!(row(&buf, 0), "╔ A long title ════╗");
    let area = Rect::new(0, 0, 20, 3);
    let mut buf = Buffer::empty(area);
    Panel::new("Title").info("hint").render(&mut buf, area);
    assert_eq!(row(&buf, 0), "╔ Title ═════ hint ╗");
}

#[test]
fn title_is_cut_at_the_edge() {
    let area = Rect::new(0, 0, 10, 3);
    let mut buf = Buffer::empty(area);
    let inner = Panel::new("Simulation Controls").render(&mut buf, area);
    assert_eq!(row(&buf, 0), "╔ Simulat╗");
    assert_eq!(inner, Rect::new(1, 1, 8, 1));
}

#[test]
fn foot_needs_two_border_cells() {
    let area = Rect::new(0, 0, 9, 2);
    let mut buf = Buffer::empty(area);
    Panel::untitled().foot("↑3 ↓12").render(&mut buf, area);
    assert_eq!(row(&buf, 1), "╚═══════╝");
    let area = Rect::new(0, 0, 10, 2);
    let mut buf = Buffer::empty(area);
    Panel::untitled().foot("↑3 ↓12").render(&mut buf, area);
    assert_eq!(row(&buf, 1), "╚ ↑3 ↓12 ╝");
}

#[test]
fn tiny_areas_do_not_panic() {
    for (w, h) in [(0, 0), (1, 1), (2, 2), (3, 1)] {
        let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
        let inner = Panel::new("t").info("i").foot("f").render(&mut buf, area);
        assert!(inner.is_empty());
    }
}

#[test]
fn section_wrapper_draws_a_divider() {
    let area = Rect::new(0, 0, 12, 2);
    let mut buf = Buffer::empty(area);
    section_in(&mut buf, area, 1, "Clock");
    assert_eq!(row(&buf, 1), "─ Clock ────");
    section_in(&mut buf, area, 5, "Off");
}
