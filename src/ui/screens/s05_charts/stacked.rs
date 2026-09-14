//! S05c — stacked species over vegetation (C4 FR9).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::Frame;
use crate::sim::{Sim, SpeciesId};
use crate::ui::screens::common::{arrow_color, sp, trend_arrow};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{panel, util};
use crate::{glyphs, theme};
use super::{Window, round_up, stats_of};

const Y_LABEL_W: u16 = 6;

const R_AXIS_W: u16 = 6;

fn shade_empty(buf: &mut Buffer, x: u16, y0: u16, y1: u16, band: Color) {
    for y in y0..y1 {
        if let Some(cell) = buf.cell_mut((x, y)) {
            if cell.symbol() == " " {
                cell.set_style(Style::default().bg(band));
            }
        }
    }
}

pub(super) fn stacked_chart(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw_with_hint(f, area, "Stacked populations + vegetation", &format!("{} days, three prey species", w.len()), panel::Kind::Outer);
    let _ = sim;
    // Legend row.
    let mut spans = vec![sp(" ", theme::text())];
    for id in SpeciesId::ALL.iter().take(3) {
        spans.push(sp(glyphs::FULL_BLOCK.to_string(), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)));
        spans.push(sp(format!(" {}   ", id.plural()), theme::text()));
    }
    spans.push(sp(glyphs::DOT.to_string(), Style::default().fg(theme::VEGETATION).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)));
    spans.push(sp(" vegetation biomass (right axis, % of max)", theme::text()));
    util::line(f, inner, 0, Line::from(spans));
    if inner.height < 12 {
        return;
    }

    let plot_x = inner.x + Y_LABEL_W;
    let plot_w = inner.width - Y_LABEL_W - R_AXIS_W;
    let plot_y = inner.y + 2;
    let plot_h = inner.height - 7;
    let axis_y = plot_y + plot_h;
    let cols = crate::cast!(plot_w => usize);
    let n = w.len();

    let (stack, veg) = stacked_bins(w, cols);
    let max_total = stack.iter().map(|v| v.iter().sum::<f32>()).fold(0.0f32, f32::max);
    let y_max = round_up(max_total, 50.0);
    let halves = crate::cast!(plot_h => usize) * 2;
    let band = theme::lerp(theme::PANEL_BG, theme::WARN, 0.22);
    let buf = f.buffer_mut();
    let band_x = stacked_columns(buf, w, plot_x, plot_y, plot_h, cols, y_max, halves, band, n, &stack, &veg);
    if let Some(bx) = band_x {
        shade_empty(buf, bx, plot_y, axis_y, band);
        buf.set_stringn(bx + 1, plot_y, format!("{} drought", glyphs::DROUGHT), 12, Style::default().fg(theme::WARN).bg(band));
    }
    for r in 0..plot_h {
        let y = plot_y + r;
        buf.set_stringn(plot_x - 1, y, glyphs::V_LINE.to_string(), 1, theme::border());
        buf.set_stringn(plot_x + plot_w, y, glyphs::V_LINE.to_string(), 1, theme::border());
    }
    for k in 0..=5 {
        let y = axis_y - 1 - crate::cast!((f32::from(plot_h - 1) * crate::cast!(k => f32) / 5.0).round() => u16);
        let count = crate::cast!((y_max * crate::cast!(k => f32) / 5.0).round() => u32);
        buf.set_stringn(inner.x, y, format!("{count:>5}"), 5, theme::dim_text());
        buf.set_stringn(plot_x - 1, y, glyphs::CROSS.to_string(), 1, theme::border());
        buf.set_stringn(plot_x + plot_w, y, format!("{}{:>4}%", glyphs::CROSS, k * 20), crate::cast!(R_AXIS_W => usize), Style::default().fg(theme::VEGETATION).bg(theme::PANEL_BG));
    }
    let axis: String = std::iter::repeat_n(glyphs::H_LINE, crate::cast!(plot_w => usize) + 1).collect();
    buf.set_stringn(plot_x - 1, axis_y, &axis, crate::cast!(plot_w => usize) + 1, theme::border());
    if n > 0 {
        for k in 0..=6 {
            let i = ((k * (n - 1)).div_euclid(6)).min(n - 1);
            let x = if k == 6 { plot_x + plot_w - 1 } else { plot_x + crate::cast!(((i * cols).div_euclid(n)) => u16) };
            buf.set_stringn(x, axis_y, glyphs::CROSS.to_string(), 1, theme::border());
            let label = w.day_label(i);
            let lx = crate::cast!((i32::from(x) - crate::cast!(label.len() => i32).div_euclid(2)).max(i32::from(inner.x)) => u16);
            let lx = lx.min(inner.right().saturating_sub(crate::cast!(label.len() => u16)));
            buf.set_stringn(lx, axis_y + 1, &label, label.len(), theme::dim_text());
        }
    }
    let note_y = axis_y + 3;
    let note = theme::dim_text();
    let per_col = crate::cast!(n => f32) / crate::cast!(cols.max(1) => f32);
    buf.set_stringn(inner.x + 1, note_y, format!("Species are stacked bottom-up in table order (voles, hares, deer); each column averages ~{per_col:.1} days."), crate::cast!(inner.width => usize) - 2, note);
    buf.set_stringn(inner.x + 1, note_y + 1, "The vegetation dot uses the right-hand scale; the prey stack follows the grass with a lag.", crate::cast!(inner.width => usize) - 2, note);
}

/// Per-column stacked prey populations and vegetation means.
fn stacked_bins(w: &Window<'_>, cols: usize) -> (Vec<[f32; 3]>, Vec<f32>) {
    let n = w.len();
    let mut stack: Vec<[f32; 3]> = Vec::with_capacity(cols);
    let mut veg: Vec<f32> = Vec::with_capacity(cols);
    for c in 0..cols {
        let mut v = [0.0f32; 3];
        if n > 0 {
            let (a, b) = w.bin(c, cols);
            for (k, item) in v.iter_mut().enumerate() {
                *item = w.samples[a..b].iter().map(|s| crate::cast!(s.population[k] => f32)).sum::<f32>() / crate::cast!((b - a) => f32);
            }
            veg.push(w.mean_over(&w.veg, c, cols));
        } else {
            veg.push(0.0);
        }
        stack.push(v);
    }
    (stack, veg)
}

/// Draw the stacked columns, the vegetation dots and the drought banding.
/// Returns the x of the first drought column, if any.
#[allow(clippy::too_many_arguments)]
fn stacked_columns(buf: &mut Buffer, w: &Window<'_>, plot_x: u16, plot_y: u16, plot_h: u16, cols: usize, y_max: f32, halves: usize, band: Color, n: usize, stack: &[[f32; 3]], veg: &[f32]) -> Option<u16> {
    let mut band_x: Option<u16> = None;
    for (c, v) in stack.iter().enumerate() {
        let x = plot_x + crate::cast!(c => u16);
        let mut lower: Vec<Option<Color>> = vec![None; crate::cast!(plot_h => usize)];
        let mut upper: Vec<Option<Color>> = vec![None; crate::cast!(plot_h => usize)];
        let mut cum = 0.0f32;
        for (k, id) in SpeciesId::ALL.iter().take(3).enumerate() {
            let h0 = crate::cast!((cum / y_max * crate::cast!(halves => f32)).round() => usize);
            cum += v[k];
            let h1 = crate::cast!((cum / y_max * crate::cast!(halves => f32)).round() => usize);
            for h in h0..h1.min(halves) {
                let row = crate::cast!(plot_h => usize) - 1 - h.div_euclid(2);
                if h % 2 == 0 {
                    lower[row] = Some(id.color());
                } else {
                    upper[row] = Some(id.color());
                }
            }
        }
        let dry = n > 0 && w.drought_in(c, cols);
        if dry && band_x.is_none() {
            band_x = Some(x);
        }
        let empty_bg = if dry { band } else { theme::PANEL_BG };
        for r in 0..crate::cast!(plot_h => usize) {
            let y = plot_y + crate::cast!(r => u16);
            let Some(cell) = buf.cell_mut((x, y)) else { continue };
            match (lower[r], upper[r]) {
                (Some(lo), Some(up)) if lo == up => {
                    cell.set_char(glyphs::FULL_BLOCK);
                    cell.set_style(Style::default().fg(lo).bg(lo));
                }
                (Some(lo), Some(up)) => {
                    cell.set_char(glyphs::HALF_LOWER);
                    cell.set_style(Style::default().fg(lo).bg(up));
                }
                (Some(lo), None) => {
                    cell.set_char(glyphs::HALF_LOWER);
                    cell.set_style(Style::default().fg(lo).bg(empty_bg));
                }
                (None, Some(up)) => {
                    cell.set_char(glyphs::HALF_UPPER);
                    cell.set_style(Style::default().fg(up).bg(empty_bg));
                }
                (None, None) => {
                    cell.set_char(' ');
                    cell.set_style(Style::default().bg(empty_bg));
                }
            }
        }
        if n > 0 {
            let vh = crate::cast!((veg[c].clamp(0.0, 1.0) * (f32::from(plot_h) - 1.0)).round() => usize);
            let r = crate::cast!(plot_h => usize) - 1 - vh;
            if let Some(cell) = buf.cell_mut((x, plot_y + crate::cast!(r => u16))) {
                let bg = match (lower[r], upper[r]) {
                    (Some(lo), _) => lo,
                    (None, Some(up)) => up,
                    _ => empty_bg,
                };
                cell.set_char(glyphs::DOT);
                cell.set_style(Style::default().fg(theme::VEGETATION).bg(bg).add_modifier(Modifier::BOLD));
            }
        }
    }
    band_x
}

pub(super) fn stacked_sidebar(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw(f, area, "Composition", panel::Kind::Outer);
    let mut row = 0u16;
    row = stacked_today(f, inner, row, sim, w);
    row = stacked_peaks(f, inner, row, w);
    row = stacked_trend(f, inner, row, sim);

    panel::section(f, inner, row, "How to read");
    row += 1;
    for line in [
        " the top edge is the whole prey population",
        " each band is one species, voles at the bottom",
        " the green dot is mean vegetation (right axis)",
        " a shaded band marks days with a drought",
        " [+/-] widens or narrows the time window",
    ] {
        util::line(f, inner, row, Line::from(sp(line, theme::dim_text())));
        row += 1;
    }
}

/// Today's per-species counts and the share bar.
fn stacked_today(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, w: &Window<'_>) -> u16 {
    panel::section(f, inner, row, "Today");
    row += 1;
    util::line(f, inner, row, Line::from(sp("   species   count  share   240d range", theme::dim_text())));
    row += 1;
    let total: u32 = sim.species.iter().take(3).map(|s| s.count).sum();
    for (k, id) in SpeciesId::ALL.iter().take(3).enumerate() {
        let s = &sim.species[k];
        let share = if total > 0 { crate::cast!(s.count => f32) / crate::cast!(total => f32) * 100.0 } else { 0.0 };
        let series: Vec<u32> = w.samples.iter().map(|x| x.population[k]).collect();
        let (lo, hi) = (series.iter().min().copied().unwrap_or(0), series.iter().max().copied().unwrap_or(0));
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", glyphs::FULL_BLOCK), Style::default().fg(id.color()).bg(theme::PANEL_BG)),
            sp(format!("{:<8}{:>6}  {:>4.0}%   {}-{}", id.plural(), s.count, share, lo, hi), theme::text()),
        ]));
        row += 1;
    }
    let veg = w.veg.last().copied().unwrap_or(0.0);
    util::line(f, inner, row, Line::from(sp(format!("   total   {:>6}          veg {:.0}%", total, veg * 100.0), theme::dim_text())));
    row += 2;

    panel::section(f, inner, row, "Share today");
    row += 1;
    {
        let width = crate::cast!(inner.width.saturating_sub(2) => usize);
        let mut used = 0usize;
        let buf = f.buffer_mut();
        let y = inner.y + row;
        for (k, id) in SpeciesId::ALL.iter().take(3).enumerate() {
            let s = &sim.species[k];
            let cells = if k == 2 { width.saturating_sub(used) } else if total > 0 { (crate::cast!(s.count => usize) * width).div_euclid(crate::cast!(total => usize)) } else { 0 };
            let seg: String = std::iter::repeat_n(glyphs::FULL_BLOCK, cells).collect();
            buf.set_stringn(inner.x + 1 + crate::cast!(used => u16), y, &seg, cells, Style::default().fg(id.color()).bg(theme::PANEL_BG));
            used += cells;
        }
    }
    row += 1;
    util::line(f, inner, row, Line::from(sp(" prey 100%   predators 0%", theme::dim_text())));
    row += 2;
    row
}

/// Peak/smallest stacks, vegetation range and the drought summary.
fn stacked_peaks(f: &mut Frame<'_>, inner: Rect, mut row: u16, w: &Window<'_>) -> u16 {
    panel::section(f, inner, row, "Peaks");
    row += 1;
    if let Some((min, imin, max, imax, _)) = stats_of(&w.prey) {
        util::line(f, inner, row, Line::from(sp(format!(" largest stack  {:>5.0}  {}", max, w.day_label(imax)), theme::text())));
        row += 1;
        util::line(f, inner, row, Line::from(sp(format!(" smallest stack {:>5.0}  {}", min, w.day_label(imin)), theme::text())));
        row += 1;
    }
    if let Some((vmin, _, vmax, _, _)) = stats_of(&w.veg) {
        util::line(f, inner, row, Line::from(sp(format!(" vegetation     {:>4.0}% .. {:.0}%", vmin * 100.0, vmax * 100.0), Style::default().fg(theme::VEGETATION).bg(theme::PANEL_BG))));
        row += 1;
    }
    row += 1;

    let dry_days = w.drought.iter().filter(|&&d| d).count();
    if dry_days > 0 {
        panel::section(f, inner, row, "Drought");
        row += 1;
        let first = w.drought.iter().position(|&d| d).unwrap_or(0);
        let last = w.drought.iter().rposition(|&d| d).unwrap_or(0);
        util::line(f, inner, row, Line::from(sp(format!(" {} {} to {} ({} days)", glyphs::DROUGHT, w.day_label(first), w.day_label(last), dry_days), Style::default().fg(theme::WARN).bg(theme::PANEL_BG))));
        row += 1;
        let before = w.prey.get(first.saturating_sub(1)).copied().unwrap_or(0.0);
        let after = w.prey.get((last + 20).min(w.len().saturating_sub(1))).copied().unwrap_or(0.0);
        util::line(f, inner, row, Line::from(sp(format!(" stack {before:.0} before, {after:.0} twenty days after"), theme::text())));
        row += 2;
    }
    row
}

/// The 30-day trend rows.
fn stacked_trend(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim) -> u16 {
    panel::section(f, inner, row, "30-day trend");
    row += 1;
    for (k, id) in SpeciesId::ALL.iter().take(3).enumerate() {
        let s = &sim.species[k];
        let a = s.trend.first().copied().unwrap_or(0);
        let b = s.trend.last().copied().unwrap_or(0);
        let arrow = trend_arrow(&s.trend);
        let pct = s.change_pct().map_or_else(|| "–".into(), |p| format!("{p:+.0}%"));
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", id.glyph().to_ascii_uppercase()), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{:<7}{:>5} {} {:<5}", id.plural(), a, glyphs::RIGHT, b), theme::text()),
            sp(format!(" {arrow} {pct}"), Style::default().fg(arrow_color(arrow)).bg(theme::PANEL_BG)),
        ]));
        row += 1;
    }
    row += 1;
    row
}
