use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;

fn draw(w: u16, c: &dyn Component) -> String {
    let area = Rect::new(0, 0, w, 1);
    let mut buf = Buffer::empty(area);
    c.render(&mut buf, area);
    (0..w).map(|x| buf[(x, 0)].symbol().to_string()).collect()
}

#[test]
fn stepper_shapes() {
    assert_eq!(draw(20, &Stepper::new("W", "20").hint("hi").label_w(2).box_w(8)), " W ◄ 20   ► hi      ");
    assert_eq!(draw(20, &Stepper::new("W", "20").typing(Some("15")).label_w(2).box_w(8)), " W ◄ 15_  ►         ");
    assert_eq!(draw(9, &Stepper::compact("240")), "◄  240 ► ");
    assert_eq!(draw(14, &Stepper::inline("auto").text_w(6).key("[←→]")), "◄ auto  ► [←→]");
    assert_eq!(Stepper::new("W", "20").min_width(), 48);
}

#[test]
fn field_too_narrow_draws_nothing() {
    assert_eq!(draw(10, &Stepper::new("Water", "20")), "          ");
    assert_eq!(draw(12, &TextField::new("N", "abc").label_w(1).box_w(8)), " N[ abc  ]  ");
}

#[test]
fn focused_field_uses_the_selection_well() {
    let area = Rect::new(0, 0, 12, 1);
    let mut buf = Buffer::empty(area);
    TextField::new("N", "abc").label_w(1).box_w(8).focused(true).render(&mut buf, area);
    assert_eq!(buf[(4, 0)].bg, theme::SELECT_BG);
    assert_eq!(buf[(2, 0)].fg, theme::KEY);
    let mut buf = Buffer::empty(area);
    TextField::new("N", "abc").label_w(1).box_w(8).render(&mut buf, area);
    assert_eq!(buf[(4, 0)].bg, theme::BG);
}
