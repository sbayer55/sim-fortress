//! S03 — the life column: minimap, hunt/survival stats, legacy and kin.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use crate::sim::creatures::{Creature, CreatureId};
use crate::sim::disease::Stage;
use crate::sim::{Kind, Sim, TRAIT_NAMES};
use crate::ui::screens::common::{ago, day_stamp};
use crate::ui::style::{EventKindStyle, SpeciesStyle};
use crate::widgets::map::{self, MapOptions, OverlayStack};
use crate::widgets::{Component, Kind as PanelKind, LabeledBar, Panel, Rows, Text};
use crate::{glyphs, theme};
use super::{clip, compass};
use super::style::{blank, line, one, section, sp};

/// Most kin rows shown (the panel scrolls, so this is a courtesy cap).
const KIN_ROWS: usize = 10;

/// Most recent-event rows shown.
const EVENT_ROWS: usize = 20;

/// Width of the Surroundings mini-map, border included.
const MINIMAP_W: u16 = 23;

/// Height of the Surroundings mini-map, border included.
const MINIMAP_H: u16 = 9;

/// The life column's rows; `width` is the column's inner width.
pub(super) fn rows<'a>(sim: &'a Sim, c: &'a Creature, id: CreatureId, width: u16) -> Rows<'a> {
    let mut rows: Rows<'a> = vec![Box::new(Header { sim, c, id })];
    if c.alive {
        rows.extend(if sim.roster().kind(c.species) == Kind::Predator { hunt_stats(sim, c) } else { survival_stats(sim, c) });
    }
    rows.extend(legacy(sim, c, id));
    rows.extend(kin(sim, c, id, width));
    rows
}

/// The surroundings mini-map top right and the life-stat lines beside it.
struct Header<'a> {
    sim: &'a Sim,
    c: &'a Creature,
    id: CreatureId,
}

impl std::fmt::Debug for Header<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Header").field("id", &self.id).finish()
    }
}

impl Component for Header<'_> {
    fn height(&self, _width: u16) -> u16 {
        MINIMAP_H + 1
    }

    fn min_width(&self) -> u16 {
        MINIMAP_W + 1
    }

    fn render(&self, buf: &mut Buffer, area: Rect) {
        let (sim, c) = (self.sim, self.c);
        let mm = Rect::new(area.right().saturating_sub(MINIMAP_W), area.y, MINIMAP_W.min(area.width), MINIMAP_H.min(area.height));
        let mm_inner = Panel::new("Surroundings").kind(PanelKind::Inner).render(buf, mm);
        let ox = c.x.saturating_sub(10).min(sim.world.width().saturating_sub(crate::cast!(mm_inner.width => usize)));
        let oy = c.y.saturating_sub(3).min(sim.world.height().saturating_sub(crate::cast!(mm_inner.height => usize)));
        let opts = MapOptions {
            stack: OverlayStack::default(),
            night: false,
            winter: false,
            cursor: Some((c.x, c.y)),
            follow: if c.alive { Some(self.id) } else { None },
            pins: Vec::new(),
            origin: (ox, oy),
            creatures: true,
            selected_region: None,
            species_color: theme::TEXT,
            creature_tint: None,
            succession: map::SuccessionScale::default(),
        };
        if !mm_inner.is_empty() {
            map::render(buf, mm_inner, sim, &opts);
        }
        let stats = Rect::new(area.x, area.y, area.width.saturating_sub(MINIMAP_W + 1), MINIMAP_H.min(area.height));
        let age = c.age_days(sim.time.day_index());
        let lines = [
            vec![sp(" days alive  ", theme::dim_text()), sp(format!("{age}"), theme::text())],
            vec![sp(" offspring   ", theme::dim_text()), sp(format!("{}", c.offspring), theme::text())],
            vec![sp(" distance    ", theme::dim_text()), sp(format!("{} cells", c.trail.len()), theme::text())],
            vec![sp(" last ate    ", theme::dim_text()), sp(ago(c.last_ate, sim.time.tick, sim.time.ticks_per_day), theme::text())],
            vec![sp(" last drank  ", theme::dim_text()), sp(ago(c.last_drank, sim.time.tick, sim.time.ticks_per_day), theme::text())],
            vec![sp(" last slept  ", theme::dim_text()), sp(ago(c.last_slept, sim.time.tick, sim.time.ticks_per_day), theme::text())],
        ];
        for (i, spans) in lines.into_iter().enumerate() {
            let y = stats.y + crate::cast!(i => u16);
            if y < stats.bottom() {
                Text::spans(spans).render(buf, Rect::new(stats.x, y, stats.width, 1));
            }
        }
    }
}

/// S03b: hunt outcomes, prey preference and chase history.
fn hunt_stats(sim: &Sim, c: &Creature) -> Rows<'static> {
    let success = if c.attempts > 0 { crate::cast!(c.kills => f32) / crate::cast!(c.attempts => f32) * 100.0 } else { 0.0 };
    let mut rows = vec![
        section("Hunt stats"),
        one(format!(" kills {}  attempts {}  success {:.0}%", c.kills, c.attempts, success), theme::text()),
        Box::new(LabeledBar::new(" success rate", success / 100.0).color(sim.roster().color(c.species)).label_and_bar(16, 16)),
        section("Preferred prey"),
    ];
    for prey_id in sim.roster().prey_ids() {
        let share = if c.kills >= 5 {
            crate::cast!(c.kills_by_species[prey_id.index()] => f32) / crate::cast!(c.kills.max(1) => f32)
        } else {
            sim.roster().preference(c.species, prey_id)
        };
        rows.push(Box::new(
            LabeledBar::new(format!(" {}", sim.roster().plural(prey_id)), share)
                .color(sim.roster().color(prey_id))
                .label_and_bar(16, 16)
                .suffix(format!("{} kills", c.kills_by_species[prey_id.index()]))
                .suffix_style(theme::dim_text()),
        ));
    }
    let last_kill = match c.last_kill {
        Some((victim, day, region)) => {
            let vname = sim
                .creatures
                .get(victim)
                .map(|v| v.label(sim.roster()))
                .or_else(|| sim.lineage.get(victim).map(|n| format!("{} {}", n.name_str(sim.roster()), n.tag)))
                .unwrap_or_else(|| format!("#{}", victim.0));
            let region_name = sim.world.regions.get(crate::cast!(region => usize)).map_or("?", |r| r.0.as_str());
            format!("{}  {}, {}", vname, day_stamp(i64::from(day), sim.time.season_days), region_name)
        }
        None => "none".to_string(),
    };
    rows.push(line(vec![sp(" last kill ", theme::dim_text()), sp(last_kill, theme::text())]));
    if let Some(o) = c.hunt_target.and_then(|t| sim.creatures.get(t)) {
        let d = crate::sim::dist(c.x, c.y, o.x, o.y);
        let mut spans = vec![sp(" current target ", theme::dim_text()), sp(format!("{} {}, {:.0} cells", o.name_str(sim.roster()), o.tag(sim.roster()), d), theme::label())];
        if let Some(i) = o.infection.filter(|i| i.stage == Stage::Infectious) {
            let bonus = sim.params.disease.kill_sick_bonus * i.severity;
            spans.push(sp(format!(" (+.{:02} sick prey)", crate::cast!((bonus * 100.0).round() => u32) % 100), Style::default().fg(theme::SICK).bg(theme::PANEL_BG)));
        }
        rows.push(line(spans));
    }
    let avg = if c.attempts > 0 { c.chase_stats.0.div_euclid(c.attempts.max(1)) } else { 0 };
    rows.push(line(vec![
        sp(" avg chase ", theme::dim_text()),
        sp(format!("{avg} ticks; longest {} (Year {})", c.chase_stats.1, c.chase_longest_year), theme::text()),
    ]));
    // C5 FR13: ground held on the species' scent grid and the contest record.
    let tp = &sim.params.territory;
    let held = sim.world.scent_block(c.species).iter().filter(|m| m.holder == c.id && m.strength >= tp.hold_min).count();
    let here = sim.world.mark(c.species, c.x, c.y);
    let standing = if here.holder == c.id && here.strength >= tp.hold_min { "resident here" } else { "off its ground" };
    rows.push(line(vec![sp(" territory ", theme::dim_text()), sp(format!("holds {held} cells · {standing}"), theme::text())]));
    rows.push(line(vec![sp(" contests ", theme::dim_text()), sp(format!("won {} lost {}", c.contests_won, c.contests_lost), theme::text())]));
    rows.push(blank(1));
    rows
}

/// S03a: escape record and the predators this prey has seen.
fn survival_stats(sim: &Sim, c: &Creature) -> Rows<'static> {
    let escape_rate = if c.chased > 0 { crate::cast!(c.escaped => f32) / crate::cast!(c.chased => f32) } else { 0.0 };
    let mut rows = vec![
        section("Survival"),
        one(format!(" chased {} times, escaped {} ({:.0}%)", c.chased, c.escaped, escape_rate * 100.0), theme::text()),
        one(format!(" grew wary of predators {} times", c.wary_count), theme::text()),
        Box::new(LabeledBar::new(" escape rate", escape_rate).color(theme::GOOD).label_and_bar(16, 16)),
        section("Threats seen"),
    ];
    let mut any = false;
    let total_threats: u32 = c.threats_by_species.iter().sum();
    for pred_id in sim.roster().predator_ids() {
        let n = c.threats_by_species[pred_id.index()];
        if n == 0 {
            continue;
        }
        any = true;
        let share = crate::cast!(n => f32) / crate::cast!(total_threats.max(1) => f32);
        rows.push(Box::new(
            LabeledBar::new(format!(" {}", sim.roster().plural(pred_id)), share)
                .color(sim.roster().color(pred_id))
                .label_and_bar(16, 16)
                .suffix(format!("{n} times"))
                .suffix_style(theme::dim_text()),
        ));
    }
    if !any {
        rows.push(one(" none seen yet", theme::dim_text()));
    }
    rows.push(blank(1));
    rows
}

/// The legacy block: descendants and a carried notable mutation.
fn legacy(sim: &Sim, c: &Creature, id: CreatureId) -> Rows<'static> {
    let (living_desc, notable_desc) = sim.lineage.living_descendants(id, 5000);
    let mut rows = vec![section("Legacy"), one(format!(" {} offspring, {} living descendants, {} notable", c.offspring, living_desc, notable_desc), theme::text())];
    if let Some(node) = sim.lineage.get(id) {
        // A mutation carried on by descendants: the first notable one among them.
        let carried = sim
            .lineage
            .descendants(id, u32::MAX, 500)
            .into_iter()
            .filter_map(|d| sim.lineage.get(d))
            .find_map(|d| d.mutations.iter().find(|m| m.delta.abs() >= sim.params.genetics.mutation_notable).map(|m| (d.name_str(sim.roster()), d.tag.clone(), *m)));
        rows.push(match carried {
            Some((name, tag, m)) => one(format!(" {} {} {} carries {} {:+.2}", glyphs::MUTATION, name, tag, TRAIT_NAMES[m.trait_idx], m.delta), theme::dim_text()),
            None if node.children.is_empty() => one(" no descendants yet", theme::dim_text()),
            None => one(" no notable mutations among descendants", theme::dim_text()),
        });
    }
    rows.push(blank(1));
    rows
}

/// Nearby kin and the recent events for this creature.
fn kin(sim: &Sim, c: &Creature, id: CreatureId, width: u16) -> Rows<'static> {
    // Kin nearby (C4 FR10): parents, siblings and children within 15 cells.
    let mut rows = vec![section("Kin nearby")];
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
        rows.push(one(" no kin within 15 cells", theme::dim_text()));
    }
    for (d, o, rel) in kin.iter().take(KIN_ROWS) {
        let dir = compass(c.x, c.y, o.x, o.y);
        rows.push(line(vec![
            sp(format!(" {} ", if o.adult { sim.roster().adult_glyph(o.species) } else { sim.roster().glyph(o.species) }), sim.roster().style(o.species)),
            sp(format!("{:<8}{:<7}", o.name_str(sim.roster()), o.tag(sim.roster())), theme::text()),
            sp(format!("{d:>3.0} cells {dir:<2} "), theme::dim_text()),
            sp((*rel).to_string(), theme::label()),
        ]));
    }
    rows.push(blank(1));

    // Recent events filtered by subject.
    rows.push(section("Recent events"));
    let evs: Vec<_> = sim.events.iter().rev().filter(|e| e.subject == Some(id)).take(EVENT_ROWS).collect();
    if evs.is_empty() {
        rows.push(one(" no events for this creature", theme::dim_text()));
    }
    for e in evs {
        let stamp = format!("Y{} D{:<3} {:02}h ", e.year, e.day, e.hour);
        let avail = usize::from(width).saturating_sub(4 + stamp.len());
        let text = clip(&e.text, avail);
        rows.push(line(vec![
            sp(format!(" {} ", e.kind.glyph()), Style::default().fg(e.kind.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(stamp, theme::dim_text()),
            sp(text, theme::text()),
        ]));
    }
    rows
}
