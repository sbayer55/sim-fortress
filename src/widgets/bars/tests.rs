use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;
use crate::widgets::trend::{trend_arrow, TrendArrow};

fn draw(w: u16, c: &dyn Component) -> String {
    let area = Rect::new(0, 0, w, 1);
    let mut buf = Buffer::empty(area);
    c.render(&mut buf, area);
    (0..w).map(|x| buf[(x, 0)].symbol().to_string()).collect()
}

#[test]
fn filled_count_rounds_half_away_from_zero() {
    assert_eq!(draw(6, &Bar::new(0.5)), "[██░░]");
    assert_eq!(draw(6, &Bar::new(0.37)), "[█░░░]"); // 1.48 -> 1
    assert_eq!(draw(6, &Bar::new(0.38)), "[██░░]"); // 1.52 -> 2
    assert_eq!(draw(6, &Bar::new(2.0)), "[████]");
}

#[test]
fn marker_replaces_one_cell() {
    assert_eq!(draw(7, &Bar::new(1.0).marker(0.5)), "[██│██]");
    assert_eq!(draw(7, &Bar::new(0.0).marker(1.0)), "[░░░░│]");
}

#[test]
fn value_drops_before_the_bar_and_the_bar_is_never_cut() {
    let lb = LabeledBar::new("ab", 0.5).label_and_bar(3, 5);
    assert_eq!(lb.min_width(), 15);
    assert_eq!(draw(15, &lb), "ab [██░]    50%");
    assert_eq!(draw(12, &lb), "ab [██░]    "); // room for label and bar only
    assert_eq!(draw(7, &lb), "       "); // bar would be cut: nothing drawn
}

#[test]
fn count_and_suffix_forms() {
    let lb = LabeledBar::new("n", 0.25).label_and_bar(2, 6).value_text("8").suffix("of 32");
    assert_eq!(lb.min_width(), 2 + 6 + 7 + 6);
    assert_eq!(draw(21, &lb), "n [█░░░]      8 of 32");
    assert_eq!(draw(18, &lb), "n [█░░░]      8 of");
}

#[test]
fn vital_rule() {
    assert_eq!(vital_color(0.7, false), theme::GOOD);
    assert_eq!(vital_color(0.5, false), theme::WARN);
    assert_eq!(vital_color(0.2, false), theme::BAD);
    assert_eq!(vital_color(0.2, true), theme::GOOD);
}

#[test]
fn range_bar_cells_and_text() {
    assert_eq!(draw(5, &RangeBar::new(0.2, 0.5, 0.9).width(5)), "[▒█▒]");
    assert_eq!(draw(11, &RangeBar::new(0.62, 0.66, 0.75).width(11)), "[░░░░░█▒░░]");
    assert_eq!(draw(16, &RangeBar::new(0.1, 0.9, 0.3).width(5).text()), "[▒▒█] 0.10..0.30");
    assert_eq!(RangeBar::new(0.0, 0.0, 0.0).width(5).text().min_width(), 16);
}

#[test]
fn sparkline_draws_the_last_width_values_and_flat_is_dark() {
    assert_eq!(draw(3, &Sparkline::new(&[9, 9, 0, 1, 2])), "░▓█");
    assert_eq!(draw(4, &Sparkline::new(&[5, 5])), "▓▓  ");
    assert_eq!(draw(2, &Sparkline::new(&[])), "  ");
}

#[test]
fn trend_rule_thresholds_are_exclusive() {
    assert_eq!(trend_arrow(&[100, 103]), glyphs::FLAT);
    assert_eq!(trend_arrow(&[100, 104]), glyphs::UP);
    assert_eq!(trend_arrow(&[100, 96]), glyphs::DOWN);
    assert_eq!(trend_arrow(&[0, 1]), glyphs::UP);
    assert_eq!(trend_arrow(&[7]), glyphs::FLAT);
    let mut s = vec![200u16; 40];
    s.push(250);
    assert_eq!(trend_arrow(&s), glyphs::UP);
    assert_eq!(draw(11, &TrendArrow::new(&[10, 5]).word()), "↓ declining");
    assert_eq!(draw(1, &TrendArrow::new(&[5, 5])), "↔");
}
