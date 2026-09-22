//! The details panel under the lanes: the selected hunter, its prey and the
//! odds with the play-by-play (spec items 12–16).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::sim::creatures::Goal;
use crate::sim::disease;
use crate::sim::hunt_watch::HuntOutcome;
use crate::sim::predation::OddsParts;
use crate::sim::Sex;
use crate::ui::screens::common::wrap;
use crate::ui::style::SpeciesStyle;
use crate::widgets::{Component, Divider, LabeledBar};
use crate::{glyphs, theme};

use super::beats::beats;
use super::model::{challenge_word, frac, hour_label, need_word, other_prey_in_range, pct, reserves, stamina, terrain_word, HuntView, Status};
use super::{bold, fg, put};

const COL_A: u16 = 2;
const COL_B: u16 = 53;
const COL_C: u16 = 103;
const W_A: u16 = 49;
const W_B: u16 = 48;
const W_C: u16 = 49;

/// A column that wraps its lines and stops at the panel's last usable row.
struct Column<'b> {
    buf: &'b mut Buffer,
    x: u16,
    y: u16,
    w: u16,
    bottom: u16,
}

impl Column<'_> {
    fn line(&mut self, text: &str, style: Style) {
        for l in wrap(text, usize::from(self.w)) {
            if self.y >= self.bottom {
                return;
            }
            put(self.buf, self.x, self.y, self.w, &l, style);
            self.y += 1;
        }
    }
}

/// `dy` is the divider row; the panel runs to `bottom` (exclusive).
pub(super) fn draw(buf: &mut Buffer, inner: Rect, dy: u16, bottom: u16, v: Option<&HuntView<'_>>) {
    let title = v.map_or_else(
        || "Details".to_string(),
        |v| {
            let roster = v.sim.roster();
            if v.hunting() {
                format!("Details · {} {} {}", v.hunter.label(roster), glyphs::PLAY, v.prey.map_or_else(String::new, |q| q.label(roster)))
            } else {
                format!("Details · {} · {}", v.hunter.label(roster), v.state_word().0.to_lowercase())
            }
        },
    );
    Divider::new(title).render(buf, Rect::new(inner.x, dy, inner.width, 1));
    let Some(v) = v else {
        put(buf, inner.x + COL_A, dy + 2, 60, "no predator tracked: select a row when one appears", theme::dim_text());
        return;
    };
    let y = dy + 1;
    if y >= bottom {
        return;
    }
    let hunting = v.hunting();
    put(buf, inner.x + COL_A, y, W_A, "Hunter", bold(theme::ACCENT));
    put(buf, inner.x + COL_B, y, W_B, if hunting { "Prey" } else { "Last prey" }, bold(theme::ACCENT));
    let odds_title = match v.odds {
        Some(o) if hunting => format!("Odds at contact {}%", pct(o.total)),
        _ => "Last hunt".to_string(),
    };
    put(buf, inner.x + COL_C, y, W_C, &odds_title, bold(theme::ACCENT));
    hunter_column(&mut Column { buf, x: inner.x + COL_A, y: y + 1, w: W_A, bottom }, v);
    prey_column(&mut Column { buf, x: inner.x + COL_B, y: y + 1, w: W_B, bottom }, v);
    odds_column(buf, inner.x + COL_C, y + 1, bottom, v);
}

const fn sex_word(s: Sex) -> char {
    match s {
        Sex::Male => glyphs::MALE,
        Sex::Female => glyphs::FEMALE,
    }
}

fn hunter_column(col: &mut Column<'_>, v: &HuntView<'_>) {
    let sim = v.sim;
    let roster = sim.roster();
    let h = v.hunter;
    let hc = roster.color(h.species);
    col.line(&format!("{}  {} adult {}", h.label(roster), roster.get(h.species).name, sex_word(h.sex)), bold(hc));
    if col.y < col.bottom {
        let (need, nc) = need_word(h.hunger, sim.params.predation.hunt_hunger_min);
        LabeledBar::new("hunger", h.hunger).color(theme::WARN).label_and_bar(7, 20).suffix(need).suffix_style(bold(nc)).render(col.buf, Rect::new(col.x, col.y, col.w, 1));
        col.y += 1;
    }
    col.line(&format!("speed {}  aggression {}  sense {} cells", frac(h.genome.speed()), frac(h.genome.aggression()), h.genome.sense_cells()), fg(theme::TEXT));
    let rate = if h.attempts == 0 { 0 } else { pct(crate::cast!(h.kills => f32) / crate::cast!(h.attempts => f32)) };
    let avg = h.chase_stats.0.checked_div(h.kills.max(1)).unwrap_or(0);
    col.line(&format!("kills {} of {} attempts ({rate}%) · avg chase {avg}, longest {} (Y{})", h.kills, h.attempts, h.chase_stats.1, h.chase_longest_year), fg(theme::TEXT));
    history_line(col, v, hc);
    if let Some((id, day, region)) = h.last_kill {
        let who = sim.creatures.get(id).map_or_else(|| format!("prey #{}", id.0), |q| q.label(roster));
        let ago = sim.time.day_index().saturating_sub(u64::from(day));
        let where_ = sim.world.regions.get(usize::from(region)).map_or("", |r| r.0.as_str());
        col.line(&format!("last kill: {who}, {ago} days ago, {where_}"), theme::dim_text());
    }
    if v.hunting() && !v.packmates.is_empty() {
        let names: Vec<String> = v.packmates.iter().filter_map(|&id| sim.creatures.get(id)).map(|c| c.label(roster)).collect();
        col.line(&format!("pack of {}: {} (+{} each to the roll)", v.packmates.len() + 1, names.join(", "), frac(sim.params.social.pack_kill_bonus)), fg(theme::MAGENTA));
    } else {
        col.line("hunting alone: no pack bonus", theme::dim_text());
    }
    reserves_line(col, v);
    let others = other_prey_in_range(sim, h);
    let hunger_note = if h.hunger >= 0.8 {
        "starvation is days away"
    } else if h.hunger >= sim.params.predation.hunt_hunger_min {
        "hungry enough to hunt"
    } else {
        "below the hunting threshold"
    };
    col.line(&format!("other prey in range: {others} · {hunger_note}"), if h.hunger >= 0.8 { fg(theme::BAD) } else { fg(theme::TEXT) });
}

/// `last 12 · ■ · ■ ■ · …  n kills`: the hunter's attempt history, most recent first.
fn history_line(col: &mut Column<'_>, v: &HuntView<'_>, hc: ratatui::style::Color) {
    let history: Vec<bool> = v.trace.history().collect();
    if history.is_empty() {
        col.line("last 12 · no attempts recorded yet", theme::dim_text());
        return;
    }
    if col.y >= col.bottom {
        return;
    }
    put(col.buf, col.x, col.y, 8, "last 12 ", theme::dim_text());
    for (i, &kill) in history.iter().enumerate() {
        let x = col.x + 8 + crate::cast!(i => u16) * 2;
        put(col.buf, x, col.y, 1, &(if kill { glyphs::SQUARE } else { glyphs::DOT }).to_string(), if kill { bold(hc) } else { fg(theme::DIM) });
    }
    let kills = history.iter().filter(|&&k| k).count();
    put(col.buf, col.x + 33, col.y, col.w.saturating_sub(33), &format!("{kills} kills"), fg(theme::TEXT));
    col.y += 1;
}

/// The hunter's energy as chase ticks, against the clock while chasing.
fn reserves_line(col: &mut Column<'_>, v: &HuntView<'_>) {
    let res = reserves(v.hunter.energy, &v.sim.params.creatures);
    let (tail, style) = match v.chase_ticks() {
        Some(_) => {
            let can = u64::from(res) >= v.left;
            (format!(" vs {} on the clock: {}", v.left, if can { "can finish" } else { "may tire first" }), if can { fg(theme::TEXT) } else { fg(theme::BAD) })
        }
        None => (String::new(), fg(theme::TEXT)),
    };
    col.line(&format!("reserves: {res} chase ticks in the legs{tail}"), style);
}

fn prey_column(col: &mut Column<'_>, v: &HuntView<'_>) {
    let sim = v.sim;
    let roster = sim.roster();
    let Some(q) = v.prey else {
        col.line("the prey is gone", theme::dim_text());
        return;
    };
    let h = v.hunter;
    let pp = &sim.params.predation;
    col.line(&format!("{}  {} adult {}", q.label(roster), roster.get(q.species).name, sex_word(q.sex)), bold(roster.color(q.species)));
    let edge = q.genome.speed() - h.genome.speed();
    let edge_word = if edge > 0.005 { format!("faster by {}", frac(edge)) } else if edge < -0.005 { format!("slower by {}", frac(-edge)) } else { "same speed".to_string() };
    col.line(&format!("speed edge: {edge_word} ({} vs {})", frac(q.genome.speed()), frac(h.genome.speed())), fg(theme::TEXT));
    let terrain = sim.world.cell(q.x, q.y).terrain;
    let cover = pp.cover_by_terrain.get(&terrain).copied().unwrap_or(0.0);
    let seen = crate::sim::predation::can_detect(h, q, &sim.world, pp);
    col.line(
        &format!("ground: {}, {} · cover {cover:.1} x camo {} = {} vs sense {}: {}", sim.world.region_name(q.x, q.y), terrain_word(terrain), frac(q.genome.camouflage()), frac(cover * q.genome.camouflage()), h.genome.sense_cells(), if seen { "seen" } else { "hidden" }),
        fg(theme::TEXT),
    );
    let record = if q.chased == 0 { "never chased before".to_string() } else { format!("escaped {} of {} chases", q.escaped, q.chased) };
    col.line(&format!("record: {record} · size {}", frac(q.genome.size())), fg(theme::TEXT));
    if v.hunting() {
        let run = stamina(q.energy, &sim.params.creatures, pp);
        let (vs, can) = match v.chase_ticks() {
            Some(_) => (format!("{} on the clock", v.left), u64::from(run) >= v.left),
            None => (format!("a {}-tick clock", pp.chase_max_ticks), u64::from(run) >= u64::from(pp.chase_max_ticks)),
        };
        col.line(&format!("stamina: {run} flight ticks vs {vs}: {}", if can { "can outlast" } else { "tiring" }), if can { fg(theme::TEXT) } else { fg(theme::BAD) });
    } else if let Some(end) = v.trace.end() {
        let word = match end.outcome {
            HuntOutcome::Kill { .. } => "taken at contact",
            HuntOutcome::Miss => "escaped at contact",
            HuntOutcome::Timeout => "outlasted the clock",
            HuntOutcome::Lost => "slipped out of sense range",
            HuntOutcome::Dropped => "hunt dropped",
        };
        col.line(&format!("outcome: {word} {}", hour_label(sim, end.tick)), fg(theme::TEXT));
    }
    if let Some(o) = v.odds {
        let (word, c) = challenge_word(o.total);
        col.line(&format!("challenge: {word} at {}% odds", pct(o.total)), bold(c));
    }
    if let Some(i) = q.infection.filter(|_| disease::is_infectious(q)) {
        col.line(&format!("sick with {}: +{} to the roll, a sick carcass", sim.disease.name(i.pathogen), frac(disease::effects(q, &sim.params.disease).kill_bonus)), fg(theme::SICK));
    } else {
        col.line(&format!("kin: {} nearby", q.kin_nearby), theme::dim_text());
    }
    let goal = match (v.hunting(), q.goal) {
        (true, Goal::Flee) => format!("fleeing, {} cells at {}x energy cost", pp.flee_distance.round(), pp.flee_energy_factor),
        (true, Goal::Wary) => "wary, edging away".to_string(),
        (true, _) => "grazing, unaware".to_string(),
        (false, _) if !q.alive => "dead".to_string(),
        (false, _) => format!("{}; back to its day", q.goal.plain()),
    };
    col.line(&format!("goal: {goal}"), theme::dim_text());
}

fn odds_column(buf: &mut Buffer, x: u16, y0: u16, bottom: u16, v: &HuntView<'_>) {
    let mut y = y0;
    if let Some(o) = v.odds.filter(|_| v.hunting()) {
        if y < bottom {
            odds_bar(buf, x, y, W_C, &o);
            y += 1;
        }
        let parts = format!(
            "base {} · speed {} · aggression {} · prey size {} · pack {} · sick {} = {}%",
            signed(o.base),
            signed(o.speed),
            signed(o.aggression),
            signed(o.size),
            signed(o.pack),
            signed(o.sick),
            pct(o.total)
        );
        let mut col = Column { buf, x, y, w: W_C, bottom };
        col.line(&parts, theme::dim_text());
        if let Some([gap, clock, legs]) = v.escape_routes() {
            col.line(&format!("escape by gap {}% · clock {}% · legs {}%", pct(gap), pct(clock), pct(legs)), fg(theme::TEXT));
        }
        y = col.y;
    } else if y < bottom {
        let text = match v.status {
            Status::Eating | Status::Fed | Status::Sated => "the hunter won at contact",
            Status::Dead => "no chase: the hunter is dead",
            Status::Hunting => "odds appear at the first contact",
            _ => "the prey won; no chase running",
        };
        put(buf, x, y, W_C, text, theme::dim_text());
        y += 1;
    }
    if y >= bottom {
        return;
    }
    Divider::new("Play by play").render(buf, Rect::new(x, y, W_C, 1));
    y += 1;
    let now = v.sim.time.tick;
    let lines: Vec<(u64, String, super::beats::Kind, bool)> = beats(v)
        .into_iter()
        .filter(|b| b.tick <= now)
        .flat_map(|b| {
            let kind = b.kind;
            wrap(&b.text, usize::from(W_C) - 5).into_iter().enumerate().map(move |(i, l)| (b.tick, l, kind, i == 0)).collect::<Vec<_>>()
        })
        .collect();
    let room = usize::from(bottom.saturating_sub(y));
    let start = lines.len().saturating_sub(room);
    for (tick, text, kind, first) in lines.into_iter().skip(start) {
        if first {
            let off = if tick == now { "now".to_string() } else { format!("-{}", now.saturating_sub(tick)) };
            put(buf, x, y, 4, &format!("{off:>3}"), theme::dim_text());
        }
        put(buf, x + 4, y, W_C - 4, &text, if kind.bold() { bold(kind.color()) } else { fg(kind.color()) });
        y += 1;
    }
}

/// `+.35`, `-.16`.
fn signed(v: f32) -> String {
    let sign = if v < 0.0 { '-' } else { '+' };
    format!("{sign}{}", frac(v.abs()))
}

/// The contact roll as a stacked bar: positives filled, the penalties hatched
/// from the right end of the positives, `│` at the clamped total (spec item 15).
fn odds_bar(buf: &mut Buffer, x: u16, y: u16, w: u16, o: &OddsParts) {
    let cells = |v: f32| crate::cast!((v.abs() * f32::from(w)).round() => u16);
    let bg = theme::PANEL_BG;
    let mut cx = 0u16;
    for (v, c) in [(o.base, theme::ROCK_FG), (o.speed.max(0.0), theme::INFO), (o.aggression, theme::WARN), (o.pack, theme::MAGENTA), (o.sick, theme::SICK)] {
        for _ in 0..cells(v) {
            if cx < w {
                buf[(x + cx, y)].set_char(glyphs::BAR_FILL).set_style(Style::default().fg(c).bg(bg));
                cx += 1;
            }
        }
    }
    let penalty = cells(o.size) + cells(o.speed.min(0.0));
    for i in 0..penalty {
        if let Some(px) = cx.checked_sub(1 + i) {
            buf[(x + px, y)].set_char(glyphs::SHADE_2).set_style(Style::default().fg(theme::BAD).bg(bg));
        }
    }
    for i in cx..w {
        buf[(x + i, y)].set_char(glyphs::SHADE_1).set_style(Style::default().fg(theme::DIM).bg(bg));
    }
    let mark = cells(o.total).min(w - 1);
    buf[(x + mark, y)].set_char(glyphs::V_LINE).set_style(Style::default().fg(theme::TEXT_BRIGHT).bg(bg));
}
