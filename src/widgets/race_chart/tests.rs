//! Race chart: corners, draw order, start offset, window, labels.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;

fn render(chart: &RaceChart<'_>, w: u16, h: u16) -> Vec<String> {
    let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
    chart.render(&mut buf, Rect::new(0, 0, w, h));
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' ')).collect::<String>().trim_end().to_string()).collect()
}

#[test]
fn rising_and_falling_steps_use_the_four_corners() {
    let chart = RaceChart::new("t").series(RaceSeries::new(&[0.0, 4.0, 4.0, 1.0]).lead(true)).rows(5);
    let rows = render(&chart, 9, 7);
    assert_eq!(
        rows,
        vec![
            "t ■1".to_string(),
            "│4┌■─■┐".to_string(),
            "│ │   │".to_string(),
            "│ │   │".to_string(),
            "│ │   └■".to_string(),
            "│■┘".to_string(),
            "└1───3───".to_string(),
        ]
    );
}

#[test]
fn the_lead_is_drawn_last_and_rivals_are_dotted() {
    let chart = RaceChart::new("k").series(RaceSeries::new(&[1.0, 1.0]).lead(true)).series(RaceSeries::new(&[1.0, 1.0])).rows(3).y_max(2.0);
    let rows = render(&chart, 7, 5);
    assert_eq!(rows.get(1).map(String::as_str), Some("│2"));
    assert_eq!(rows.get(2).map(String::as_str), Some("│■─■"), "the lead overwrites the rival's ∙ points");
    let rival_only = RaceChart::new("k").series(RaceSeries::new(&[1.0, 3.0])).rows(3);
    let rows = render(&rival_only, 7, 5);
    assert_eq!(rows.get(1).map(String::as_str), Some("│3┌∙"));
    assert_eq!(rows.first().map(String::as_str), Some("k ∙3"));
}

#[test]
fn a_late_start_draws_no_point_before_it_and_the_window_keeps_the_newest() {
    let late = RaceSeries::new(&[5.0, 5.0]).start(2);
    let chart = RaceChart::new("y").series(RaceSeries::new(&[1.0, 1.0, 1.0, 1.0]).lead(true)).series(late).rows(2);
    let rows = render(&chart, 9, 4);
    assert_eq!(rows.get(1).map(String::as_str), Some("│5   ∙─∙"));
    assert_eq!(rows.get(2).map(String::as_str), Some("│■─■─■─■"));
    // Twelve samples in a width that shows only four: the labels start at 9.
    let v: Vec<f32> = (0..12u16).map(f32::from).collect();
    let chart = RaceChart::new("w").series(RaceSeries::new(&v).lead(true)).rows(2);
    let rows = render(&chart, 8, 4);
    assert_eq!(rows.get(3).map(String::as_str), Some("└9───11─"));
    assert_eq!(rows.get(1).map(String::as_str), Some("│■─■─■─■"), "a line at the top overwrites the y-max label");
    let low = RaceChart::new("w").series(RaceSeries::new(&[0.0, 0.0, 0.0]).lead(true)).rows(2).y_max(9.0);
    let rows = render(&low, 8, 4);
    assert_eq!(rows.get(1).map(String::as_str), Some("│9"), "the y-max label sits inside the plot");
    assert_eq!(rows.get(2).map(String::as_str), Some("│■─■─■"));
}

#[test]
fn an_empty_chart_draws_only_the_axes_and_a_formatter_applies() {
    let chart = RaceChart::new("age").series(RaceSeries::new(&[])).rows(2);
    assert_eq!(render(&chart, 6, 4), vec!["age".to_string(), "│1".to_string(), "│".to_string(), "└─────".to_string()]);
    let years = |v: f32| format!("{}y", (v / 360.0).floor());
    let chart = RaceChart::new("age").series(RaceSeries::new(&[0.0, 720.0]).lead(true)).rows(2).value_fmt(&years);
    let rows = render(&chart, 8, 4);
    assert_eq!(rows.first().map(String::as_str), Some("age ■2y"));
    assert_eq!(rows.get(1).map(String::as_str), Some("│2┌■"), "the corner overwrites the label's second cell");
    assert_eq!(rows.get(2).map(String::as_str), Some("│■┘"));
    assert_eq!(chart.height(80), 4);
    assert_eq!(chart.min_width(), 5);
}

/// Prints the two sheet examples so they can be pasted into
/// `docs/components/race-chart.md`; run with `--nocapture`.
#[test]
fn sheet_examples() {
    let lead = [4.0, 13.0, 27.0, 45.0, 67.0, 92.0, 113.0, 139.0, 169.0, 202.0, 240.0, 243.0];
    let a = [3.0, 9.0, 17.0, 24.0, 33.0, 41.0, 52.0, 66.0, 79.0, 90.0, 99.0, 102.0];
    let b = [0.0, 6.0, 15.0, 22.0, 31.0, 40.0, 49.0, 61.0, 70.0, 81.0, 88.0, 89.0];
    let c = [2.0, 8.0, 12.0, 19.0, 30.0, 44.0, 56.0, 63.0, 71.0, 78.0, 85.0, 87.0];
    let chart = RaceChart::new("kills")
        .series(RaceSeries::new(&a).color(theme::TAN))
        .series(RaceSeries::new(&b).color(theme::ROSE))
        .series(RaceSeries::new(&c).color(theme::PREY))
        .series(RaceSeries::new(&lead).color(theme::PRED).lead(true))
        .rows(5);
    for row in render(&chart, 35, 7) {
        println!("{row}");
    }
    println!("--");
    let chart = RaceChart::new("young").series(RaceSeries::new(&[1.0, 4.0, 4.0, 2.0, 6.0, 6.0, 3.0]).color(theme::PRED).lead(true)).series(RaceSeries::new(&[5.0, 5.0, 2.0, 2.0, 1.0]).start(2).color(theme::TAN)).rows(5);
    for row in render(&chart, 21, 7) {
        println!("{row}");
    }
}
