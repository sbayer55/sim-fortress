//! S03 — the life column: minimap, hunt/survival stats, legacy and kin.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::buffer::Buffer;
use crate::sim::creatures::{Creature, CreatureId};
use crate::sim::disease::Stage;
use crate::sim::{Kind, SpeciesId, TRAIT_NAMES};
use crate::ui::screens::common::day_stamp;
use crate::ui::style::{EventKindStyle, SpeciesStyle};
use crate::widgets::map::{self, MapOptions, Overlay};
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};
use super::{clip, compass};
use super::style::{sp, species_style};

/// Most kin rows shown (the panel scrolls, so this is a courtesy cap).
const KIN_ROWS: usize = 10;

/// Most recent-event rows shown.
const EVENT_ROWS: usize = 20;

/// Draw the life column into `inner`; returns the rows used.
pub(super) fn life(buf: &mut Buffer, inner: Rect, sim: &crate::sim::Sim, c: &Creature, id: CreatureId) -> u16 {
    let mut row = life_minimap(buf, inner, sim, c, id);
    row = life_stats(buf, inner, row, sim, c);
    row = life_legacy(buf, inner, row, sim, c, id);
    life_kin(buf, inner, row, sim, c, id)
}

/// The surroundings mini-map and the life-stat lines beside it.
fn life_minimap(buf: &mut Buffer, inner: Rect, sim: &crate::sim::Sim, c: &Creature, id: CreatureId) -> u16 {
    let mut row = 0u16;
    // Mini-map top-right.
    let mm_w = 23u16;
    let mm_h = 9u16;
    let mm = Rect::new(inner.right() - mm_w, inner.y, mm_w, mm_h);
    let mm_inner = panel::draw_in(buf, mm, "Surroundings", panel::Kind::Inner);
    let ox = c.x.saturating_sub(10).min(sim.world.width() - crate::cast!(mm_inner.width => usize));
    let oy = c.y.saturating_sub(3).min(sim.world.height() - crate::cast!(mm_inner.height => usize));
    let opts = MapOptions {
        overlay: Overlay::None,
        night: false,
        winter: false,
        cursor: Some((c.x, c.y)),
        follow: if c.alive { Some(id) } else { None },
        origin: (ox, oy),
        creatures: true,
        fade_creatures: false,
        selected_region: None,
        species_color: theme::TEXT,
        creature_tint: None,
    };
    map::render(buf, mm_inner, sim, &opts);

    // Life stats to the left of the mini-map.
    let stats = Rect::new(inner.x, inner.y, inner.width - mm_w - 1, mm_h);
    let age = c.age_days(sim.time.day_index());
    let lines = vec![
        Line::from(vec![sp(" days alive  ", theme::dim_text()), sp(format!("{age}"), theme::text())]),
        Line::from(vec![sp(" offspring   ", theme::dim_text()), sp(format!("{}", c.offspring), theme::text())]),
        Line::from(vec![sp(" distance    ", theme::dim_text()), sp(format!("{} cells", c.trail.len()), theme::text())]),
    ];
    for (i, l) in lines.into_iter().enumerate() {
        util::line_in(buf, stats, crate::cast!(i => u16), l);
    }
    row += mm_h + 1;
    row
}

/// Hunt stats (predator) or survival stats (prey).
fn life_stats(buf: &mut Buffer, inner: Rect, row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    if !c.alive {
        return row;
    }
    if c.species.kind() == Kind::Predator {
        hunt_stats(buf, inner, row, sim, c)
    } else {
        survival_stats(buf, inner, row, c)
    }
}

/// S03b: hunt outcomes, prey preference and chase history.
fn hunt_stats(buf: &mut Buffer, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    // S03b Hunt stats.
    panel::section_in(buf, inner, row, "Hunt stats");
    row += 1;
    let success = if c.attempts > 0 { crate::cast!(c.kills => f32) / crate::cast!(c.attempts => f32) * 100.0 } else { 0.0 };
    util::line_in(buf, inner, row, Line::from(sp(format!(" kills {}  attempts {}  success {:.0}%", c.kills, c.attempts, success), theme::text())));
    row += 1;
    bars::labeled(buf, inner, row, " success rate", success / 100.0, c.species.color(), 16, 16);
    row += 1;
    panel::section_in(buf, inner, row, "Preferred prey");
    row += 1;
    for prey_id in SpeciesId::ALL.iter().filter(|s| s.kind() == Kind::Prey) {
        let share = if c.kills >= 5 {
            crate::cast!(c.kills_by_species[prey_id.index()] => f32) / crate::cast!(c.kills.max(1) => f32)
        } else {
            sim.params.predation.preference(c.species, *prey_id)
        };
        bars::labeled(buf, inner, row, &format!(" {}", prey_id.plural()), share, prey_id.color(), 16, 16);
        buf.set_stringn(
            inner.x + 40,
            inner.y + row,
            format!("{} kills", c.kills_by_species[prey_id.index()]),
            12,
            theme::dim_text(),
        );
        row += 1;
    }
    let last_kill = match c.last_kill {
        Some((victim, day, region)) => {
            let vname = sim
                .creatures
                .get(victim)
                .map(|v| format!("{} {}", v.name_str(), v.tag()))
                .or_else(|| sim.lineage.get(victim).map(|n| format!("{} {}", n.name_str(), n.tag)))
                .unwrap_or_else(|| format!("#{}", victim.0));
            let region_name = sim.world.regions.get(crate::cast!(region => usize)).map_or("?", |r| r.0.as_str());
            format!("{}  {}, {}", vname, day_stamp(i64::from(day), sim.time.season_days), region_name)
        }
        None => "none".to_string(),
    };
    util::line_in(buf, inner, row, Line::from(vec![sp(" last kill ", theme::dim_text()), sp(last_kill, theme::text())]));
    row += 1;
    if let Some(o) = c.hunt_target.and_then(|t| sim.creatures.get(t)) {
        let d = crate::sim::dist(c.x, c.y, o.x, o.y);
        let mut spans = vec![sp(" current target ", theme::dim_text()), sp(format!("{} {}, {:.0} cells", o.name_str(), o.tag(), d), theme::label())];
        if let Some(i) = o.infection.filter(|i| i.stage == Stage::Infectious) {
            let bonus = sim.params.disease.kill_sick_bonus * i.severity;
            spans.push(sp(format!(" (+.{:02} sick prey)", crate::cast!((bonus * 100.0).round() => u32) % 100), Style::default().fg(theme::SICK).bg(theme::PANEL_BG)));
        }
        util::line_in(buf, inner, row, Line::from(spans));
        row += 1;
    }
    let avg = if c.attempts > 0 { c.chase_stats.0.div_euclid(c.attempts.max(1)) } else { 0 };
    util::line_in(buf, inner, row, Line::from(vec![
        sp(" avg chase ", theme::dim_text()),
        sp(format!("{avg} ticks; longest {} (Year {})", c.chase_stats.1, c.chase_longest_year), theme::text()),
    ]));
    row += 2;
    row
}

/// S03a: escape record and the predators this prey has seen.
fn survival_stats(buf: &mut Buffer, inner: Rect, mut row: u16, c: &Creature) -> u16 {
    // S03a Survival.
    panel::section_in(buf, inner, row, "Survival");
    row += 1;
    let escape_rate = if c.chased > 0 { crate::cast!(c.escaped => f32) / crate::cast!(c.chased => f32) } else { 0.0 };
    util::line_in(buf, inner, row, Line::from(sp(format!(" chased {} times, escaped {} ({:.0}%)", c.chased, c.escaped, escape_rate * 100.0), theme::text())));
    row += 1;
    util::line_in(buf, inner, row, Line::from(sp(format!(" grew wary of predators {} times", c.wary_count), theme::text())));
    row += 1;
    bars::labeled(buf, inner, row, " escape rate", escape_rate, theme::GOOD, 16, 16);
    row += 1;
    panel::section_in(buf, inner, row, "Threats seen");
    row += 1;
    let mut any = false;
    let total_threats: u32 = c.threats_by_species.iter().sum();
    for pred_id in SpeciesId::ALL.iter().filter(|s| s.kind() == Kind::Predator) {
        let n = c.threats_by_species[pred_id.index()];
        if n == 0 {
            continue;
        }
        any = true;
        let share = crate::cast!(n => f32) / crate::cast!(total_threats.max(1) => f32);
        bars::labeled(buf, inner, row, &format!(" {}", pred_id.plural()), share, pred_id.color(), 16, 16);
        buf.set_stringn(inner.x + 40, inner.y + row, format!("{n} times"), 12, theme::dim_text());
        row += 1;
    }
    if !any {
        util::line_in(buf, inner, row, Line::from(sp(" none seen yet", theme::dim_text())));
        row += 1;
    }
    row += 1;
    row
}

/// The legacy block: descendants and a carried notable mutation.
fn life_legacy(buf: &mut Buffer, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature, id: CreatureId) -> u16 {
    // Legacy (C4 FR10).
    panel::section_in(buf, inner, row, "Legacy");
    row += 1;
    let (living_desc, notable_desc) = sim.lineage.living_descendants(id, 5000);
    util::line_in(buf, inner, row, Line::from(vec![
        sp(format!(" {} offspring, {} living descendants, {} notable", c.offspring, living_desc, notable_desc), theme::text()),
    ]));
    row += 1;
    if let Some(node) = sim.lineage.get(id) {
        // A mutation carried on by descendants: the first notable one among them.
        let carried = sim
            .lineage
            .descendants(id, u32::MAX, 500)
            .into_iter()
            .filter_map(|d| sim.lineage.get(d))
            .find_map(|d| d.mutations.iter().find(|m| m.delta.abs() >= sim.params.genetics.mutation_notable).map(|m| (d.name_str(), d.tag.clone(), *m)));
        match carried {
            Some((name, tag, m)) => {
                util::line_in(buf, inner, row, Line::from(sp(format!(" {} {} {} carries {} {:+.2}", glyphs::MUTATION, name, tag, TRAIT_NAMES[m.trait_idx], m.delta), theme::dim_text())));
            }
            None if node.children.is_empty() => {
                util::line_in(buf, inner, row, Line::from(sp(" no descendants yet", theme::dim_text())));
            }
            None => {
                util::line_in(buf, inner, row, Line::from(sp(" no notable mutations among descendants", theme::dim_text())));
            }
        }
        row += 1;
    }
    row += 1;
    row
}

/// Nearby kin and the recent events for this creature.
fn life_kin(buf: &mut Buffer, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature, id: CreatureId) -> u16 {
    // Kin nearby (C4 FR10): parents, siblings and children within 15 cells.
    panel::section_in(buf, inner, row, "Kin nearby");
    row += 1;
    let mut kin: Vec<(f32, &Creature, &str)> = Vec::new();
    let sibling_of = |o: &Creature| c.parents.is_some() && o.parents.is_some() && (o.parents.map(|p| p.0) == c.parents.map(|p| p.0) || o.parents.map(|p| p.1) == c.parents.map(|p| p.1));
    for o in sim.creatures.living().filter(|o| o.id != id && o.species == c.species) {
        let rel = if c.parents.is_some_and(|p| p.0 == o.id) {
            "mother"
        } else if c.parents.is_some_and(|p| p.1 == o.id) {
            "father"
        } else if o.parents.is_some_and(|p| p.0 == id || p.1 == id) {
            "child"
        } else if sibling_of(o) {
            "sibling"
        } else {
            continue;
        };
        let d = crate::sim::dist(c.x, c.y, o.x, o.y);
        if d <= 15.0 {
            kin.push((d, o, rel));
        }
    }
    kin.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.id.cmp(&b.1.id)));
    if kin.is_empty() {
        util::line_in(buf, inner, row, Line::from(sp(" no kin within 15 cells", theme::dim_text())));
        row += 1;
    }
    for (d, o, rel) in kin.iter().take(KIN_ROWS) {
        if row >= inner.height {
            break;
        }
        let dir = compass(c.x, c.y, o.x, o.y);
        util::line_in(buf, inner, row, Line::from(vec![
            sp(format!(" {} ", if o.adult { o.species.glyph().to_ascii_uppercase() } else { o.species.glyph() }), species_style(o.species)),
            sp(format!("{:<8}{:<7}", o.name_str(), o.tag()), theme::text()),
            sp(format!("{d:>3.0} cells {dir:<2} "), theme::dim_text()),
            sp((*rel).to_string(), theme::label()),
        ]));
        row += 1;
    }
    row += 1;

    // Recent events filtered by subject.
    panel::section_in(buf, inner, row, "Recent events");
    row += 1;
    let evs: Vec<_> = sim.events.iter().rev().filter(|e| e.subject == Some(id)).take(EVENT_ROWS).collect();
    if evs.is_empty() {
        util::line_in(buf, inner, row, Line::from(sp(" no events for this creature", theme::dim_text())));
        row += 1;
    }
    for e in evs {
        let stamp = format!("Y{} D{:<3} {:02}h ", e.year, e.day, e.hour);
        let avail = crate::cast!(inner.width => usize) - 2 - stamp.len() - 2;
        let text = clip(&e.text, avail);
        util::line_in(buf, inner, row, Line::from(vec![
            sp(format!(" {} ", e.kind.glyph()), Style::default().fg(e.kind.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(stamp, theme::dim_text()),
            sp(text, theme::text()),
        ]));
        row += 1;
    }
    row
}
