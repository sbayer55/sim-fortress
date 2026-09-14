//! S05a — prey/predator population lines, with the drought band and census sidebar.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::Frame;
use crate::sim::{Kind, Sim, SpeciesId};
use crate::ui::screens::common::{arrow_color, sp, trend_arrow};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{panel, util};
use crate::{glyphs, theme};
use super::{Window, round_up, stats_of};

pub(super) fn time_chart(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw_with_hint(f, area, "Population — prey vs predators", &format!("last {} days, 1 point per day", w.len()), panel::Kind::Outer);
    let half = (inner.height.saturating_sub(1)).div_euclid(2);
    let upper = Rect::new(inner.x, inner.y, inner.width, half);
    let lower = Rect::new(inner.x, inner.y + half + 1, inner.width, inner.height - half - 1);
    util::line(f, upper, 0, Line::from(sp(" Prey (voles + hares + deer)", Style::default().fg(theme::HARE).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))));
    line_chart(f, Rect::new(upper.x, upper.y + 1, upper.width, upper.height - 1), w, &w.prey, theme::HARE, 100.0);
    panel::section(f, inner, half, "Predators (foxes + wolves + lynx)");
    let label = if w.pred.iter().all(|&v| v <= 0.0) { " no predators yet" } else { " " };
    util::line(f, lower, 0, Line::from(sp(label, theme::dim_text())));
    line_chart(f, Rect::new(lower.x, lower.y + 1, lower.width, lower.height - 1), w, &w.pred, theme::WOLF, 20.0);
    let _ = sim;
}

/// One series as a half-block line with a y axis on the left, x labels below.
fn line_chart(f: &mut Frame<'_>, area: Rect, w: &Window<'_>, series: &[f32], color: Color, step: f32) {
    if area.height < 4 || area.width < 12 {
        return;
    }
    let label_w = 6u16;
    let plot = Rect::new(area.x + label_w, area.y, area.width - label_w - 1, area.height - 2);
    let axis_y = plot.bottom();
    let cols = crate::cast!(plot.width => usize);
    let rows = crate::cast!(plot.height => usize);
    let max = series.iter().copied().fold(0.0f32, f32::max);
    let y_max = round_up(max, step);
    let buf = f.buffer_mut();
    let band = theme::lerp(theme::PANEL_BG, theme::WARN, 0.22);
    plot_columns(buf, plot, w, series, color, y_max, cols, rows, band);
    draw_axes(buf, area, plot, axis_y, w, y_max, cols, rows);
}

/// Draw the series columns, including the drought band and its label.
#[allow(clippy::too_many_arguments)]
fn plot_columns(buf: &mut Buffer, plot: Rect, w: &Window<'_>, series: &[f32], color: Color, y_max: f32, cols: usize, rows: usize, band: Color) {
    let mut band_started: Option<u16> = None;
    for c in 0..cols {
        let x = plot.x + crate::cast!(c => u16);
        let dry = w.len() > 0 && w.drought_in(c, cols);
        if dry {
            for r in 0..rows {
                if let Some(cell) = buf.cell_mut((x, plot.y + crate::cast!(r => u16))) {
                    cell.set_char(' ');
                    cell.set_style(Style::default().bg(band));
                }
            }
            if band_started.is_none() {
                band_started = Some(x);
            }
        }
        if w.len() == 0 {
            continue;
        }
        let v = w.mean_over(series, c, cols);
        let halves = crate::cast!(((v / y_max) * (crate::cast!(rows => f32) * 2.0)).round() => usize);
        if halves == 0 {
            // Flat zero: a lower half-block on the bottom row.
            if let Some(cell) = buf.cell_mut((x, plot.y + crate::cast!(rows => u16) - 1)) {
                cell.set_char(glyphs::HALF_LOWER);
                cell.set_style(Style::default().fg(color).bg(cell.bg));
            }
            continue;
        }
        let r = rows - 1 - ((halves - 1).div_euclid(2)).min(rows - 1);
        let ch = if halves % 2 == 1 { glyphs::HALF_LOWER } else { glyphs::HALF_UPPER };
        if let Some(cell) = buf.cell_mut((x, plot.y + crate::cast!(r => u16))) {
            cell.set_char(ch);
            cell.set_style(Style::default().fg(color).bg(cell.bg).add_modifier(Modifier::BOLD));
        }
    }
    if let Some(bx) = band_started {
        buf.set_stringn(bx + 1, plot.y, format!("{} drought", glyphs::DROUGHT), 10, Style::default().fg(theme::WARN).bg(band));
    }
}

/// Draw the Y labels, the box lines and the X axis with seven labels.
#[allow(clippy::too_many_arguments)]
fn draw_axes(buf: &mut Buffer, area: Rect, plot: Rect, axis_y: u16, w: &Window<'_>, y_max: f32, cols: usize, rows: usize) {
    // Y labels: 0, ¼, ½, ¾, max.
    for k in 0..=4 {
        let y = axis_y - 1 - crate::cast!((crate::cast!((rows - 1) => f32) * crate::cast!(k => f32) / 4.0).round() => u16);
        let v = crate::cast!((y_max * crate::cast!(k => f32) / 4.0).round() => u32);
        buf.set_stringn(area.x, y, format!("{v:>5}"), 5, theme::dim_text());
        buf.set_stringn(plot.x - 1, y, glyphs::CROSS.to_string(), 1, theme::border());
    }
    for r in 0..rows {
        let y = plot.y + crate::cast!(r => u16);
        if buf.cell((plot.x - 1, y)).is_some_and(|c| c.symbol() != "┼") {
            buf.set_stringn(plot.x - 1, y, glyphs::V_LINE.to_string(), 1, theme::border());
        }
    }
    // X axis with seven labels.
    let axis: String = std::iter::repeat_n(glyphs::H_LINE, cols + 1).collect();
    buf.set_stringn(plot.x - 1, axis_y, &axis, cols + 1, theme::border());
    let n = w.len().max(1);
    for k in 0..=6 {
        let i = ((k * (n - 1)).div_euclid(6)).min(n - 1);
        let x = if k == 6 { plot.x + crate::cast!(cols => u16) - 1 } else { plot.x + crate::cast!(((i * cols).div_euclid(n)) => u16) };
        buf.set_stringn(x, axis_y, glyphs::CROSS.to_string(), 1, theme::border());
        let label = w.day_label(i);
        if !label.is_empty() {
            let lx = crate::cast!((i32::from(x) - crate::cast!(label.len() => i32).div_euclid(2)).max(i32::from(area.x)) => u16);
            let lx = lx.min(area.right().saturating_sub(crate::cast!(label.len() => u16) + 1));
            buf.set_stringn(lx, axis_y + 1, &label, label.len(), theme::dim_text());
        }
    }
    buf.set_stringn(plot.right() - 3, axis_y, "now", 3, Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG));
}

fn pct_change(series: &[f32], days: usize) -> Option<f32> {
    if series.len() < 2 {
        return None;
    }
    let a = series[series.len().saturating_sub(days).min(series.len() - 1)];
    let b = series.last().copied().unwrap_or(0.0);
    if a <= 0.0 {
        return None;
    }
    Some((b - a) / a * 100.0)
}

pub(super) fn time_sidebar(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw(f, area, "Statistics", panel::Kind::Outer);
    let mut row = 0u16;
    panel::section(f, inner, row, "Current");
    row += 1;
    let prey_now = sim.species.iter().filter(|s| s.species.kind() == Kind::Prey).map(|s| s.count).sum::<u32>();
    let counts: Vec<u16> = w.prey.iter().map(|v| crate::cast!(*v => u16)).collect();
    let arrow = trend_arrow(&counts);
    let pct = pct_change(&w.prey, 30).map_or_else(|| "–".into(), |p| format!("{p:+.0}%"));
    util::line(f, inner, row, Line::from(vec![
        sp(" prey       ", theme::dim_text()),
        sp(format!("{prey_now:>5}"), Style::default().fg(theme::HARE).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(format!("  {arrow} {pct} / 30d"), Style::default().fg(arrow_color(arrow)).bg(theme::PANEL_BG)),
    ]));
    row += 1;
    let pred_now = sim.species.iter().filter(|s| s.species.kind() == Kind::Predator).map(|s| s.count).sum::<u32>();
    let pred_counts: Vec<u16> = w.pred.iter().map(|v| crate::cast!(*v => u16)).collect();
    let parrow = trend_arrow(&pred_counts);
    let ppct = pct_change(&w.pred, 30).map_or_else(|| "–".into(), |p| format!("{p:+.0}%"));
    util::line(f, inner, row, Line::from(vec![
        sp(" predators  ", theme::dim_text()),
        sp(format!("{pred_now:>5}"), Style::default().fg(theme::WOLF).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(format!("  {parrow} {ppct} / 30d"), Style::default().fg(arrow_color(parrow)).bg(theme::PANEL_BG)),
    ]));
    row += 1;
    let ratio = if pred_now > 0 { format!("{:.1} prey per predator", crate::cast!(prey_now => f32) / crate::cast!(pred_now => f32)) } else { "–  (no predators)".to_string() };
    util::line(f, inner, row, Line::from(sp(format!(" ratio {ratio}"), theme::dim_text())));
    row += 2;

    row = side_stats(f, inner, row, "Prey", &w.prey, w, "  (max − min) ÷ mean", " no history yet");
    row = side_stats(f, inner, row, "Predators", &w.pred, w, "", " no predators yet");

    row = drought_section(f, inner, row, w);

    row = map_census(f, inner, row, sim);

    panel::section(f, inner, row, "Legend");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {}{} ", glyphs::HALF_UPPER, glyphs::HALF_LOWER), Style::default().fg(theme::HARE).bg(theme::PANEL_BG)),
        sp("prey (voles + hares + deer)", theme::text()),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {}{} ", glyphs::HALF_UPPER, glyphs::HALF_LOWER), Style::default().fg(theme::WOLF).bg(theme::PANEL_BG)),
        sp("predators (none yet)", theme::text()),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" ░ ", Style::default().fg(theme::WARN).bg(theme::PANEL_BG)),
        sp("drought band (any region in drought)", theme::text()),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(sp(" separate y scales: prey per 100, predators per 20", theme::dim_text())));
    row += 2;
    panel::section(f, inner, row, "Keys");
    row += 1;
    util::line(f, inner, row, Line::from(sp(" [+/-] zoom 60/240/720d", theme::dim_text())));
}

/// One min/max/mean/swing block for a series.
#[allow(clippy::too_many_arguments)]
fn side_stats(f: &mut Frame<'_>, inner: Rect, mut row: u16, title: &str, series: &[f32], w: &Window<'_>, swing_note: &str, empty: &str) -> u16 {
    panel::section(f, inner, row, title);
    row += 1;
    if let Some((min, imin, max, imax, mean)) = stats_of(series) {
        let swing = if mean > 0.0 { (max - min) / mean * 100.0 } else { 0.0 };
        for (k, v) in [
            ("min", format!("{:.0}  on {}", min, w.day_label(imin))),
            ("max", format!("{:.0}  on {}", max, w.day_label(imax))),
            ("mean", format!("{mean:.0}")),
            ("swing", format!("{swing:.0}%{swing_note}")),
        ] {
            util::line(f, inner, row, Line::from(vec![sp(format!(" {k:<7}"), theme::dim_text()), sp(v, theme::text())]));
            row += 1;
        }
    } else {
        util::line(f, inner, row, Line::from(sp(empty, theme::dim_text())));
        row += 1;
    }
    row += 1;
    row
}

/// The drought summary, shown only when the window contains drought days.
fn drought_section(f: &mut Frame<'_>, inner: Rect, mut row: u16, w: &Window<'_>) -> u16 {
    let dry_days = w.drought.iter().filter(|&&d| d).count();
    if dry_days > 0 {
        panel::section(f, inner, row, "Drought");
        row += 1;
        let first = w.drought.iter().position(|&d| d).unwrap_or(0);
        let last = w.drought.iter().rposition(|&d| d).unwrap_or(0);
        let veg_before = w.veg.get(first.saturating_sub(1)).copied().unwrap_or(0.0);
        let veg_min = w.veg[first..=last].iter().copied().fold(1.0f32, f32::min);
        let water = w.samples.get(last).map_or(1.0, |s| s.water_level);
        util::line(f, inner, row, Line::from(sp(format!(" {} {} drought days: {} to {}", glyphs::DROUGHT, dry_days, w.day_label(first), w.day_label(last)), Style::default().fg(theme::WARN).bg(theme::PANEL_BG))));
        row += 1;
        util::line(f, inner, row, Line::from(sp(format!(" vegetation {:.0}% {} {:.0}%, water x{:.2}", veg_before * 100.0, glyphs::DOWN, veg_min * 100.0, water), theme::text())));
        row += 1;
        let p0 = w.prey.get(first).copied().unwrap_or(0.0);
        let p1 = w.prey.get(last).copied().unwrap_or(0.0);
        util::line(f, inner, row, Line::from(sp(format!(" prey {p0:.0} at the start, {p1:.0} at the end"), theme::text())));
        row += 2;
    }
    row
}

/// The per-species census rows.
fn map_census(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim) -> u16 {
    panel::section(f, inner, row, "Map census");
    row += 1;
    for chunk in SpeciesId::ALL.chunks(3) {
        let mut spans = Vec::new();
        for id in chunk {
            let s = &sim.species[id.index()];
            spans.push(sp(format!(" {} ", id.glyph().to_ascii_uppercase()), Style::default().fg(if s.count == 0 { theme::DIM } else { id.color() }).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)));
            spans.push(sp(format!("{:<5}{:>4}  ", id.name(), s.count), if s.count == 0 { theme::dim_text() } else { theme::text() }));
        }
        util::line(f, inner, row, Line::from(spans));
        row += 1;
    }
    row += 1;
    row
}
