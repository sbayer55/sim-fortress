//! S03: the live creature inspector (S03a prey / S03c corpse; S03b predator is
//! placeholder until C5). Mirrors the three-column layout with live
//!
//! data; C4 adds family names, offspring forecast, kin, legacy and timeline.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::creatures::{adult_age_days, Cause, Creature, CreatureId, Goal};
use crate::sim::disease::{PathogenId, Stage};
use crate::sim::{Genome, Kind, SpeciesId, TRAIT_NAMES};
use crate::ui::app::AppState;
use crate::ui::screens::common::day_stamp;
use crate::ui::screens::s08_lineage::LineageScreen;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{EventKindStyle, SpeciesStyle};
use crate::widgets::map::{self, MapOptions, Overlay};
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

const LEFT_W: u16 = 52;
const MID_W: u16 = 52;

#[derive(Debug)]
pub struct Inspector {
    pub id: CreatureId,
}

impl Inspector {
    pub const fn new(id: CreatureId) -> Self {
        Self { id }
    }
}

impl Screen for Inspector {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Char('f') => {
                app.follow = Some(self.id);
                app.look_cursor = None;
                Action::Pop
            }
            KeyCode::Tab => {
                if let Some(sim) = &app.sim {
                    let ids = sim.creatures.living_ids();
                    if !ids.is_empty() {
                        let cur = ids.iter().position(|&x| x == self.id).unwrap_or(0);
                        self.id = ids[(cur + 1) % ids.len()];
                    }
                }
                Action::None
            }
            KeyCode::Char('l') => Action::Push(Box::new(LineageScreen::new(self.id))),
            KeyCode::Esc => Action::Pop,
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else { return };
        let Some(c) = sim.creatures.get(self.id) else { return };
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;

        let left = Rect::new(area.x, area.y, LEFT_W, body_h);
        let mid = Rect::new(area.x + LEFT_W, area.y, MID_W, body_h);
        let right = Rect::new(area.x + LEFT_W + MID_W, area.y, area.width - LEFT_W - MID_W, body_h);

        identity(f, left, app, c);
        genome(f, mid, sim, c);
        life(f, right, app, sim, c, self.id);

        let right_text = format!("{} {}  {}", c.name_str(), c.tag(), sim.time.clock_label());
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("f", "follow"), ("l", "lineage"), ("Tab", "next creature"), ("Esc", "back")],
            &right_text,
        );
    }
}

fn sp(s: impl Into<String>, st: Style) -> Span<'static> {
    Span::styled(s.into(), st)
}

fn species_style(id: SpeciesId) -> Style {
    Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)
}

fn identity(f: &mut Frame<'_>, area: Rect, app: &AppState, c: &Creature) {
    let Some(sim) = app.sim.as_ref() else { return };
    let title = if c.alive { "Identity & Vitals" } else { "Identity & Death" };
    let inner = panel::draw(f, area, title, panel::Kind::Outer);
    let mut row = 0u16;

    // Name line.
    let state = if !c.alive {
        sp("  DEAD", Style::default().fg(theme::BAD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
    } else if c.species.kind() == Kind::Predator {
        sp("  predator", Style::default().fg(c.species.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
    } else {
        sp("  prey", Style::default().fg(theme::GOOD).bg(theme::PANEL_BG))
    };
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} ", if c.alive { c.species.glyph().to_ascii_uppercase() } else { glyphs::CARCASS }), species_style(c.species)),
        sp(c.name_str().to_string(), theme::title()),
        sp(format!("  {}", c.tag()), theme::label()),
        state,
    ]));
    row += 1;
    let (sex_g, sex_name) = match c.sex {
        crate::sim::Sex::Male => (glyphs::MALE, "male"),
        crate::sim::Sex::Female => (glyphs::FEMALE, "female"),
    };
    util::line(f, inner, row, Line::from(vec![
        sp("   ", theme::text()),
        sp(c.species.name(), species_style(c.species)),
        sp(format!("  {sex_g} {sex_name}"), theme::text()),
        sp(format!("  {}", if c.adult { "adult" } else { "juvenile" }), theme::text()),
        sp(format!("  diet: {}", c.species.diet()), theme::dim_text()),
    ]));
    row += 2;

    // Age bar (live: age from born_day, max from genome).
    let max_age = c.max_age_days(&sim.params.creatures, &sim.params.genetics);
    let age = c.age_days(sim.time.day_index());
    let age_t = crate::cast!(age => f32) / crate::cast!(max_age.max(1) => f32);
    let age_color = if c.alive { bars::vital_color(1.0 - age_t * 0.8, false) } else { theme::DIM };
    bars::labeled(f.buffer_mut(), inner, row, " age", age_t, age_color, 6, 20);
    f.buffer_mut().set_stringn(inner.x + 34, inner.y + row, format!("{age} / {max_age} days"), 16, theme::dim_text());
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(format!("       {:.1} years old", crate::cast!(age => f32) / 360.0), theme::dim_text()),
        sp(format!("   generation {}", c.generation), theme::text()),
    ]));
    row += 2;

    row = identity_family(f, inner, row, sim, c);
    row = identity_location(f, inner, row, sim, c);
    identity_state(f, inner, row, sim, c);
    identity_timeline(f, inner, row, sim, c);
}
/// The identity panel's family rows.
fn identity_family(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    panel::section(f, inner, row, "Family");
    row += 1;
    let (mother, father) = match c.parents {
        Some((m, fa)) => (kin_name(sim, m), kin_name(sim, fa)),
        None => ("founder".to_string(), "founder".to_string()),
    };
    util::line(f, inner, row, Line::from(vec![
        sp(" mother  ", theme::dim_text()),
        sp(format!("{mother:<18}"), theme::text()),
        sp(" father  ", theme::dim_text()),
        sp(father, theme::text()),
    ]));
    row += 1;
    let pregnant = c.pregnant_due.map(|d| format!("   pregnant, due in {} h", d.saturating_sub(sim.time.tick))).unwrap_or_default();
    util::line(f, inner, row, Line::from(vec![
        sp(" offspring  ", theme::dim_text()),
        sp(format!("{}", c.offspring), theme::text()),
        sp(pregnant, Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        sp(format!("    {} lineage: [l]", glyphs::NOTE), theme::dim_text()),
    ]));
    row += 2;
    row
}

/// The identity panel's location block.
fn identity_location(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    panel::section(f, inner, row, "Location");
    row += 1;
    let cell = sim.world.cell(c.x, c.y);
    let (tg, tfg, _) = map::terrain_cell(cell, false);
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" ({}, {})  ", c.x, c.y), theme::text()),
        sp(sim.world.region_name(c.x, c.y), theme::title()),
        sp(format!("   {tg} "), Style::default().fg(tfg).bg(theme::PANEL_BG)),
        sp(cell.terrain.name(), theme::dim_text()),
    ]));
    row += 1;
    if c.alive {
        let goal = c.goal.label(sim.world.dens.iter().any(|&(x, y)| x == c.x && y == c.y));
        util::line(f, inner, row, Line::from(vec![sp(" goal    ", theme::dim_text()), sp(goal, theme::text())]));
        row += 1;
        let target = match c.target {
            Some((tx, ty)) => {
                let d = crate::sim::dist(c.x, c.y, tx, ty);
                format!("{} ({}, {})  {:.0} cells", glyphs::DIAMOND, tx, ty, d)
            }
            None => "none".to_string(),
        };
        util::line(f, inner, row, Line::from(vec![sp(" target  ", theme::dim_text()), sp(target, theme::label())]));
        row += 1;
        let trail: Vec<String> = c.trail.iter().rev().take(5).map(|(x, y)| format!("({x},{y})")).collect();
        util::line(f, inner, row, Line::from(vec![
            sp(" trail   ", theme::dim_text()),
            sp(if trail.is_empty() { "no recent movement".to_string() } else { trail.join(" ") }, theme::dim_text()),
        ]));
        row += 1;
    }
    row += 1;
    row
}

/// Vitals, condition and behaviour (living) or the death summary (dead).
fn identity_state(f: &mut Frame<'_>, inner: Rect, row: u16, sim: &crate::sim::Sim, c: &Creature) {
    if c.alive {
        identity_vitals(f, inner, row, sim, c);
    } else {
        let age = c.age_days(sim.time.day_index());
        let max_age = c.max_age_days(&sim.params.creatures, &sim.params.genetics);
        let age_t = crate::cast!(age => f32) / crate::cast!(max_age.max(1) => f32);
        identity_death(f, inner, row, sim, c, age, max_age, age_t);
    }
}
/// Vitals, condition and behaviour for a living creature.
fn identity_vitals(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature) {

    panel::section(f, inner, row, "Vitals");
    row += 1;
    for (label, v, inv) in [("health", c.hp, false), ("hunger", c.hunger, true), ("thirst", c.thirst, true), ("energy", c.energy, false)] {
        bars::labeled(f.buffer_mut(), inner, row, &format!(" {label}"), v, bars::vital_color(v, inv), 9, 24);
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
    util::line(f, inner, row, Line::from(vec![sp(format!("{:<9}", " sickness"), theme::text()), sickness]));
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
    util::line(f, inner, row, Line::from(vec![sp(format!("{:<9}", " immune"), theme::text()), immune_span]));
    row += 1;
    let load = c.parasite_load;
    bars::labeled(f.buffer_mut(), inner, row, &format!(" {} parasites", glyphs::PARASITE), load, theme::WARN, 12, 24);
    let note = if load < 0.2 { "light" } else if load < 0.5 { "heavy" } else { "severe" };
    f.buffer_mut().set_stringn(inner.x + 44, inner.y + row, note, 6, Style::default().fg(bars::vital_color(load, true)).bg(theme::PANEL_BG));
    row += 2;
    panel::section(f, inner, row, "Condition");
    row += 1;
    bars::labeled(f.buffer_mut(), inner, row, " predation risk", c.predation_risk, bars::vital_color(c.predation_risk, true), 17, 16);
    row += 1;
    let contagion = contagion_risk(sim, c);
    bars::labeled(f.buffer_mut(), inner, row, " contagion risk", contagion, bars::vital_color(contagion, true), 17, 16);
    row += 1;
    // Local forage: mean vegetation within 3 cells.
    let forage = local_forage(sim, c.x, c.y);
    bars::labeled(f.buffer_mut(), inner, row, " local forage", forage, theme::VEGETATION, 17, 16);
    row += 1;
    util::line(f, inner, row, Line::from(vec![sp(" nearest water", theme::dim_text()), sp(" —", theme::text()), sp("   nearest den", theme::dim_text()), sp(" —", theme::text())]));
    row += 2;

    panel::section(f, inner, row, "Behaviour");
    row += 1;
    if crate::sim::disease::is_infectious(c) {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", glyphs::ALERT), Style::default().fg(theme::SICK).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp("sick — resting more, no mating", Style::default().fg(theme::SICK).bg(theme::PANEL_BG)),
        ]));
        row += 1;
    }
    let line = match c.goal {
        Goal::Drink => format!(" {} because thirst = {:.2}", c.goal.label(false), c.thirst),
        Goal::Graze => format!(" {} because hunger = {:.2}", c.goal.label(false), c.hunger),
        Goal::Rest => format!(" {} because energy = {:.2}", c.goal.label(false), c.energy),
        Goal::Mate => match c.mate_id {
            Some(m) => format!(" {} — heading for {}", c.goal.label(false), kin_name(sim, m)),
            None => format!(" {}", c.goal.label(false)),
        },
        _ => {
            if !c.adult && c.mother.is_some_and(|m| sim.creatures.get(m).is_some_and(|m| m.alive)) && c.age_days(sim.time.day_index()) < sim.params.genetics.follow_mother_days {
                " wandering near its mother".to_string()
            } else {
                format!(" {}", c.goal.label(false))
            }
        }
    };
    util::line(f, inner, row, Line::from(sp(line, theme::text())));
}

/// The death summary for a dead creature.
#[allow(clippy::too_many_arguments)]
fn identity_death(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature, age: u32, max_age: u32, age_t: f32) {

    panel::section(f, inner, row, "Death");
    row += 1;
    let cause = c.death.map_or("unknown", |d| d.cause.label());
    let cause = match c.died_infected {
        Some(p) => format!("{} ({})", cause, sim.disease.name(p)),
        None => cause.to_string(),
    };
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} ", glyphs::DEATH), Style::default().fg(theme::BAD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(cause, theme::text()),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(format!("   lived {age} of {max_age} days ({}% of lifespan)", crate::cast!((age_t * 100.0).round() => u32)), theme::dim_text()),
    ]));
    row += 2;
    bars::labeled(f.buffer_mut(), inner, row, " decay", c.decay, theme::CARCASS, 12, 24);
    row += 1;
    let nutrition = 1.0 - c.decay;
    let kg = crate::cast!((c.genome.size() * 120.0 * nutrition).round() => u32);
    let gone_in = crate::cast!(((1.0 - c.decay) * crate::cast!(sim.params.creatures.carcass_decay_days => f32)).ceil() => u32);
    util::line(f, inner, row, Line::from(vec![
        sp(format!("   {kg} kg of meat remaining; gone in ~{gone_in} days"), theme::dim_text()),
    ]));
    row += 2;
    killer_and_scavengers(f, inner, row, sim, c);
}

fn identity_timeline(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature) {
    // Timeline (C4 FR10): born, adult, each litter, death.
    panel::section(f, inner, row, "Timeline");
    row += 1;
    let season_days = sim.time.season_days;
    let mut events: Vec<(char, Color, i64, String)> = Vec::new();
    let born_text = match c.parents {
        Some((m, fa)) => format!("born to {} and {}", kin_name(sim, m), kin_name(sim, fa)),
        None => "placed as a founder".to_string(),
    };
    events.push((glyphs::BIRTH, theme::GOOD, i64::from(c.born_day), born_text));
    let adult_day = i64::from(c.born_day) + i64::from(adult_age_days(c.species, &c.genome, &sim.params.creatures, &sim.params.genetics));
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
    let max_rows = crate::cast!(inner.height.saturating_sub(row) => usize);
    let skip = events.len().saturating_sub(max_rows);
    for (g, color, d, text) in events.into_iter().skip(skip) {
        if row >= inner.height {
            break;
        }
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {g} "), Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{:<9} ", day_stamp(d, season_days)), theme::dim_text()),
            sp(text, theme::text()),
        ]));
        row += 1;
    }
}

/// `Name tag` for a relative, from the store or the lineage.
/// S03c (Identity & Death panel): the killer from
/// `death.killer` and the two nearest living predators with the Scavenge goal.
pub(crate) fn killer_and_scavengers(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    // C7 (S03c): a disease death names its outbreak instead of a killer.
    let outbreak_index = if c.death.is_some_and(|d| d.cause == Cause::Disease) {
        sim.lineage.get(c.id).and_then(|n| n.outbreak).or_else(|| c.infection.map(|i| i.outbreak))
    } else {
        None
    };
    if let Some(o) = outbreak_index.and_then(|i| sim.disease.outbreak(i)) {
        panel::section(f, inner, row, "Outbreak");
        row += 1;
        let year = o.started_day.div_euclid((4 * sim.time.season_days).max(1)) + 1;
        let region = sim.world.regions.get(crate::cast!(o.origin_region => usize)).map_or("?", |r| r.0.as_str());
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", glyphs::DISEASE), Style::default().fg(theme::SICK).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{} outbreak of Year {}, began {}", sim.disease.name(o.pathogen), year, region), theme::text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(sp(format!("   {} others died in it", o.deaths.saturating_sub(1)), theme::dim_text())));
        row += 2;
    } else if let Some(killer_id) = c.death.and_then(|d| d.killer) {
        panel::section(f, inner, row, "Killer");
        row += 1;
        let kname = sim
            .creatures
            .get(killer_id)
            .map(|k| format!("{} {}", k.name_str(), k.tag()))
            .or_else(|| sim.lineage.get(killer_id).map(|n| format!("{} {}", n.name_str(), n.tag)))
            .unwrap_or_else(|| format!("#{}", killer_id.0));
        let kglyph = sim.creatures.get(killer_id).map_or('?', |k| k.species.glyph().to_ascii_uppercase());
        let kcolor = sim.creatures.get(killer_id).map_or(theme::DIM, |k| k.species.color());
        let kills = sim.creatures.get(killer_id).map_or(0, |k| k.kills);
        let chase = c.death.map_or(0, |d| d.chase_ticks);
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {kglyph} "), Style::default().fg(kcolor).bg(theme::PANEL_BG)),
            sp(kname, theme::title()),
            sp(format!("  {kills} kills  chase {chase} ticks"), theme::text()),
        ]));
        row += 2;
    }
    panel::section(f, inner, row, "Scavengers nearby");
    row += 1;
    let mut scav: Vec<(f32, &Creature)> = sim
        .creatures
        .living()
        .filter(|o| o.species.kind() == Kind::Predator && o.goal == Goal::Scavenge)
        .map(|o| (crate::sim::dist(c.x, c.y, o.x, o.y), o))
        .collect();
    scav.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.id.cmp(&b.1.id)));
    if scav.is_empty() {
        util::line(f, inner, row, Line::from(sp(" none", theme::dim_text())));
        row += 1;
    }
    for (d, o) in scav.iter().take(2) {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", o.species.glyph().to_ascii_uppercase()), Style::default().fg(o.species.color()).bg(theme::PANEL_BG)),
            sp(format!("{:<9}{:<7} {:>3.0} cells {}  {}", o.name_str(), o.tag(), d, compass(c.x, c.y, o.x, o.y), o.goal.plain()), theme::text()),
        ]));
        row += 1;
    }
    row
}

pub(crate) fn kin_name(sim: &crate::sim::Sim, id: CreatureId) -> String {
    if let Some(c) = sim.creatures.get(id) {
        return format!("{} {}", c.name_str(), c.tag());
    }
    match sim.lineage.get(id) {
        Some(n) => format!("{} {}", n.name_str(), n.tag),
        None => format!("#{}", id.0),
    }
}

fn genome(f: &mut Frame<'_>, area: Rect, sim: &crate::sim::Sim, c: &Creature) {
    let inner = panel::draw_with_hint(f, area, "Genome", &format!("vs {} mean", c.species.plural()), panel::Kind::Outer);
    let stats = &sim.species[c.species.index()];
    let mean = stats.mean;
    let min = stats.min;
    let max = stats.max;
    let mut row = 0u16;
    util::line(f, inner, row, Line::from(sp(
        format!(" {:<11}{:^12} {:<4} {:<6} {}", "trait", "individual", "own", "delta", "species range"),
        theme::dim_text(),
    )));
    row += 1;
    // One row per trait (C8 widened the genome to eleven): label, own bar, value,
    // delta against the species mean, then the species min/mean/max range.
    for t in 0..Genome::LEN {
        let v = c.genome.0[t];
        let d = v - mean.0[t];
        let color = trait_color(t);
        let y = inner.y + row;
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x, y, format!(" {:<11}", TRAIT_NAMES[t]), 12, theme::text());
        bars::bar(buf, inner.x + 12, y, 12, v, color);
        buf.set_stringn(inner.x + 25, y, format!("{v:.2}"), 4, theme::text());
        buf.set_stringn(inner.x + 30, y, format!("{}{:+.2}", glyphs::PLUS_MINUS, d), 6, delta_style(d));
        bars::range(buf, inner.x + 37, y, 11, min.0[t], mean.0[t], max.0[t], color);
        row += 1;
    }
    row += 1;

    panel::section(f, inner, row, "Mutation history");
    row += 1;
    if c.mutations.is_empty() {
        util::line(f, inner, row, Line::from(sp(" none recorded", theme::dim_text())));
        row += 1;
    }
    for m in &c.mutations {
        util::line(f, inner, row, Line::from(sp(format!(" {} {} {:+.2} (gen {})", glyphs::MUTATION, TRAIT_NAMES[m.trait_idx], m.delta, m.generation), theme::text())));
        row += 1;
    }
    util::line(f, inner, row, Line::from(sp(
        format!(" from {} lines; rate {:.2} per trait per birth", c.generation, sim.params.genetics.mutation_rate),
        theme::dim_text(),
    )));
    row += 2;

    row = genome_derived(f, inner, row, sim, c);

    genome_forecast(f, inner, row, sim, c, &mean);
}
/// The derived-trait table (sense, speed, food need, maturity, ...).
fn genome_derived(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    panel::section(f, inner, row, "Derived");
    row += 1;
    let g = &c.genome;
    let social = &sim.params.social;
    let group_word = match c.species.kind() {
        Kind::Prey => {
            if social.herding(g.sociality(), c.kin_nearby) {
                "herd"
            } else {
                "scattered"
            }
        }
        Kind::Predator => {
            if social.herding(g.sociality(), c.kin_nearby) {
                "pack"
            } else {
                "alone"
            }
        }
    };
    let derived: Vec<(String, String)> = vec![
        ("sense range".into(), format!("{} cells", g.sense_cells())),
        ("move speed".into(), format!("{:.1} cells/tick", 0.5 + g.speed() * 2.0)),
        ("daily food need".into(), format!("{:.2} biomass", 24.0 * sim.params.creatures.hunger_per_hour(g.size(), g.metabolism(), 1.0))),
        ("adult at".into(), format!("{} days", adult_age_days(c.species, g, &sim.params.creatures, &sim.params.genetics))),
        ("max lifespan".into(), format!("{} days", c.max_age_days(&sim.params.creatures, &sim.params.genetics))),
        (
            "litter size".into(),
            format!(
                "{} (fert {:.2}, mat {:.2})",
                sim.params.genetics.litter_size(c.species, g.fertility(), g.maturity()),
                g.fertility(),
                g.maturity()
            ),
        ),
        ("mate cooldown".into(), format!("{} days", sim.params.genetics.cooldown(c.species))),
        ("resistance cost".into(), format!("+{} % food", crate::cast!((100.0 * sim.params.disease.resist_hunger_cost * g.resistance()).round() => u32))),
        ("kin nearby".into(), format!("{} ({})", c.kin_nearby, group_word)),
    ];
    for (k, v) in derived {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {k:<18}"), theme::dim_text()),
            sp(v, theme::text()),
        ]));
        row += 1;
    }
    row += 1;
    row
}

/// The offspring-trait forecast against an average mate.
fn genome_forecast(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature, mean: &Genome) -> u16 {
    // Offspring forecast (C4 FR10): with an average mate, each trait is drawn
    // from either parent and mutates with sd `mutation_strength`.
    panel::section(f, inner, row, "Offspring forecast (with an average mate)");
    row += 1;
    let sd = sim.params.genetics.mutation_strength;
    for t in 0..Genome::LEN {
        if row >= inner.height {
            break;
        }
        let a = c.genome.0[t];
        let b = mean.0[t];
        let centre = (a + b) / 2.0;
        let lo = a.min(b) - sd;
        let hi = a.max(b) + sd;
        let y = inner.y + row;
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x, y, format!(" {:<12}", TRAIT_NAMES[t]), 13, theme::text());
        bars::range(buf, inner.x + 13, y, 22, lo.max(0.0), centre, hi.min(1.0), trait_color(t));
        buf.set_stringn(inner.x + 36, y, format!("{:.2}..{:.2}", lo.max(0.0), hi.min(1.0)), 12, theme::dim_text());
        row += 1;
    }
    row
}

fn life(f: &mut Frame<'_>, area: Rect, app: &AppState, sim: &crate::sim::Sim, c: &Creature, id: CreatureId) {
    let inner = panel::draw(f, area, "Life", panel::Kind::Outer);
    let mut row = life_minimap(f, inner, sim, c, id);
    row = life_stats(f, inner, row, sim, c);
    row = life_legacy(f, inner, row, sim, c, id);
    life_kin(f, inner, row, sim, c, id);
    let _ = app;
}
/// The surroundings mini-map and the life-stat lines beside it.
fn life_minimap(f: &mut Frame<'_>, inner: Rect, sim: &crate::sim::Sim, c: &Creature, id: CreatureId) -> u16 {
    let mut row = 0u16;
    // Mini-map top-right.
    let mm_w = 23u16;
    let mm_h = 9u16;
    let mm = Rect::new(inner.right() - mm_w, inner.y, mm_w, mm_h);
    let mm_inner = panel::draw(f, mm, "Surroundings", panel::Kind::Inner);
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
    map::render(f.buffer_mut(), mm_inner, sim, &opts);

    // Life stats to the left of the mini-map.
    let stats = Rect::new(inner.x, inner.y, inner.width - mm_w - 1, mm_h);
    let age = c.age_days(sim.time.day_index());
    let lines = vec![
        Line::from(vec![sp(" days alive  ", theme::dim_text()), sp(format!("{age}"), theme::text())]),
        Line::from(vec![sp(" offspring   ", theme::dim_text()), sp(format!("{}", c.offspring), theme::text())]),
        Line::from(vec![sp(" distance    ", theme::dim_text()), sp(format!("{} cells", c.trail.len()), theme::text())]),
    ];
    for (i, l) in lines.into_iter().enumerate() {
        util::line(f, stats, crate::cast!(i => u16), l);
    }
    row += mm_h + 1;
    row
}

/// Hunt stats (predator) or survival stats (prey).
fn life_stats(f: &mut Frame<'_>, inner: Rect, row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    if !c.alive {
        return row;
    }
    if c.species.kind() == Kind::Predator {
        hunt_stats(f, inner, row, sim, c)
    } else {
        survival_stats(f, inner, row, c)
    }
}

/// S03b: hunt outcomes, prey preference and chase history.
fn hunt_stats(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature) -> u16 {
    // S03b Hunt stats.
    panel::section(f, inner, row, "Hunt stats");
    row += 1;
    let success = if c.attempts > 0 { crate::cast!(c.kills => f32) / crate::cast!(c.attempts => f32) * 100.0 } else { 0.0 };
    util::line(f, inner, row, Line::from(sp(format!(" kills {}  attempts {}  success {:.0}%", c.kills, c.attempts, success), theme::text())));
    row += 1;
    bars::labeled(f.buffer_mut(), inner, row, " success rate", success / 100.0, c.species.color(), 16, 16);
    row += 1;
    panel::section(f, inner, row, "Preferred prey");
    row += 1;
    for prey_id in SpeciesId::ALL.iter().filter(|s| s.kind() == Kind::Prey) {
        let share = if c.kills >= 5 {
            crate::cast!(c.kills_by_species[prey_id.index()] => f32) / crate::cast!(c.kills.max(1) => f32)
        } else {
            sim.params.predation.preference(c.species, *prey_id)
        };
        bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", prey_id.plural()), share, prey_id.color(), 16, 16);
        f.buffer_mut().set_stringn(
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
    util::line(f, inner, row, Line::from(vec![sp(" last kill ", theme::dim_text()), sp(last_kill, theme::text())]));
    row += 1;
    if let Some(o) = c.hunt_target.and_then(|t| sim.creatures.get(t)) {
        let d = crate::sim::dist(c.x, c.y, o.x, o.y);
        let mut spans = vec![sp(" current target ", theme::dim_text()), sp(format!("{} {}, {:.0} cells", o.name_str(), o.tag(), d), theme::label())];
        if let Some(i) = o.infection.filter(|i| i.stage == Stage::Infectious) {
            let bonus = sim.params.disease.kill_sick_bonus * i.severity;
            spans.push(sp(format!(" (+.{:02} sick prey)", crate::cast!((bonus * 100.0).round() => u32) % 100), Style::default().fg(theme::SICK).bg(theme::PANEL_BG)));
        }
        util::line(f, inner, row, Line::from(spans));
        row += 1;
    }
    let avg = if c.attempts > 0 { c.chase_stats.0.div_euclid(c.attempts.max(1)) } else { 0 };
    util::line(f, inner, row, Line::from(vec![
        sp(" avg chase ", theme::dim_text()),
        sp(format!("{avg} ticks; longest {} (Year {})", c.chase_stats.1, c.chase_longest_year), theme::text()),
    ]));
    row += 2;
    row
}

/// S03a: escape record and the predators this prey has seen.
fn survival_stats(f: &mut Frame<'_>, inner: Rect, mut row: u16, c: &Creature) -> u16 {
    // S03a Survival.
    panel::section(f, inner, row, "Survival");
    row += 1;
    let escape_rate = if c.chased > 0 { crate::cast!(c.escaped => f32) / crate::cast!(c.chased => f32) } else { 0.0 };
    util::line(f, inner, row, Line::from(sp(format!(" chased {} times, escaped {} ({:.0}%)", c.chased, c.escaped, escape_rate * 100.0), theme::text())));
    row += 1;
    bars::labeled(f.buffer_mut(), inner, row, " escape rate", escape_rate, theme::GOOD, 16, 16);
    row += 1;
    panel::section(f, inner, row, "Threats seen");
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
        bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", pred_id.plural()), share, pred_id.color(), 16, 16);
        f.buffer_mut().set_stringn(inner.x + 40, inner.y + row, format!("{n} times"), 12, theme::dim_text());
        row += 1;
    }
    if !any {
        util::line(f, inner, row, Line::from(sp(" none seen yet", theme::dim_text())));
        row += 1;
    }
    row += 1;
    row
}

/// The legacy block: descendants and a carried notable mutation.
fn life_legacy(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature, id: CreatureId) -> u16 {
    // Legacy (C4 FR10).
    panel::section(f, inner, row, "Legacy");
    row += 1;
    let (living_desc, notable_desc) = sim.lineage.living_descendants(id, 5000);
    util::line(f, inner, row, Line::from(vec![
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
                util::line(f, inner, row, Line::from(sp(format!(" {} {} {} carries {} {:+.2}", glyphs::MUTATION, name, tag, TRAIT_NAMES[m.trait_idx], m.delta), theme::dim_text())));
            }
            None if node.children.is_empty() => {
                util::line(f, inner, row, Line::from(sp(" no descendants yet", theme::dim_text())));
            }
            None => {
                util::line(f, inner, row, Line::from(sp(" no notable mutations among descendants", theme::dim_text())));
            }
        }
        row += 1;
    }
    row += 1;
    row
}

/// Nearby kin and the recent events for this creature.
fn life_kin(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &crate::sim::Sim, c: &Creature, id: CreatureId) -> u16 {
    // Kin nearby (C4 FR10): parents, siblings and children within 15 cells.
    panel::section(f, inner, row, "Kin nearby");
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
        util::line(f, inner, row, Line::from(sp(" no kin within 15 cells", theme::dim_text())));
        row += 1;
    }
    for (d, o, rel) in kin.iter().take(5) {
        if row >= inner.height {
            break;
        }
        let dir = compass(c.x, c.y, o.x, o.y);
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", if o.adult { o.species.glyph().to_ascii_uppercase() } else { o.species.glyph() }), species_style(o.species)),
            sp(format!("{:<8}{:<7}", o.name_str(), o.tag()), theme::text()),
            sp(format!("{d:>3.0} cells {dir:<2} "), theme::dim_text()),
            sp((*rel).to_string(), theme::label()),
        ]));
        row += 1;
    }
    row += 1;

    // Recent events filtered by subject.
    panel::section(f, inner, row, "Recent events");
    row += 1;
    let evs: Vec<_> = sim.events.iter().rev().filter(|e| e.subject == Some(id)).take(6).collect();
    if evs.is_empty() {
        util::line(f, inner, row, Line::from(sp(" no events for this creature", theme::dim_text())));
        row += 1;
    }
    for e in evs {
        let stamp = format!("Y{} D{:<3} {:02}h ", e.year, e.day, e.hour);
        let avail = crate::cast!(inner.width => usize) - 2 - stamp.len() - 2;
        let text = clip(&e.text, avail);
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", e.kind.glyph()), Style::default().fg(e.kind.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(stamp, theme::dim_text()),
            sp(text, theme::text()),
        ]));
        row += 1;
    }
    row
}

/// Compass direction from `(x, y)` to `(tx, ty)` (map cells are 2:1).
pub(crate) fn compass(x: usize, y: usize, tx: usize, ty: usize) -> &'static str {
    let dx = crate::cast!(tx => i64) - crate::cast!(x => i64);
    let dy = (crate::cast!(ty => i64) - crate::cast!(y => i64)) * 2;
    let ns = if dy < -1 { "N" } else if dy > 1 { "S" } else { "" };
    let ew = if dx < -1 { "W" } else if dx > 1 { "E" } else { "" };
    match (ns, ew) {
        ("", "") => "here",
        ("N", "") => "N",
        ("S", "") => "S",
        ("", "E") => "E",
        ("", "W") => "W",
        ("N", "E") => "NE",
        ("N", "W") => "NW",
        ("S", "E") => "SE",
        _ => "SW",
    }
}

/// C7 (S03 Condition): infectious conspecifics within `contact_cheb` Chebyshev
/// cells, over 8, clamped to 0..1.
pub(crate) fn contagion_risk(sim: &crate::sim::Sim, c: &Creature) -> f32 {
    let r = crate::cast!(sim.params.disease.contact_cheb => i64);
    let n = sim
        .creatures
        .living()
        .filter(|o| o.id != c.id && o.species == c.species && crate::sim::disease::is_infectious(o))
        .filter(|o| (crate::cast!(o.x => i64) - crate::cast!(c.x => i64)).abs() <= r && (crate::cast!(o.y => i64) - crate::cast!(c.y => i64)).abs() <= r)
        .count();
    (crate::cast!(n => f32) / 8.0).clamp(0.0, 1.0)
}

pub(crate) fn local_forage(sim: &crate::sim::Sim, x: usize, y: usize) -> f32 {
    let (mut sum, mut n) = (0.0f32, 0usize);
    for dy in -3i32..=3 {
        for dx in -3i32..=3 {
            let (nx, ny) = (crate::cast!(x => i32) + dx, crate::cast!(y => i32) + dy);
            if sim.world.in_bounds(nx, ny) {
                sum += sim.world.cell(crate::cast!(nx => usize), crate::cast!(ny => usize)).vegetation;
                n += 1;
            }
        }
    }
    if n > 0 { sum / crate::cast!(n => f32) } else { 0.0 }
}

const fn trait_color(t: usize) -> Color {
    match t {
        0 => theme::INFO,
        1 => theme::DEER,
        2 => theme::ACCENT,
        3 => theme::WARN,
        4 => theme::BAD,
        5 => theme::VEGETATION,
        6 => theme::MAGENTA,
        7 => theme::LYNX,
        _ => theme::SICK,
    }
}

fn delta_style(d: f32) -> Style {
    let c = if d > 0.005 {
        theme::GOOD
    } else if d < -0.005 {
        theme::BAD
    } else {
        theme::DIM
    };
    Style::default().fg(c).bg(theme::PANEL_BG)
}

pub(crate) fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut t: String = text.chars().take(max.saturating_sub(1)).collect();
        t.push(glyphs::DOT);
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::disease::Infection;
    use crate::sim::{Params, Sim};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn screen_text(app: &AppState, screen: &dyn Screen) -> String {
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| screen.render(app, f, Rect::new(0, 0, 155, 45))).unwrap();
        let buf = terminal.backend().buffer();
        (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    }

    #[test]
    fn s03_sickness_rows() {
        let mut app = AppState::new(Params::default());
        let mut sim = Sim::new(7, Params::default());
        let id = sim.creatures.living_ids()[0];
        sim.creatures.get_mut(id).unwrap().infection =
            Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 0, ends_day: 9, severity: 0.8, source: None, outbreak: 0 });
        sim.creatures.get_mut(id).unwrap().immune_until[1] = u32::MAX;
        app.sim = Some(sim);
        let text = screen_text(&app, &Inspector::new(id));
        assert!(text.contains(&format!("{} ", glyphs::DISEASE)), "sickness row missing: {text}");
        assert!(text.contains("infectious day 1/~9"), "{text}");
        assert!(text.contains("sick — resting more, no mating"), "{text}");
        assert!(text.contains("for life"), "{text}");
        assert!(text.contains("contagion risk"), "{text}");
        assert!(text.contains("resistance cost"), "{text}");
        assert!(text.contains("fell ill with"), "{text}");
    }
}
