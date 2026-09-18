//! The lower part of the S15a main panel: the selected trait across age among
//! the living (population, mean trait, commonest death per band) and the key.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::{cause_mark, days, fg, frac, put, View, CAUSES};
use crate::glyphs;
use crate::sim::species::{TRAIT_ABBR, TRAIT_NAMES};
use crate::sim::stats::outcomes::{age_profile, age_trend, AGE_BANDS};
use crate::theme;
use crate::widgets::{Component, Divider, Histogram};

/// Inner row of the age section divider (right under the thirteenth trait
/// row), and the band column origin and pitch.
const TOP: u16 = 29;
const BX: u16 = 13;
const BC: u16 = 5;

pub(super) fn draw(buf: &mut Buffer, inner: Rect, v: &View<'_>) {
    let (x, y) = (inner.x, inner.y + TOP);
    let name = TRAIT_NAMES.get(v.trait_ix).copied().unwrap_or("");
    Divider::new(format!("{name} across ages, living {}", v.plural())).render(buf, Rect::new(x, y, inner.width, 1));
    let p = age_profile(v.lives, v.trait_ix);
    put(buf, x + 1, y + 1, 11, "age", fg(theme::DIM));
    for b in (0..AGE_BANDS).step_by(4) {
        let d = crate::cast!(b => f32) * crate::cast!(p.band_days => f32);
        put(buf, x + BX + crate::cast!(b => u16) * BC, y + 1, 16, &days(d), fg(theme::DIM));
    }
    put(buf, x + 1, y + 2, 12, "living now", fg(theme::TEXT));
    let living: Vec<u16> = p.bands.iter().map(|b| crate::cast!(b.living.min(u32::from(u16::MAX)) => u16)).collect();
    Histogram::new(&living).rows(2).col_w(BC).color(v.color).render(buf, Rect::new(x + BX, y + 2, BC * crate::cast!(AGE_BANDS => u16), 2));
    let abbr = TRAIT_ABBR.get(v.trait_ix).copied().unwrap_or("");
    put(buf, x + 1, y + 4, 12, &format!("mean {abbr}"), fg(theme::TEXT));
    let means: Vec<f32> = p.bands.iter().filter_map(|b| b.trait_mean).collect();
    let (mn, mx) = means.iter().fold((f32::MAX, f32::MIN), |(a, b), &m| (a.min(m), b.max(m)));
    put(buf, x + 1, y + 5, 12, "died of", fg(theme::TEXT));
    for (b, band) in p.bands.iter().enumerate() {
        let bx = x + BX + crate::cast!(b => u16) * BC;
        match band.trait_mean {
            Some(m) => {
                let t = if mx - mn > 0.005 { (m - mn) / (mx - mn) } else { 0.5 };
                put(buf, bx, y + 4, BC, &format!("{} ", frac(m)), Style::default().fg(theme::TEXT_BRIGHT).bg(theme::dim(theme::heat(t), 0.55)));
            }
            None => put(buf, bx + 1, y + 4, 1, &glyphs::DOT.to_string(), fg(theme::DIM)),
        }
        match band.top {
            Some((cause, n)) => {
                let (g, c) = cause_mark(cause);
                let pct = (n * 100).div_euclid(band.deaths.max(1));
                put(buf, bx, y + 5, BC - 1, &format!("{g}{pct}"), fg(c));
            }
            None => put(buf, bx + 1, y + 5, 1, &glyphs::DOT.to_string(), fg(theme::DIM)),
        }
    }
    let trend = match age_trend(v.lives, v.trait_ix) {
        Some(t) => {
            let verdict = if !t.significant() {
                "within noise, no sign of selection with age."
            } else if t.old > t.young {
                "the ones with more of it live longer."
            } else {
                "the ones with less of it live longer."
            };
            format!("Youngest fifth of the living average {}, oldest fifth {}: {verdict}", frac(t.young), frac(t.old))
        }
        None => "Too few living to compare the young with the old.".to_string(),
    };
    put(buf, x + 1, y + 6, inner.width - 2, &trend, fg(theme::DIM));
    key(buf, inner, y + 7);
}

fn key(buf: &mut Buffer, inner: Rect, y: u16) {
    let x = inner.x;
    let w = inner.width - 2;
    Divider::new("Key").render(buf, Rect::new(x, y, inner.width, 1));
    for i in 0..11u16 {
        let t = (f32::from(i) - 5.0) / 5.0;
        crate::widgets::util::fill(buf, Rect::new(x + 1 + i, y + 1, 1, 1), Style::default().bg(theme::diverge_bg(t)));
    }
    let hurts = format!("hurts {} {} helps   cell = r of trait and outcome   n/a = under 30 lives   none = all the same", glyphs::REWIND, glyphs::PLAY);
    put(buf, x + 14, y + 1, w - 13, &hurts, fg(theme::DIM));
    let arrows = format!("{} more of the outcome as the trait rises, {} less    crowded / sparse = population above / below its median", glyphs::UP, glyphs::DOWN);
    put(buf, x + 1, y + 2, w, &arrows, fg(theme::DIM));
    let lead = "died of = commonest death, % of band:  ";
    put(buf, x + 1, y + 3, w, lead, fg(theme::DIM));
    let mut cx = x + 1 + crate::cast!(lead.chars().count() => u16);
    for (c, label) in CAUSES.into_iter().zip(["starved", "thirst", "old age", "preyed on", "disease", "injury"]) {
        let room = inner.right().saturating_sub(cx + 1);
        if room < 3 {
            break;
        }
        let (g, color) = cause_mark(c);
        put(buf, cx, y + 3, 1, &g.to_string(), fg(color).add_modifier(ratatui::style::Modifier::BOLD));
        put(buf, cx + 2, y + 3, room - 2, label, fg(theme::DIM));
        cx += crate::cast!(label.chars().count() => u16) + 4;
    }
    put(buf, x + 1, y + 4, w, "OldAge is left uncoloured: within 240 days a long life means fewer deaths of old age, not more", fg(theme::DIM));
}
