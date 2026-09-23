//! S03a/c — the identity column: family, location, vitals, death and timeline.

use ratatui::style::{Color, Modifier, Style};
use crate::sim::creatures::{Creature, Goal};
use crate::sim::params::QuirkTier;
use crate::sim::disease::{PathogenId, Stage};
use crate::sim::{Kind, Sim};
use crate::ui::screens::common::day_stamp;
use crate::ui::style::SpeciesStyle;
use crate::widgets::map;
use crate::widgets::{bars, LabeledBar, Rows};
use crate::{glyphs, theme};
use super::{contagion_risk, killer_and_scavengers_rows, kin_name, local_forage};
use super::style::{blank, line, one, section, sp};

/// Panel title: the identity column changes name when the creature is dead.
pub(super) const fn identity_title(c: &Creature) -> &'static str {
    if c.alive { "Identity & Vitals" } else { "Identity & Death" }
}

/// The identity column's rows.
pub(super) fn rows(sim: &Sim, c: &Creature, pinned: bool) -> Rows<'static> {
    let mut rows = Rows::new();
    let state = if !c.alive {
        sp("  DEAD", Style::default().fg(theme::BAD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
    } else if sim.roster().kind(c.species) == Kind::Predator {
        sp("  predator", Style::default().fg(sim.roster().color(c.species)).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
    } else {
        sp("  prey", Style::default().fg(theme::GOOD).bg(theme::PANEL_BG))
    };
    rows.push(line(vec![
        sp(format!(" {} ", if c.alive { sim.roster().adult_glyph(c.species) } else { glyphs::CARCASS }), sim.roster().style(c.species)),
        sp(c.name_str(sim.roster()).to_string(), theme::title()),
        sp(format!("  {}", c.tag(sim.roster())), theme::label()),
        sp(if pinned { format!(" {}", glyphs::DIAMOND) } else { String::new() }, Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        quirk_badge(sim, c),
        state,
    ]));
    let (sex_g, sex_name) = match c.sex {
        crate::sim::Sex::Male => (glyphs::MALE, "male"),
        crate::sim::Sex::Female => (glyphs::FEMALE, "female"),
    };
    rows.push(line(vec![
        sp("   ", theme::text()),
        sp(sim.roster().display_name(c.species), sim.roster().style(c.species)),
        sp(format!("  {sex_g} {sex_name}"), theme::text()),
        sp(format!("  {}", if c.adult { "adult" } else { "juvenile" }), theme::text()),
        sp(if c.sterile { ", sterile" } else { "" }, Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
        sp(format!("  diet: {}", sim.species_params(c.species).diet), theme::dim_text()),
    ]));
    rows.push(blank(1));

    // Age bar (live: age from born_day, max from genome).
    let max_age = c.max_age_days(&sim.params.creatures, &sim.params.genetics);
    let age = c.age_days(sim.time.day_index());
    let age_t = crate::cast!(age => f32) / crate::cast!(max_age.max(1) => f32);
    let age_color = if c.alive { bars::vital_color(1.0 - age_t * 0.8, false) } else { theme::DIM };
    rows.push(Box::new(
        LabeledBar::new(" age", age_t).color(age_color).label_and_bar(6, 20).suffix(format!("{age} / {max_age} days")).suffix_style(theme::dim_text()),
    ));
    rows.push(line(vec![
        sp(format!("       {:.1} years old", crate::cast!(age => f32) / 360.0), theme::dim_text()),
        sp(format!("   generation {}", c.generation), theme::text()),
    ]));
    rows.push(blank(1));

    rows.extend(family(sim, c));
    rows.extend(quirks(sim, c));
    rows.extend(location(sim, c));
    rows.extend(if c.alive { vitals(sim, c) } else { death(sim, c, (age, max_age, age_t)) });
    rows.extend(timeline(sim, c));
    rows
}

/// The identity panel's family rows.
fn family(sim: &Sim, c: &Creature) -> Rows<'static> {
    let (mother, father) = match c.parents {
        Some((m, fa)) => (kin_name(sim, m), kin_name(sim, fa)),
        None => ("founder".to_string(), "founder".to_string()),
    };
    let pregnant = c.pregnant_due.map(|d| format!("   pregnant, due in {} h", d.saturating_sub(sim.time.tick))).unwrap_or_default();
    vec![
        section("Family"),
        line(vec![sp(" mother  ", theme::dim_text()), sp(format!("{mother:<18}"), theme::text()), sp(" father  ", theme::dim_text()), sp(father, theme::text())]),
        line(vec![
            sp(" offspring  ", theme::dim_text()),
            sp(format!("{}", c.offspring), theme::text()),
            sp(pregnant, Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
            sp(format!("    {} lineage: [l]", glyphs::NOTE), theme::dim_text()),
        ]),
        blank(1),
    ]
}

/// The badge beside the name: `Φ` for a legendary quirk, `φ` for any other.
fn quirk_badge(sim: &Sim, c: &Creature) -> ratatui::text::Span<'static> {
    let qp = &sim.params.quirks;
    if c.quirks.is_empty() {
        return sp("", theme::text());
    }
    let legendary = c.quirks.iter().any(|i| qp.catalog.get(i).is_some_and(|d| d.tier == QuirkTier::Legendary));
    let (g, col) = if legendary { (glyphs::QUIRK_LEGEND, theme::MAGENTA) } else { (glyphs::QUIRK, theme::INFO) };
    sp(format!(" {g}"), Style::default().fg(col).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
}

/// The Quirks block: one line per quirk with its effects. Absent when quirks are off.
fn quirks(sim: &Sim, c: &Creature) -> Rows<'static> {
    let qp = &sim.params.quirks;
    if !qp.enabled {
        return Rows::new();
    }
    let mut rows = vec![section("Quirks")];
    if c.quirks.is_empty() {
        rows.push(one(" none", theme::dim_text()));
    }
    for d in c.quirks.iter().filter_map(|i| qp.catalog.get(i)) {
        let (g, col) = match d.tier {
            QuirkTier::Legendary => (glyphs::QUIRK_LEGEND, theme::MAGENTA),
            QuirkTier::Rare => (glyphs::QUIRK, theme::INFO),
            QuirkTier::Common => (glyphs::QUIRK, theme::TEXT),
        };
        rows.push(line(vec![
            sp(format!(" {g} "), Style::default().fg(col).bg(theme::PANEL_BG)),
            sp(format!("{:<13}", d.name), Style::default().fg(col).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(d.summary(), theme::dim_text()),
        ]));
    }
    rows.push(blank(1));
    rows
}

/// The identity panel's location block.
fn location(sim: &Sim, c: &Creature) -> Rows<'static> {
    let cell = sim.world.cell(c.x, c.y);
    let (tg, tfg, _) = map::terrain_cell(cell, false);
    let mut rows = vec![
        section("Location"),
        line(vec![
            sp(format!(" ({}, {})  ", c.x, c.y), theme::text()),
            sp(sim.world.region_name(c.x, c.y), theme::title()),
            sp(format!("   {tg} "), Style::default().fg(tfg).bg(theme::PANEL_BG)),
            sp(cell.terrain.name(), theme::dim_text()),
        ]),
    ];
    if c.alive {
        let goal = c.goal.label(sim.world.dens.iter().any(|&(x, y)| x == c.x && y == c.y));
        rows.push(line(vec![sp(" goal    ", theme::dim_text()), sp(goal, theme::text())]));
        let target = match c.target {
            Some((tx, ty)) => {
                let d = crate::sim::dist(c.x, c.y, tx, ty);
                format!("{} ({}, {})  {:.0} cells", glyphs::DIAMOND, tx, ty, d)
            }
            None => "none".to_string(),
        };
        rows.push(line(vec![sp(" target  ", theme::dim_text()), sp(target, theme::label())]));
        let trail: Vec<String> = c.trail.iter().rev().take(5).map(|(x, y)| format!("({x},{y})")).collect();
        rows.push(line(vec![
            sp(" trail   ", theme::dim_text()),
            sp(if trail.is_empty() { "no recent movement".to_string() } else { trail.join(" ") }, theme::dim_text()),
        ]));
    }
    rows.push(blank(1));
    rows
}

/// Vitals, condition and behaviour for a living creature.
fn vitals(sim: &Sim, c: &Creature) -> Rows<'static> {
    let mut rows = vec![section("Vitals")];
    for (label, v, inv) in [("health", c.hp, false), ("hunger", c.hunger, true), ("thirst", c.thirst, true), ("energy", c.energy, false)] {
        rows.push(Box::new(LabeledBar::new(format!(" {label}"), v).color(bars::vital_color(v, inv)).label_and_bar(9, 24)));
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
    rows.push(line(vec![sp(format!("{:<9}", " sickness"), theme::text()), sickness]));
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
    rows.push(line(vec![sp(format!("{:<9}", " immune"), theme::text()), immune_span]));
    let load = c.parasite_load;
    let note = if load < 0.2 { "light" } else if load < 0.5 { "heavy" } else { "severe" };
    rows.push(Box::new(
        LabeledBar::new(format!(" {} parasites", glyphs::PARASITE), load)
            .color(theme::WARN)
            .label_and_bar(12, 24)
            .suffix(note)
            .suffix_style(Style::default().fg(bars::vital_color(load, true)).bg(theme::PANEL_BG)),
    ));
    rows.push(blank(1));
    rows.push(section("Condition"));
    rows.push(Box::new(LabeledBar::new(" predation risk", c.predation_risk).color(bars::vital_color(c.predation_risk, true)).label_and_bar(17, 16)));
    let contagion = contagion_risk(sim, c);
    rows.push(Box::new(LabeledBar::new(" contagion risk", contagion).color(bars::vital_color(contagion, true)).label_and_bar(17, 16)));
    // Local forage: mean vegetation within 3 cells.
    let forage = local_forage(sim, c.x, c.y);
    rows.push(Box::new(LabeledBar::new(" local forage", forage).color(theme::VEGETATION).label_and_bar(17, 16)));
    rows.push(line(vec![sp(" nearest water", theme::dim_text()), sp(" —", theme::text()), sp("   nearest den", theme::dim_text()), sp(" —", theme::text())]));
    rows.push(blank(1));

    rows.push(section("Behaviour"));
    if crate::sim::disease::is_infectious(c) {
        rows.push(line(vec![
            sp(format!(" {} ", glyphs::ALERT), Style::default().fg(theme::SICK).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp("sick — resting more, no mating", Style::default().fg(theme::SICK).bg(theme::PANEL_BG)),
        ]));
    }
    rows.push(one(goal_line(sim, c), theme::text()));
    rows.push(blank(1));
    rows
}

/// The Behaviour line: why the creature is doing what it is doing.
fn goal_line(sim: &Sim, c: &Creature) -> String {
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
        // C5 FR13: a resident walking at an intruder, or an evicted predator.
        Goal::Challenge => match c.challenge_target {
            Some(t) => format!(" {} — driving off {}", c.goal.label(false), kin_name(sim, t)),
            None => format!(" {}", c.goal.label(false)),
        },
        Goal::Flee if sim.roster().kind(c.species) == Kind::Predator => format!(" {} — driven off a rival's ground", c.goal.label(false)),
        _ => {
            if !c.adult && c.mother.is_some_and(|m| sim.creatures.get(m).is_some_and(|m| m.alive)) && c.age_days(sim.time.day_index()) < sim.params.genetics.follow_mother_days {
                " wandering near its mother".to_string()
            } else {
                format!(" {}", c.goal.label(false))
            }
        }
    }
}

/// The death summary for a dead creature; `age` is `(age, max_age, age_t)`.
fn death(sim: &Sim, c: &Creature, age: (u32, u32, f32)) -> Rows<'static> {
    let (age, max_age, age_t) = age;
    let cause = c.death.map_or("unknown", |d| d.cause.label());
    let cause = match c.died_infected {
        Some(p) => format!("{} ({})", cause, sim.disease.name(p)),
        None => cause.to_string(),
    };
    let nutrition = 1.0 - c.decay;
    let kg = crate::cast!((c.genome.size() * 120.0 * nutrition).round() => u32);
    let gone_in = crate::cast!(((1.0 - c.decay) * crate::cast!(sim.params.creatures.carcass_decay_days => f32)).ceil() => u32);
    let mut rows = vec![
        section("Death"),
        line(vec![sp(format!(" {} ", glyphs::DEATH), Style::default().fg(theme::BAD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)), sp(cause, theme::text())]),
        one(format!("   lived {age} of {max_age} days ({}% of lifespan)", crate::cast!((age_t * 100.0).round() => u32)), theme::dim_text()),
        blank(1),
        Box::new(LabeledBar::new(" decay", c.decay).color(theme::CARCASS).label_and_bar(12, 24)),
        one(format!("   {kg} kg of meat remaining; gone in ~{gone_in} days"), theme::dim_text()),
        blank(1),
    ];
    rows.extend(killer_and_scavengers_rows(sim, c));
    rows.push(blank(1));
    rows
}

/// Timeline (C4 FR10): born, adult, each litter, death.
fn timeline(sim: &Sim, c: &Creature) -> Rows<'static> {
    let season_days = sim.time.season_days;
    let mut events: Vec<(char, Color, i64, String)> = Vec::new();
    let born_text = match c.parents {
        Some((m, fa)) => format!("born to {} and {}", kin_name(sim, m), kin_name(sim, fa)),
        None => "placed as a founder".to_string(),
    };
    events.push((glyphs::BIRTH, theme::GOOD, i64::from(c.born_day), born_text));
    let adult_day = i64::from(c.born_day) + i64::from(c.adult_age_days(sim.species_params(c.species), &sim.params.genetics));
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
    let mut rows = vec![section("Timeline")];
    for (g, color, d, text) in events {
        rows.push(line(vec![
            sp(format!(" {g} "), Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{:<9} ", day_stamp(d, season_days)), theme::dim_text()),
            sp(text, theme::text()),
        ]));
    }
    rows
}
