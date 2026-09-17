use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;

fn rows_of(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height).map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect()
}

#[test]
fn bins_and_means() {
    assert_eq!(bin(10, 0, 5), (0, 2));
    assert_eq!(bin(10, 4, 5), (8, 10));
    assert_eq!(bin(3, 5, 10), (1, 2));
    assert!((mean_over(&[1.0, 3.0, 5.0, 7.0], 1, 2) - 6.0).abs() < 1e-6);
    assert!(mean_over(&[], 0, 4).abs() < 1e-6);
    assert!((round_up(0.0, 100.0) - 100.0).abs() < 1e-6);
    assert!((round_up(101.0, 100.0) - 200.0).abs() < 1e-6);
}

#[test]
fn empty_chart_draws_labels_axis_and_now() {
    let area = Rect::new(0, 0, 20, 7);
    let mut buf = Buffer::empty(area);
    Chart::new().series(Series::new(&[]).color(theme::PREY)).y_step(100.0).render(&mut buf, area);
    let rows = rows_of(&buf);
    assert_eq!(rows[0], "  100┼              ");
    assert_eq!(rows[4], "    0┼              ");
    assert_eq!(rows[5], "     ─┼─────────now ");
    assert_eq!(rows[6], "                    ");
}

#[test]
fn series_uses_half_rows_and_flat_zero_is_a_lower_block() {
    // Five plot columns over four samples (column 1 repeats sample 0), four plot rows.
    let area = Rect::new(0, 0, 12, 6);
    let mut buf = Buffer::empty(area);
    let labels = |i: usize| format!("D{i}");
    Chart::new().series(Series::new(&[0.0, 25.0, 50.0, 100.0]).color(theme::PREY)).y_step(100.0).x_label(&labels).render(&mut buf, area);
    let rows = rows_of(&buf);
    assert_eq!(rows[0], "  100┼    ▀ ");
    assert_eq!(rows[2], "   25┼   ▀  ");
    assert_eq!(rows[3], "    0┼▄▄▀   ");
    assert_eq!(rows[4], "     ─┼┼now ");
    assert!(rows[5].ends_with("D3 "), "the last x label survives at the right: {:?}", rows[5]);
}

#[test]
fn band_tints_columns_and_labels_the_first() {
    // Sixteen plot columns over eight samples: sample 2 spans columns 4 and 5.
    let area = Rect::new(0, 0, 23, 6);
    let mut buf = Buffer::empty(area);
    let values = [1.0f32; 8];
    let dry = [false, false, true, true, false, false, false, false];
    Chart::new()
        .series(Series::new(&values).color(theme::PREY))
        .y_step(10.0)
        .band(Band::new(&dry).color(theme::WARN).label(glyphs::DROUGHT, "drought"))
        .render(&mut buf, area);
    let tint = theme::lerp(theme::PANEL_BG, theme::WARN, 0.22);
    assert_eq!(buf[(6 + 4, 1)].bg, tint);
    assert_eq!(buf[(6 + 7, 1)].bg, tint);
    assert_ne!(buf[(6 + 3, 1)].bg, tint);
    assert_ne!(buf[(6 + 8, 1)].bg, tint);
    assert_eq!(rows_of(&buf)[0], "   10┼     ¡ drought   ");
    assert_eq!(buf[(6, 3)].symbol(), "▄", "0.1 of the scale is one half: a lower block on the bottom row");
}
