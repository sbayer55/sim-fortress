//! One lane: the hunter column, the ribbon and the prey column (spec, Lane anatomy).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::sim::creatures::Goal;
use crate::sim::hunt_watch::HuntOutcome;
use crate::ui::screens::common::wrap;
use crate::ui::style::SpeciesStyle;
use crate::widgets::{Bar, Component, Ribbon, RibbonEnd};
use crate::{glyphs, theme};

use super::model::{challenge_word, need_word, HuntView, Status};
use super::{bold, fg, put, HUNTER_X, LANE_H, LANE_W, PREY_X, RIBBON_W, RIBBON_X, RULE_L, RULE_R};

/// Width of the hunter and prey columns' value field.
const VALUE_W: u16 = 28;
/// The bars in both columns.
const BAR_W: u16 = 27;

/// The `row <n> · free` filler on an empty slot.
pub(super) fn draw_free(buf: &mut Buffer, inner: Rect, y0: u16, slot: usize) {
    let bg = theme::lerp(theme::PANEL_BG, theme::HEADER_BG, 0.3);
    fill_row(buf, inner, y0, bg);
    put(buf, inner.x + HUNTER_X, y0, 20, &format!("row {} · free", slot + 1), Style::default().fg(theme::DIM).bg(bg));
}

pub(super) fn draw(buf: &mut Buffer, inner: Rect, y0: u16, v: &HuntView<'_>, selected: bool) {
    let roster = v.sim.roster();
    let bg = if selected { theme::SELECT_BG } else { theme::lerp(theme::PANEL_BG, theme::HEADER_BG, 0.6) };
    fill_row(buf, inner, y0, bg);
    if selected {
        put(buf, inner.x, y0, 1, &glyphs::CUE.to_string(), bold(theme::KEY).bg(bg));
    }
    let hc = roster.color(v.hunter.species);
    let pin = if v.pinned { format!(" {}", glyphs::DIAMOND) } else { String::new() };
    put(buf, inner.x + HUNTER_X, y0, 34, &format!("{}{pin}", v.hunter.label(roster)), bold(hc).bg(bg));
    for r in 0..LANE_H {
        for x in [RULE_L, RULE_R] {
            put(buf, inner.x + x, y0 + r, 1, &glyphs::V_LINE.to_string(), fg(theme::BORDER));
        }
    }
    hunter_column(buf, inner, y0, v);
    ribbon(buf, inner, y0, v);
    prey_column(buf, inner, y0, v, bg);
}

fn fill_row(buf: &mut Buffer, inner: Rect, y: u16, bg: ratatui::style::Color) {
    for x in inner.x..inner.x + LANE_W.min(inner.width) {
        buf[(x, y)].set_char(' ').set_style(Style::default().bg(bg));
    }
}

fn hunter_column(buf: &mut Buffer, inner: Rect, y0: u16, v: &HuntView<'_>) {
    let x = inner.x + HUNTER_X;
    let vx = x + 7;
    let hc = v.sim.roster().color(v.hunter.species);
    label(buf, x, y0 + 1, "energy");
    Bar::new(v.hunter.energy).color(hc).render(buf, Rect::new(vx, y0 + 1, BAR_W, 1));
    let (need, nc) = need_word(v.hunter.hunger, v.sim.params.predation.hunt_hunger_min);
    label(buf, x, y0 + 2, "need");
    put(buf, vx, y0 + 2, VALUE_W, need, bold(nc));
    label(buf, x, y0 + 3, "state");
    let (state, sc) = v.state_word();
    let n = put(buf, vx, y0 + 3, VALUE_W, &state, bold(sc));
    if v.hunting() && !v.packmates.is_empty() {
        put(buf, vx + n + 1, y0 + 3, VALUE_W.saturating_sub(n + 1), &format!("· pack of {}", v.packmates.len() + 1), fg(theme::MAGENTA));
    }
    label(buf, x, y0 + 4, "where");
    put(buf, vx, y0 + 4, VALUE_W, v.region(), fg(theme::TEXT));
}

fn ribbon(buf: &mut Buffer, inner: Rect, y0: u16, v: &HuntView<'_>) {
    let (stalk, chase) = v.ribbon_values();
    let end = v.trace.end().map(|e| match e.outcome {
        HuntOutcome::Kill { .. } => RibbonEnd::Kill,
        HuntOutcome::Miss => RibbonEnd::Escaped,
        HuntOutcome::Timeout => RibbonEnd::TimedOut,
        HuntOutcome::Lost | HuntOutcome::Dropped => RibbonEnd::Lost,
    });
    let idle = v.idle_sentence();
    let mut r = Ribbon::new(&stalk, &chase).outcome(end).live(v.hunting()).chase_max(usize::try_from(v.sim.params.predation.chase_max_ticks).unwrap_or(30));
    if !idle.is_empty() {
        r = r.caption(idle);
    }
    r.render(buf, Rect::new(inner.x + RIBBON_X, y0, RIBBON_W.min(inner.width.saturating_sub(RIBBON_X)), LANE_H));
}

fn prey_column(buf: &mut Buffer, inner: Rect, y0: u16, v: &HuntView<'_>, title_bg: ratatui::style::Color) {
    let roster = v.sim.roster();
    let x = inner.x + PREY_X;
    let vx = x + 7;
    let w = inner.right().saturating_sub(x);
    if v.hunting() {
        let Some(q) = v.prey else { return };
        let qc = roster.color(q.species);
        put(buf, x, y0, w, &q.label(roster), bold(qc).bg(title_bg));
        label(buf, x, y0 + 1, "energy");
        Bar::new(q.energy).color(qc).render(buf, Rect::new(vx, y0 + 1, BAR_W, 1));
        label(buf, x, y0 + 2, "level");
        if let Some(o) = v.odds {
            let (word, c) = challenge_word(o.total);
            put(buf, vx, y0 + 2, VALUE_W, word, bold(c));
        }
        label(buf, x, y0 + 3, "goal");
        let (goal, gc) = match q.goal {
            Goal::Flee => ("fleeing", theme::WARN),
            Goal::Wary => ("wary", theme::WARN),
            _ => ("grazing, unaware", theme::GOOD),
        };
        put(buf, vx, y0 + 3, VALUE_W, goal, fg(gc));
        label(buf, x, y0 + 4, "gap");
        if let Some(gap) = v.gap() {
            let literal: String = std::iter::once(roster.adult_glyph(v.hunter.species))
                .chain(std::iter::repeat_n(glyphs::DOT, usize::from(gap.clamp(1, 10)) - 1))
                .chain(std::iter::once(roster.adult_glyph(q.species)))
                .collect();
            let hot = gap <= 2;
            put(buf, vx, y0 + 4, 14, &format!("{gap}  {literal}"), if hot { bold(theme::BAD) } else { fg(theme::TEXT) });
        }
        if let Some(chase) = v.chase_ticks() {
            let left = u64::from(v.sim.params.predation.chase_max_ticks).saturating_sub(chase);
            let hot = left <= 6;
            put(buf, vx + 14, y0 + 4, 12, &format!("{left} left"), if hot { bold(theme::BAD) } else { fg(theme::DIM) });
        }
    } else {
        let dead = v.status == Status::Dead;
        put(buf, x, y0, w, if dead { "dead" } else { "no prey" }, if dead { bold(theme::BAD).bg(title_bg) } else { fg(theme::DIM).bg(title_bg) });
        let prey = v.prey.map_or_else(|| "its prey".to_string(), |q| q.label(roster));
        put(buf, x, y0 + 1, w, &format!("last: {prey}"), fg(theme::DIM));
        if let Some(end) = v.trace.end() {
            let (word, c) = match end.outcome {
                HuntOutcome::Kill { .. } => ("taken", theme::BAD),
                HuntOutcome::Miss => ("escaped", theme::GOOD),
                HuntOutcome::Timeout => ("outlasted", theme::GOOD),
                HuntOutcome::Lost => ("lost", theme::GOOD),
                HuntOutcome::Dropped => ("dropped", theme::DIM),
            };
            put(buf, x, y0 + 2, w, &format!("{word} {}", super::model::hour_label(v.sim, end.tick)), fg(c));
        }
        if let Some(last) = super::beats::beats(v).into_iter().rev().find(|b| b.tick <= v.sim.time.tick) {
            for (i, line) in wrap(&last.text, usize::from(w)).into_iter().take(2).enumerate() {
                put(buf, x, y0 + 3 + crate::cast!(i => u16), w, &line, fg(theme::DIM));
            }
        }
    }
}

fn label(buf: &mut Buffer, x: u16, y: u16, text: &str) {
    put(buf, x, y, 7, text, theme::dim_text());
}
