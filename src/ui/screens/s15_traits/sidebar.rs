//! The S15a sidebar: the selected cell explained — its correlation, the
//! outcome per trait third, a sentence, crowded against sparse days, the
//! strongest links for the species and its population.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::{bold, cause_mark, days, effect_color, fg, frac, helps, put, signed, strength, third_color, View, CAUSES};
use crate::glyphs;
use crate::sim::species::TRAIT_NAMES;
use crate::sim::stats::outcomes::{strongest, third_cuts, thirds, Outcome, MIN_N, OUTCOMES};
use crate::sim::Kind;
use crate::theme;
use crate::widgets::{Component, Divider, LabeledBar, Panel};

const THIRDS: [&str; 3] = ["low", "mid", "high"];

pub(super) fn draw(buf: &mut Buffer, area: Rect, v: &View<'_>) {
    let inner = Panel::new("Selected").render(buf, area);
    if inner.width < 30 || inner.height < 30 {
        return;
    }
    let mut y = inner.y;
    y = cell(buf, inner, y, v);
    y = by_third(buf, inner, y + 1, v);
    y = in_words(buf, inner, y + 1, v);
    y = crowding(buf, inner, y + 1, v);
    y = links(buf, inner, y + 1, v);
    population(buf, inner, y + 1, v);
}

fn section(buf: &mut Buffer, inner: Rect, y: u16, title: &str) -> u16 {
    Divider::new(title.to_string()).render(buf, Rect::new(inner.x, y, inner.width, 1));
    y + 1
}

/// `Speed against predation`, `r -.29  moderate, the trait helps`, `from 688 deaths`.
fn cell(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>) -> u16 {
    let (x, w) = (inner.x + 1, inner.width - 2);
    let y = section(buf, inner, y, "Cell");
    let name = TRAIT_NAMES.get(v.trait_ix).copied().unwrap_or("");
    let o = v.outcome();
    let label = o.label(v.kind);
    put(buf, x, y, w, name, bold(theme::TEXT_BRIGHT));
    let nx = x + crate::cast!(name.chars().count() => u16);
    put(buf, nx, y, w.saturating_sub(nx - x), " against ", fg(theme::DIM));
    put(buf, nx + 9, y, w.saturating_sub(nx + 9 - x), label, bold(theme::TEXT));
    let stat = v.matrix.cell(v.trait_ix, v.outcome);
    if stat.n < MIN_N {
        put(buf, x, y + 1, w, "too few lives to say (under 30)", fg(theme::DIM));
    } else if stat.flat {
        put(buf, x, y + 1, w, &format!("none: {}", flat_words(v, o)), fg(theme::DIM));
    } else {
        let color = if strength(stat.r) == "no link" { theme::DIM } else { effect_color(stat.r, o) };
        put(buf, x, y + 1, 7, &format!("r {}", signed(stat.r)), bold(color));
        let verdict = match (strength(stat.r), helps(stat.r, o)) {
            ("no link", _) => "no link".to_string(),
            (s, Some(true)) => format!("{s}, the trait helps"),
            (s, Some(false)) => format!("{s}, the trait hurts"),
            (s, None) => format!("{s}, a neutral outcome"),
        };
        put(buf, x + 8, y + 1, w - 8, &verdict, fg(theme::TEXT));
    }
    let of = if matches!(o, Outcome::Lifespan | Outcome::Died(_)) { "death" } else { "life" };
    let of = if stat.n == 1 { of.to_string() } else if of == "life" { "lives".to_string() } else { format!("{of}s") };
    put(buf, x, y + 2, w, &format!("from {} {of}", stat.n), fg(theme::DIM));
    y + 3
}

/// The outcome's value in the sidebar's four-cell Value slot.
fn value(o: Outcome, v: f32) -> String {
    match o {
        Outcome::Lifespan => days(v),
        Outcome::Young => format!("{v:.1}"),
        Outcome::Hunt | Outcome::Died(_) => format!("{}%", crate::cast!((v * 100.0).round() => u32)),
    }
}

fn by_third(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>) -> u16 {
    let x = inner.x + 1;
    let y = section(buf, inner, y, "By trait third");
    let o = v.outcome();
    let stats = thirds(v.lives, v.trait_ix, o);
    let (a, b) = third_cuts(v.lives, v.trait_ix);
    let top = stats.iter().map(|s| s.mean).fold(0.0f32, f32::max).max(1e-6);
    for (k, s) in stats.iter().enumerate() {
        let range = match k {
            0 => format!("<{}", frac(a)),
            1 => format!("{}-{}", frac(a), frac(b)),
            _ => format!(">{}", frac(b)),
        };
        let label = format!("{:<5}{range}", THIRDS.get(k).copied().unwrap_or(""));
        let text = if s.n > 0 { value(o, s.mean) } else { "-".to_string() };
        LabeledBar::new(label, s.mean / top)
            .label_w(15)
            .bar_w(16)
            .color(third_color(v.color, k))
            .value_text(text)
            .render(buf, Rect::new(x, y + crate::cast!(k => u16), inner.width - 2, 1));
    }
    y + 3
}

/// The selected cell as one sentence, comparing the top and bottom thirds.
fn sentence(v: &View<'_>) -> String {
    let o = v.outcome();
    let stat = v.matrix.cell(v.trait_ix, v.outcome);
    if stat.n < MIN_N {
        return format!("Too few {} lives count here yet to compare the thirds ({} of 30).", v.singular(), stat.n);
    }
    if stat.flat {
        let words = flat_words(v, o);
        return format!("{}{}, so the thirds cannot differ.", words.get(..1).unwrap_or("").to_uppercase(), words.get(1..).unwrap_or(""));
    }
    let t = thirds(v.lives, v.trait_ix, o);
    let (lo, hi) = (t[0].mean, t[2].mean);
    let pl = v.plural();
    let tn = TRAIT_NAMES.get(v.trait_ix).copied().unwrap_or("").to_lowercase();
    let pct = |x: f32| format!("{}%", crate::cast!((x * 100.0).round() => u32));
    match o {
        Outcome::Lifespan => format!("{} in the top third for {tn} lived {} on average; the bottom third lived {}.", super::capital(&pl), days(hi), days(lo)),
        Outcome::Young => format!("As adults, high-{tn} {pl} raised {hi:.1} young a year; low-{tn} {pl} raised {lo:.1}."),
        Outcome::Hunt => match v.kind {
            Kind::Prey => format!("High-{tn} {pl} escaped {} of chases; low-{tn} {pl} escaped {}.", pct(hi), pct(lo)),
            Kind::Predator => format!("High-{tn} {pl} killed in {} of hunts; low-{tn} {pl} in {}.", pct(hi), pct(lo)),
        },
        Outcome::Died(_) => format!("{} of high-{tn} deaths were {}, against {} for low-{tn} {pl}.", pct(hi), o.label(v.kind), pct(lo)),
    }
}

/// What a flat column means, in words: `no counted death was starvation`,
/// `every counted death was predation`, `all lives had 0.0 young per year`
/// (at most 33 cells, so the Cell line `none: …` fits the sidebar).
fn flat_words(v: &View<'_>, o: Outcome) -> String {
    let value = v.lives.iter().find_map(|l| o.value(l)).unwrap_or(0.0);
    match o {
        Outcome::Died(_) if value > 0.5 => format!("every counted death was {}", o.label(v.kind)),
        Outcome::Died(_) => format!("no counted death was {}", o.label(v.kind)),
        _ => format!("all lives had {} {}", value_words(o, value), o.label(v.kind)),
    }
}

/// An outcome value in prose.
fn value_words(o: Outcome, v: f32) -> String {
    match o {
        Outcome::Lifespan => days(v),
        Outcome::Young => format!("{v:.1}"),
        Outcome::Hunt | Outcome::Died(_) => format!("a {}%", crate::cast!((v * 100.0).round() => u32)),
    }
}

fn in_words(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>) -> u16 {
    let mut y = section(buf, inner, y, "In words");
    for line in wrap(&sentence(v), usize::from(inner.width - 2)).iter().take(4) {
        put(buf, inner.x + 1, y, inner.width - 2, line, fg(theme::TEXT));
        y += 1;
    }
    y
}

use crate::ui::screens::common::wrap;

/// The same cell's r on all days, crowded days and sparse days, as bars either side of zero.
fn crowding(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>) -> u16 {
    let x = inner.x + 1;
    let mut y = section(buf, inner, y, "Crowding");
    let o = v.outcome();
    let mid = x + 21;
    for (i, c) in crate::sim::stats::outcomes::Crowding::ALL.into_iter().enumerate() {
        let stat = v.all.matrices.get(i).map(|m| m.cell(v.trait_ix, v.outcome)).unwrap_or_default();
        let on = c == v.crowding;
        put(buf, x, y, 12, c.label(), if on { bold(theme::TEXT_BRIGHT) } else { fg(theme::TEXT) });
        put(buf, mid, y, 1, &glyphs::V_LINE.to_string(), fg(theme::DIM));
        if stat.shown() {
            let n = crate::cast!((stat.r.abs() / 0.6).clamp(0.0, 1.0) * 9.0 => f32).round();
            let color = effect_color(stat.r, o);
            let blocks: String = std::iter::repeat_n(glyphs::FULL_BLOCK, crate::cast!(n => usize)).collect();
            let bx = if stat.r > 0.0 { mid + 1 } else { mid - crate::cast!(n => u16) };
            put(buf, bx, y, crate::cast!(n => u16), &blocks, fg(color));
            put(buf, x + 33, y, 5, &signed(stat.r), fg(theme::TEXT));
        } else {
            put(buf, x + 33, y, 5, "n/a", fg(theme::DIM));
        }
        y += 1;
    }
    put(buf, x + 12, y, 20, &format!("less {}   {} more", glyphs::REWIND, glyphs::PLAY), fg(theme::DIM));
    y + 1
}

fn links(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>) -> u16 {
    let x = inner.x + 1;
    let mut y = section(buf, inner, y, &format!("Strongest links, {}", v.plural()));
    for (t, o, r) in strongest(v.matrix, 6) {
        let outcome = OUTCOMES.get(o).copied().unwrap_or(Outcome::Lifespan);
        if t == v.trait_ix && o == v.outcome {
            crate::widgets::util::fill(buf, Rect::new(inner.x, y, inner.width, 1), Style::default().bg(theme::SELECT_BG));
        }
        put(buf, x, y, 13, TRAIT_NAMES.get(t).copied().unwrap_or(""), fg(theme::TEXT));
        let arrow = if r > 0.0 { glyphs::UP } else { glyphs::DOWN };
        let color = effect_color(r, outcome);
        put(buf, x + 13, y, 21, &format!("{arrow} {}", outcome.label(v.kind)), fg(color));
        put(buf, x + 34, y, 5, &signed(r), fg(theme::TEXT));
        y += 1;
    }
    y
}

fn population(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>) {
    let x = inner.x + 1;
    let y = section(buf, inner, y, "Population");
    let living = v.lives.iter().filter(|l| !l.dead()).count();
    put(buf, x, y, 16, &format!("alive now {living}"), fg(theme::TEXT));
    put(buf, x + 17, y, 20, &format!("died {}", v.lives.len() - living), fg(theme::TEXT));
    if y + 1 >= inner.bottom() {
        return;
    }
    let counts: Vec<usize> = CAUSES.iter().map(|&c| v.lives.iter().filter(|l| l.fate == crate::sim::stats::outcomes::Fate::Died(c)).count()).collect();
    let total: usize = counts.iter().sum();
    let w = usize::from(inner.width - 2);
    if total == 0 {
        put(buf, x, y + 1, inner.width - 2, "no deaths in the window", fg(theme::DIM));
        return;
    }
    let mut cx = x;
    let mut acc = 0usize;
    for (c, n) in CAUSES.iter().zip(counts) {
        let end = ((acc + n) * w).div_euclid(total);
        let start = (acc * w).div_euclid(total);
        acc += n;
        let cells = end - start;
        let blocks: String = std::iter::repeat_n(glyphs::FULL_BLOCK, cells).collect();
        put(buf, cx, y + 1, crate::cast!(cells => u16), &blocks, fg(cause_mark(*c).1));
        cx += crate::cast!(cells => u16);
    }
}
