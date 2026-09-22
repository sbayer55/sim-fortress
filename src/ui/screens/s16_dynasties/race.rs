//! The six race charts: the selected line against the region's other lines.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::widgets::{Component, Divider, RaceChart, RaceSeries};
use crate::{glyphs, theme};

use super::{bold, fg, line_name, put, race_values, Stat, View};

const CHART_W: u16 = 35;
const CHART_H: u16 = 7;
const NOTE: &str = "totals: kills, young, survival, mutations · year-end levels: territory = cells the living hold, age = oldest";

/// Draws the divider, the legend, the 3 × 2 grid and the note from `y`; returns the next row.
pub(super) fn draw(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>) -> u16 {
    let Some(d) = v.dynasty() else { return y };
    Divider::new("Race since Y1").render(buf, Rect::new(inner.x, y, inner.width, 1));
    let rivals = v.rivals();
    let mut lx = inner.x + 18;
    let lead = format!(" {} {} ", glyphs::SQUARE, line_name(d));
    put(buf, lx, y, crate::cast!(lead.chars().count() => u16), &lead, bold(v.color(d.species)));
    lx += crate::cast!(lead.chars().count() => u16);
    for &ri in &rivals {
        let Some(r) = v.ranked.dynasties.get(ri) else { continue };
        let text = format!(" {} {} ", glyphs::TRAIL, line_name(r));
        put(buf, lx, y, crate::cast!(text.chars().count() => u16), &text, fg(theme::dim(v.color(r.species), 0.55)));
        lx += crate::cast!(text.chars().count() => u16);
    }
    let year_days = v.year_days;
    let years = move |v: f32| format!("{}y", (v / crate::cast!(year_days.max(1) => f32)).floor());
    for (k, stat) in Stat::ALL.into_iter().enumerate() {
        let (col, row) = (crate::cast!(k % 3 => u16), crate::cast!(k.div_euclid(3) => u16));
        let area = Rect::new(inner.x + 1 + col * (CHART_W + 1), y + 1 + row * CHART_H, CHART_W, CHART_H);
        let rival_vals: Vec<(usize, Vec<f32>, ratatui::style::Color)> =
            rivals.iter().filter_map(|&ri| v.ranked.dynasties.get(ri)).map(|r| { let (s, vals) = race_values(r, stat); (s, vals, v.color(r.species)) }).collect();
        let (start, vals) = race_values(d, stat);
        let mut chart = RaceChart::new(stat.label()).series(RaceSeries::new(&vals).start(start).color(v.color(d.species)).lead(true)).rows(CHART_H - 2);
        for (s, rv, c) in &rival_vals {
            chart = chart.series(RaceSeries::new(rv).start(*s).color(*c));
        }
        if stat == Stat::Age {
            chart = chart.value_fmt(&years);
        }
        chart.render(buf, area);
    }
    put(buf, inner.x + 1, y + 1 + 2 * CHART_H, inner.width - 1, NOTE, fg(theme::DIM));
    y + 2 + 2 * CHART_H
}
