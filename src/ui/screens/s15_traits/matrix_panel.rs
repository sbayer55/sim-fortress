//! The S15a main panel: species and crowding strips, the trait × outcome
//! matrix, then (in `ages`) the selected trait across age and the key.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::{ages, bold, capital, fg, frac, helps, intensity, put, signed, View};
use crate::glyphs;
use crate::sim::species::{N_TRAITS, TRAIT_NAMES};
use crate::sim::stats::outcomes::{Crowding, MIN_N, N_OUTCOMES, OUTCOMES};
use crate::theme;
use crate::ui::style::SpeciesStyle;
use crate::widgets::{Component, Divider, FilterStrip, Histogram, Kind, Panel};

/// Inner column of the first matrix cell, and the cell pitch (7 cells + a gap).
const X0: u16 = 31;
const CW: u16 = 8;
/// Inner column of the living histogram and the trait mean.
const HIST_X: u16 = 13;
const MEAN_X: u16 = 25;
/// Inner row of the first trait row; each trait takes two rows.
const ROW0: u16 = 3;

pub(super) fn draw(buf: &mut Buffer, area: Rect, v: &View<'_>) {
    let living = v.lives.iter().filter(|l| !l.dead()).count();
    let title = format!("Traits and outcomes: {}", capital(&v.plural()));
    let info = format!("{} lives in 240 days, {living} alive now", v.lives.len());
    let inner = Panel::new(title).kind(Kind::Focus).info(info).render(buf, area);
    if inner.width < X0 + CW * 9 || inner.height < 40 {
        return;
    }
    strips(buf, inner, v);
    header(buf, inner, v);
    for t in 0..N_TRAITS {
        trait_row(buf, inner, v, t);
    }
    ages::draw(buf, inner, v);
}

fn strips(buf: &mut Buffer, inner: Rect, v: &View<'_>) {
    let roster = v.roster();
    let ids: Vec<_> = roster.ids().collect();
    let chips: Vec<(char, &str)> = ids.iter().map(|&id| (roster.adult_glyph(id), roster.name(id))).collect();
    let active: Vec<bool> = ids.iter().map(|&id| id == v.species).collect();
    let colors: Vec<_> = ids.iter().map(|&id| roster.color(id)).collect();
    let hint = format!("[1-{}] species", ids.len().clamp(1, 9));
    let split = 66.min(inner.width);
    FilterStrip::new(&chips).active(&active).colors(&colors).hint(hint).render(buf, Rect::new(inner.x, inner.y, split, 1));
    let days: Vec<(char, &str)> = Crowding::ALL.iter().map(|c| (' ', c.label())).collect();
    let on = Crowding::ALL.map(|c| c == v.crowding);
    FilterStrip::new(&days).active(&on).hint("[c] days").render(buf, Rect::new(inner.x + split, inner.y, inner.width - split, 1));
}

fn header(buf: &mut Buffer, inner: Rect, v: &View<'_>) {
    let (x, y) = (inner.x, inner.y);
    Divider::new("what they achieved").render(buf, Rect::new(x + X0, y + 1, 3 * CW - 1, 1));
    Divider::new("how they died: share of deaths").render(buf, Rect::new(x + X0 + 3 * CW, y + 1, 6 * CW - 1, 1));
    let head = Style::default().fg(theme::HEADER_FG).bg(theme::HEADER_BG);
    put(buf, x, y + 2, inner.width, &" ".repeat(usize::from(inner.width)), head);
    put(buf, x + 1, y + 2, 11, "trait", head.add_modifier(ratatui::style::Modifier::BOLD));
    put(buf, x + HIST_X, y + 2, 10, "living", head);
    put(buf, x + MEAN_X, y + 2, 5, "mean", head);
    for (o, outcome) in OUTCOMES.iter().enumerate() {
        let h = outcome.header(v.kind);
        let cx = x + X0 + crate::cast!(o => u16) * CW;
        let pad = (CW - 1).saturating_sub(crate::cast!(h.chars().count() => u16)).div_euclid(2);
        let color = if o == v.outcome { theme::KEY } else { theme::HEADER_FG };
        put(buf, cx + pad, y + 2, CW - 1, h, bold(color).bg(theme::HEADER_BG));
    }
}

fn trait_row(buf: &mut Buffer, inner: Rect, v: &View<'_>, t: usize) {
    let (x, y) = (inner.x, inner.y + ROW0 + 2 * crate::cast!(t => u16));
    let on = t == v.trait_ix;
    if on {
        crate::widgets::util::fill(buf, Rect::new(x, y, X0 - 1, 2), Style::default().bg(theme::SELECT_BG));
        put(buf, x, y, 1, &glyphs::CUE.to_string(), bold(theme::BORDER_FOCUS));
    }
    let name = TRAIT_NAMES.get(t).copied().unwrap_or("");
    put(buf, x + 1, y, 11, name, if on { bold(theme::TEXT_BRIGHT) } else { fg(theme::TEXT) });
    let (lo, hi) = range(v, t);
    let mut buckets = [0u16; 10];
    let mut sum = 0.0;
    let mut n = 0u16;
    for l in v.lives.iter().filter(|l| !l.dead()) {
        let g = l.genome.0.get(t).copied().unwrap_or(0.0);
        let b = crate::cast!(((g - lo) / (hi - lo) * 10.0).floor().clamp(0.0, 9.0) => usize);
        if let Some(c) = buckets.get_mut(b) {
            *c = c.saturating_add(1);
        }
        sum += g;
        n = n.saturating_add(1);
    }
    Histogram::new(&buckets).rows(2).col_w(1).color(v.color).render(buf, Rect::new(x + HIST_X, y, 10, 2));
    if n > 0 {
        put(buf, x + MEAN_X, y, 4, &frac(sum / f32::from(n)), fg(if on { theme::TEXT_BRIGHT } else { theme::TEXT }));
    }
    for o in 0..N_OUTCOMES {
        cell(buf, x + X0 + crate::cast!(o => u16) * CW, y, v, t, o);
    }
}

/// One 7 × 2 matrix cell: `r` on top, an arrow under it, the ground coloured by effect.
fn cell(buf: &mut Buffer, cx: u16, y: u16, v: &View<'_>, t: usize, o: usize) {
    let stat = v.matrix.cell(t, o);
    let outcome = OUTCOMES.get(o).copied().unwrap_or(OUTCOMES[0]);
    let shown = stat.shown();
    let mag = if shown { intensity(stat.r) } else { 0.0 };
    let signed_mag = match helps(stat.r, outcome) {
        Some(true) => mag,
        Some(false) => -mag,
        None => 0.0,
    };
    crate::widgets::util::fill(buf, Rect::new(cx, y, CW - 1, 2), Style::default().bg(theme::diverge_bg(signed_mag)));
    let sel = t == v.trait_ix && o == v.outcome;
    let color = if sel || mag > 0.6 { theme::TEXT_BRIGHT } else if mag > 0.15 { theme::TEXT } else { theme::DIM };
    let style = if sel || mag > 0.6 { bold(color) } else { fg(color) };
    let txt = if shown { signed(stat.r) } else if stat.flat && stat.n >= MIN_N { "none".to_string() } else { "n/a".to_string() };
    let pad = (CW - 1).saturating_sub(crate::cast!(txt.chars().count() => u16)).div_euclid(2);
    put(buf, cx + pad, y, CW - 1, &txt, style);
    if shown && mag > 0.15 {
        let arrow = if stat.r > 0.0 { glyphs::UP } else { glyphs::DOWN };
        put(buf, cx + 3, y + 1, 1, &arrow.to_string(), fg(color));
    }
    if sel {
        for dy in 0..2 {
            put(buf, cx - 1, y + dy, 1, &glyphs::HALF_RIGHT.to_string(), fg(theme::BORDER_FOCUS));
            put(buf, cx + CW - 1, y + dy, 1, &glyphs::HALF_LEFT.to_string(), fg(theme::BORDER_FOCUS));
        }
    }
}

/// The histogram range for a trait: the lives' min and max, widened to the
/// nearest .05 and to at least .10.
pub(super) fn range(v: &View<'_>, t: usize) -> (f32, f32) {
    let vals = v.lives.iter().filter_map(|l| l.genome.0.get(t).copied());
    let (mn, mx) = vals.fold((1.0f32, 0.0f32), |(a, b), g| (a.min(g), b.max(g)));
    if mn > mx {
        return (0.0, 1.0);
    }
    let lo = (mn * 20.0).floor() / 20.0;
    let hi = ((mx * 20.0).ceil() / 20.0).max(lo + 0.1);
    (lo, hi)
}
