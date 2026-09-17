use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;
use crate::widgets::stack::VStack;
use crate::widgets::text::Text;

fn rows_of(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height).map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect()
}

#[test]
fn foot_text_omits_zero_parts() {
    assert_eq!(foot(0, 9, 2), Some("↓7".into()));
    assert_eq!(foot(3, 9, 2), Some("↑3 ↓4".into()));
    assert_eq!(foot(7, 9, 2), Some("↑7".into()));
    assert_eq!(foot(0, 2, 2), None);
}

#[test]
fn window_shows_the_rows_at_the_clamped_offset() {
    let lines: Vec<Text<'_>> = (0..5).map(|i| Text::new(i.to_string())).collect();
    let stack = lines.iter().fold(VStack::new(), |s, t| s.child(t));
    let inner = Rect::new(2, 1, 3, 2);
    let mut buf = Buffer::empty(Rect::new(0, 0, 6, 4));
    let ov = ScrollRegion::new(&stack).offset(9).render(&mut buf, inner);
    assert_eq!(ov, Overflow { content: 5, offset: 3, visible: 2 });
    assert_eq!(rows_of(&buf), vec!["      ", "  3   ", "  4   ", "      "]);
    assert_eq!(ov.foot(), Some("↑3".into()));
    assert_eq!(ScrollRegion::new(&stack).overflow(inner).offset, 0);
}

#[test]
fn short_body_fills_the_window_with_the_panel_background() {
    let t = Text::new("a");
    let stack = VStack::new().child(&t);
    let inner = Rect::new(0, 0, 2, 3);
    let mut buf = Buffer::empty(inner);
    let ov = ScrollRegion::new(&stack).render(&mut buf, inner);
    assert_eq!(ov.foot(), None);
    assert_eq!(buf[(1, 2)].bg, theme::PANEL_BG);
}
