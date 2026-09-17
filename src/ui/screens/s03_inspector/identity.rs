//! S03a/c — the identity column: family, location, vitals, death and timeline.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::buffer::Buffer;
use crate::sim::creatures::{adult_age_days, Creature, Goal};
use crate::sim::disease::{PathogenId, Stage};
use crate::sim::Kind;
use crate::ui::screens::common::day_stamp;
use crate::ui::style::SpeciesStyle;
use crate::widgets::map::{self};
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};
use super::{contagion_risk, killer_and_scavengers, kin_name, local_forage};
use super::style::sp;

/// Panel title: the identity column changes name when the creature is dead.
pub(super) const fn identity_title(c: &Creature) -> &'static str {
    if c.alive { "Identity & Vitals" } else { "Identity & Death" }
}

/// Draw the identity column into `inner`; returns the rows used.
pub(super) fn identity(buf: &mut Buffer, inner: Rect, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    let mut row = 0u16;

    // Name line.
    let state = if !c.alive {
        sp("  DEAD", Style::default().fg(theme::BAD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
    } else if sim.roster().kind(c.species) == Kind::Predator {
        sp("  predator", Style::default().fg(sim.roster().color(c.species)).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
    } else {
        sp("  prey", Style::default().fg(theme::GOOD).bg(theme::PANEL_BG))
    };
    util::line_in(buf, inner, row, Line::from(vec![
        sp(format!(" {} ", if c.alive { sim.roster().adult_glyph(c.species) } else { glyphs::CARCASS }), sim.roster().style(c.species)),
        sp(c.name_str(sim.roster()).to_string(), theme::title()),
        sp(format!("  {}", c.tag(sim.roster())), theme::label()),
        state,
    ]));
    row += 1;
    let (sex_g, sex_name) = match c.sex {
        crate::sim::Sex::Male => (glyphs::MALE, "male"),
        crate::sim::Sex::Female => (glyphs::FEMALE, "female"),
    };
    util::line_in(buf, inner, row, Line::from(vec![
        sp("   ", theme::text()),
        sp(sim.roster().display_name(c.species), sim.roster().style(c.species)),
        sp(format!("  {sex_g} {sex_name}"), theme::text()),
        sp(format!("  {}", if c.adult { "adult" } else { "juvenile" }), theme::text()),
        sp(if c.sterile { ", sterile" } else { "" }, Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
        sp(format!("  diet: {}", sim.species_params(c.species).diet), theme::dim_text()),
    ]));
    row += 2;

    // Age bar (live: age from born_day, max from genome).
    let max_age = c.max_age_days(&sim.params.creatures, &sim.params.genetics);
    let age = c.age_days(sim.time.day_index());
    let age_t = crate::cast!(age => f32) / crate::cast!(max_age.max(1) => f32);
    let age_color = if c.alive { bars::vital_color(1.0 - age_t * 0.8, false) } else { theme::DIM };
    bars::labeled(buf, inner, row, " age", age_t, age_color, 6, 20);
    buf.set_stringn(inner.x + 34, inner.y + row, format!("{age} / {max_age} days"), 16, theme::dim_text());
    row += 1;
    util::line_in(buf, inner, row, Line::from(vec![
        sp(format!("       {:.1} years old", crate::cast!(age => f32) / 360.0), theme::dim_text()),
        sp(format!("   generation {}", c.generation), theme::text()),
    ]));
    row += 2;

    row = identity_family(buf, inner, row, sim, c);
    row = identity_location(buf, inner, row, sim, c);
    row = identity_state(buf, inner, row, sim, c);
    identity_timeline(buf, inner, row, sim, c)
}

/// The identity panel's family rows.
fn identity_family(buf: &mut Buffer, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    panel::section_in(buf, inner, row, "Family");
    row += 1;
    let (mother, father) = match c.parents {
        Some((m, fa)) => (kin_name(sim, m), kin_name(sim, fa)),
        None => ("founder".to_string(), "founder".to_string()),
    };
    util::line_in(buf, inner, row, Line::from(vec![
        sp(" mother  ", theme::dim_text()),
        sp(format!("{mother:<18}"), theme::text()),
        sp(" father  ", theme::dim_text()),
        sp(father, theme::text()),
    ]));
    row += 1;
    let pregnant = c.pregnant_due.map(|d| format!("   pregnant, due in {} h", d.saturating_sub(sim.time.tick))).unwrap_or_default();
    util::line_in(buf, inner, row, Line::from(vec![
        sp(" offspring  ", theme::dim_text()),
        sp(format!("{}", c.offspring), theme::text()),
        sp(pregnant, Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        sp(format!("    {} lineage: [l]", glyphs::NOTE), theme::dim_text()),
    ]));
    row += 2;
    row
}

/// The identity panel's location block.
fn identity_location(buf: &mut Buffer, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    panel::section_in(buf, inner, row, "Location");
    row += 1;
    let cell = sim.world.cell(c.x, c.y);
    let (tg, tfg, _) = map::terrain_cell(cell, false);
    util::line_in(buf, inner, row, Line::from(vec![
        sp(format!(" ({}, {})  ", c.x, c.y), theme::text()),
        sp(sim.world.region_name(c.x, c.y), theme::title()),
        sp(format!("   {tg} "), Style::default().fg(tfg).bg(theme::PANEL_BG)),
        sp(cell.terrain.name(), theme::dim_text()),
    ]));
    row += 1;
    if c.alive {
        let goal = c.goal.label(sim.world.dens.iter().any(|&(x, y)| x == c.x && y == c.y));
        util::line_in(buf, inner, row, Line::from(vec![sp(" goal    ", theme::dim_text()), sp(goal, theme::text())]));
        row += 1;
        let target = match c.target {
            Some((tx, ty)) => {
                let d = crate::sim::dist(c.x, c.y, tx, ty);
                format!("{} ({}, {})  {:.0} cells", glyphs::DIAMOND, tx, ty, d)
            }
            None => "none".to_string(),
        };
        util::line_in(buf, inner, row, Line::from(vec![sp(" target  ", theme::dim_text()), sp(target, theme::label())]));
        row += 1;
        let trail: Vec<String> = c.trail.iter().rev().take(5).map(|(x, y)| format!("({x},{y})")).collect();
        util::line_in(buf, inner, row, Line::from(vec![
            sp(" trail   ", theme::dim_text()),
            sp(if trail.is_empty() { "no recent movement".to_string() } else { trail.join(" ") }, theme::dim_text()),
        ]));
        row += 1;
    }
    row += 1;
    row
}

/// Vitals, condition and behaviour (living) or the death summary (dead).
fn identity_state(buf: &mut Buffer, inner: Rect, row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    if c.alive {
        identity_vitals(buf, inner, row, sim, c)
    } else {
        let age = c.age_days(sim.time.day_index());
        let max_age = c.max_age_days(&sim.params.creatures, &sim.params.genetics);
        let age_t = crate::cast!(age => f32) / crate::cast!(max_age.max(1) => f32);
        identity_death(buf, inner, row, sim, c, age, max_age, age_t)
    }
}

/// Vitals, condition and behaviour for a living creature.
fn identity_vitals(buf: &mut Buffer, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    panel::section_in(buf, inner, row, "Vitals");
    row += 1;
    for (label, v, inv) in [("health", c.hp, false), ("hunger", c.hunger, true), ("thirst", c.thirst, true), ("energy", c.energy, false)] {
        bars::labeled(buf, inner, row, &format!(" {label}"), v, bars::vital_color(v, inv), 9, 24);
        row += 1;
    }
    // C7: sickness, immunity and parasite load.
    let today = crate::cast!(sim.time.day_index() => u32);
    let sick_style = Style::default().fg(theme::SICK).bg(theme::PANEL_BG);
    let sickness = match c.infection {
        Some(i) if i.stage == Stage::Infectious => {
            let n = today.saturating_sub(i.since_day) + 1;
            let m = i.ends_day.saturating_sub(i.since_day);
            sp(format!("{} {} infectious day {}/~{} sev .{:02}", glyphs::DISEASE, sim.disease.name(i.pathogen), n, m, crate::cast!((i.severity * 100.0).round() => u32) % 100), sick_style)
        }
        Some(i) => sp(format!("{} {} incubating (shows in {} days)", glyphs::DISEASE, sim.disease.name(i.pathogen), i.ends_day.saturating_sub(today)), sick_style),
        None => sp("healthy", theme::dim_text()),
    };
    util::line_in(buf, inner, row, Line::from(vec![sp(format!("{:<9}", " sickness"), theme::text()), sickness]));
    row += 1;
    let immune: Vec<String> = c
        .immune_until
        .iter()
        .enumerate()
        .filter(|(_, &until)| until > today)
        .map(|(slot, &until)| {
            let name = sim.disease.name(PathogenId(crate::cast!(slot => u8)));
            if until == u32::MAX { format!("{name} (for life)") } else { name.to_string() }
        })
        .collect();
    let immune_span = if immune.is_empty() {
        sp("none", theme::dim_text())
    } else {
        sp(format!("{} {}", glyphs::IMMUNE, immune.join(", ")), Style::default().fg(theme::IMMUNE).bg(theme::PANEL_BG))
    };
    util::line_in(buf, inner, row, Line::from(vec![sp(format!("{:<9}", " immune"), theme::text()), immune_span]));
    row += 1;
    let load = c.parasite_load;
    bars::labeled(buf, inner, row, &format!(" {} parasites", glyphs::PARASITE), load, theme::WARN, 12, 24);
    let note = if load < 0.2 { "light" } else if load < 0.5 { "heavy" } else { "severe" };
    buf.set_stringn(inner.x + 44, inner.y + row, note, 6, Style::default().fg(bars::vital_color(load, true)).bg(theme::PANEL_BG));
    row += 2;
    panel::section_in(buf, inner, row, "Condition");
    row += 1;
    bars::labeled(buf, inner, row, " predation risk", c.predation_risk, bars::vital_color(c.predation_risk, true), 17, 16);
    row += 1;
    let contagion = contagion_risk(sim, c);
    bars::labeled(buf, inner, row, " contagion risk", contagion, bars::vital_color(contagion, true), 17, 16);
    row += 1;
    // Local forage: mean vegetation within 3 cells.
    let forage = local_forage(sim, c.x, c.y);
    bars::labeled(buf, inner, row, " local forage", forage, theme::VEGETATION, 17, 16);
    row += 1;
    util::line_in(buf, inner, row, Line::from(vec![sp(" nearest water", theme::dim_text()), sp(" —", theme::text()), sp("   nearest den", theme::dim_text()), sp(" —", theme::text())]));
    row += 2;

    panel::section_in(buf, inner, row, "Behaviour");
    row += 1;
    if crate::sim::disease::is_infectious(c) {
        util::line_in(buf, inner, row, Line::from(vec![
            sp(format!(" {} ", glyphs::ALERT), Style::default().fg(theme::SICK).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp("sick — resting more, no mating", Style::default().fg(theme::SICK).bg(theme::PANEL_BG)),
        ]));
        row += 1;
    }
    util::line_in(buf, inner, row, Line::from(sp(goal_line(sim, c), theme::text())));
    row + 2
}

/// The Behaviour line: why the creature is doing what it is doing.
fn goal_line(sim: &crate::sim::Sim, c: &Creature) -> String {
    match c.goal {
        Goal::Drink => format!(" {} because thirst = {:.2}", c.goal.label(false), c.thirst),
        Goal::Graze => format!(" {} because hunger = {:.2}", c.goal.label(false), c.hunger),
        Goal::Rest => format!(" {} because energy = {:.2}", c.goal.label(false), c.energy),
        Goal::Mate => match c.mate_id {
            Some(m) => format!(" {} — heading for {}", c.goal.label(false), kin_name(sim, m)),
            None => format!(" {}", c.goal.label(false)),
        },
        Goal::Wary => match c.wary_by {
            Some((px, py, _)) => format!(" {} — a predator {:.0} cells away", c.goal.label(false), crate::sim::dist(c.x, c.y, px, py)),
            None => format!(" {}", c.goal.label(false)),
        },
        _ => {
            if !c.adult && c.mother.is_some_and(|m| sim.creatures.get(m).is_some_and(|m| m.alive)) && c.age_days(sim.time.day_index()) < sim.params.genetics.follow_mother_days {
                " wandering near its mother".to_string()
            } else {
                format!(" {}", c.goal.label(false))
            }
        }
    }
}

/// The death summary for a dead creature.
#[allow(clippy::too_many_arguments)]
fn identity_death(buf: &mut Buffer, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature, age: u32, max_age: u32, age_t: f32) -> u16 {
    panel::section_in(buf, inner, row, "Death");
    row += 1;
    let cause = c.death.map_or("unknown", |d| d.cause.label());
    let cause = match c.died_infected {
        Some(p) => format!("{} ({})", cause, sim.disease.name(p)),
        None => cause.to_string(),
    };
    util::line_in(buf, inner, row, Line::from(vec![
        sp(format!(" {} ", glyphs::DEATH), Style::default().fg(theme::BAD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(cause, theme::text()),
    ]));
    row += 1;
    util::line_in(buf, inner, row, Line::from(vec![
        sp(format!("   lived {age} of {max_age} days ({}% of lifespan)", crate::cast!((age_t * 100.0).round() => u32)), theme::dim_text()),
    ]));
    row += 2;
    bars::labeled(buf, inner, row, " decay", c.decay, theme::CARCASS, 12, 24);
    row += 1;
    let nutrition = 1.0 - c.decay;
    let kg = crate::cast!((c.genome.size() * 120.0 * nutrition).round() => u32);
    let gone_in = crate::cast!(((1.0 - c.decay) * crate::cast!(sim.params.creatures.carcass_decay_days => f32)).ceil() => u32);
    util::line_in(buf, inner, row, Line::from(vec![
        sp(format!("   {kg} kg of meat remaining; gone in ~{gone_in} days"), theme::dim_text()),
    ]));
    row += 2;
    killer_and_scavengers(buf, inner, row, sim, c) + 1
}

fn identity_timeline(buf: &mut Buffer, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    // Timeline (C4 FR10): born, adult, each litter, death.
    panel::section_in(buf, inner, row, "Timeline");
    row += 1;
    let season_days = sim.time.season_days;
    let mut events: Vec<(char, Color, i64, String)> = Vec::new();
    let born_text = match c.parents {
        Some((m, fa)) => format!("born to {} and {}", kin_name(sim, m), kin_name(sim, fa)),
        None => "placed as a founder".to_string(),
    };
    events.push((glyphs::BIRTH, theme::GOOD, i64::from(c.born_day), born_text));
    let adult_day = i64::from(c.born_day) + i64::from(adult_age_days(sim.species_params(c.species), &c.genome, &sim.params.genetics));
    if c.adult && adult_day >= 0 {
        events.push((glyphs::UP, theme::INFO, adult_day, "reached adulthood".to_string()));
    }
    if let Some(node) = sim.lineage.get(c.id) {
        let mut litters: Vec<(i64, usize)> = Vec::new();
        for k in &node.children {
            if let Some(kn) = sim.lineage.get(*k) {
                let d = i64::from(kn.born_day);
                match litters.iter_mut().find(|l| l.0 == d) {
                    Some(l) => l.1 += 1,
                    None => litters.push((d, 1)),
                }
            }
        }
        for (d, n) in litters {
            events.push((glyphs::BIRTH, theme::GOOD, d, format!("litter of {n}")));
        }
    }
    if let Some(i) = c.infection {
        events.push((glyphs::DISEASE, theme::SICK, i64::from(i.since_day), format!("fell ill with {}", sim.disease.name(i.pathogen))));
    }
    if c.infections_survived > 0 {
        let n = c.infections_survived;
        let when = c.death.map_or_else(|| crate::cast!(sim.time.day_index() => i64), |d| i64::from(d.day));
        events.push((glyphs::IMMUNE, theme::IMMUNE, when, format!("recovered {} time{}", n, if n == 1 { "" } else { "s" })));
    }
    if let Some(d) = c.death {
        events.push((glyphs::DEATH, theme::BAD, i64::from(d.day), format!("died of {}", d.cause.label())));
    }
    events.sort_by_key(|e| e.2);
    for (g, color, d, text) in events {
        if row >= inner.height {
            break;
        }
        util::line_in(buf, inner, row, Line::from(vec![
            sp(format!(" {g} "), Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{:<9} ", day_stamp(d, season_days)), theme::dim_text()),
            sp(text, theme::text()),
        ]));
        row += 1;
    }
    row
}
