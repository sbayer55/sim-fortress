use super::*;

fn draw(r: &Ribbon<'_>, w: u16, h: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
    r.render(&mut buf, Rect::new(0, 0, w, h));
    buf
}

fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect()
}

#[test]
fn geometry_of_the_s17_ribbon() {
    // 6 stalk ticks + rule + 30 chase ticks + ┤ at two cells per tick = 74 columns.
    let r = Ribbon::new(&[], &[]);
    assert_eq!(r.min_width(), 74);
    assert_eq!(r.height(74), 5);
    let buf = draw(&r, 74, 5);
    for y in 0..5 {
        assert_eq!(buf[(12, y)].symbol(), "│", "the clock rule on row {y}");
        assert_eq!(buf[(73, y)].symbol(), "┤", "the clock out on row {y}");
    }
    let bottom: Vec<char> = row_text(&buf, 4).chars().collect();
    assert_eq!(bottom[33..35].iter().collect::<String>(), "10", "tick 10 labels at 13 + 2·10");
    assert_eq!(bottom[53..55].iter().collect::<String>(), "20");
}

#[test]
fn values_land_on_their_rows_and_the_newest_is_the_play_mark() {
    let chase = [0.0, 0.5, 1.0];
    let r = Ribbon::new(&[0.25], &chase);
    assert_eq!(r.row_of(0.0), 0);
    assert_eq!(r.row_of(0.5), 2);
    assert_eq!(r.row_of(1.0), 4);
    let buf = draw(&r, 74, 5);
    // The one stalk value sits right before the rule at column 10, row 1.
    assert_eq!(buf[(10, 1)].symbol(), "■");
    assert_eq!(buf[(11, 1)].symbol(), "─", "the connector cell");
    assert_eq!(buf[(13, 0)].symbol(), "■", "chase tick 0 on the escape edge");
    assert_eq!(buf[(15, 2)].symbol(), "■");
    assert_eq!(buf[(17, 4)].symbol(), "►", "the newest tick is the play mark");
    assert_eq!(buf[(17, 3)].symbol(), "│", "a jump of two rows leaves a connector in the new tick's column");
}

#[test]
fn a_resolved_hunt_pins_the_final_tick_to_its_edge() {
    let r = Ribbon::new(&[], &[0.6, 0.7, 0.8]).outcome(Some(RibbonEnd::Escaped));
    let buf = draw(&r, 74, 5);
    assert_eq!(buf[(17, 0)].symbol(), "→", "an escape ends on the escape edge whatever its value was");
    let k = Ribbon::new(&[], &[0.2, 0.3]).outcome(Some(RibbonEnd::Kill));
    let buf = draw(&k, 74, 5);
    assert_eq!(buf[(15, 4)].symbol(), "x");
}

#[test]
fn caption_is_centred_on_the_middle_row_of_the_clock_zone() {
    let r = Ribbon::new(&[], &[]).live(false).caption("resting");
    let buf = draw(&r, 74, 5);
    let mid: Vec<char> = row_text(&buf, 2).chars().collect();
    let i = (0..mid.len()).find(|&i| mid[i..].iter().collect::<String>().starts_with(" resting ")).expect("the caption is drawn");
    // Clock zone is columns 13..=72 (60 cells); the 9-cell caption starts at 13 + (60 − 9) / 2.
    assert_eq!(i, 13 + (60usize - 9).div_euclid(2));
}

#[test]
fn only_the_last_stalk_ticks_and_the_clock_length_are_shown() {
    let stalk = [0.1; 10];
    let chase = [0.9; 40];
    let r = Ribbon::new(&stalk, &chase);
    let buf = draw(&r, 74, 5);
    assert_eq!(buf[(0, 0)].symbol(), "■", "the sixth-newest stalk tick fills the first stalk cell");
    assert_eq!(buf[(71, 4)].symbol(), "►", "chase tick 29 is the last one shown, at 13 + 2·29");
    assert_eq!(buf[(73, 4)].symbol(), "┤", "nothing writes over the clock-out rule");
}

#[test]
fn a_narrow_area_is_never_written_outside() {
    let r = Ribbon::new(&[0.5; 6], &[0.5; 30]).caption("a long caption that does not fit");
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 7));
    r.render(&mut buf, Rect::new(1, 1, 30, 5));
    for x in 0..40 {
        assert_eq!(buf[(x, 0)].symbol(), " ");
        assert_eq!(buf[(x, 6)].symbol(), " ");
    }
    for y in 0..7 {
        assert_eq!(buf[(0, y)].symbol(), " ");
        assert_eq!(buf[(31, y)].symbol(), " ", "column past the area on row {y}");
    }
}

#[test]
fn every_glyph_is_cp437() {
    let r = Ribbon::new(&[0.2, 0.4], &[0.3, 0.9, 1.0]).outcome(Some(RibbonEnd::Lost)).caption("lost");
    let buf = draw(&r, 74, 5);
    for y in 0..5 {
        for ch in row_text(&buf, y).chars() {
            assert!(glyphs::is_cp437(ch), "{ch:?}");
        }
    }
}

/// Prints the sheet examples; run with `--ignored --nocapture` when the sheet changes.
#[test]
#[ignore = "prints the docs/components/ribbon.md fixtures"]
fn dump_sheet_examples() {
    let ex1 = Ribbon::new(&[0.30, 0.35], &[0.40, 0.45, 0.55, 0.60, 0.70, 0.80, 0.85]).stalk_ticks(2).chase_max(12);
    let ex2 = Ribbon::new(&[0.30, 0.30], &[0.35, 0.30, 0.25, 0.20, 0.10]).stalk_ticks(2).chase_max(12).outcome(Some(RibbonEnd::Escaped)).live(false).caption("cooldown 5 h");
    let ex3 = Ribbon::new(&[0.35, 0.38, 0.42, 0.45, 0.48, 0.50], &[0.52, 0.55, 0.60, 0.62, 0.70, 0.72, 0.78, 0.85]);
    for (name, r, w) in [("ex1", &ex1, 30u16), ("ex2", &ex2, 30), ("ex3", &ex3, 74)] {
        let buf = draw(r, w, 5);
        println!("=== {name}");
        for y in 0..5 {
            println!("{}", row_text(&buf, y));
        }
    }
}
