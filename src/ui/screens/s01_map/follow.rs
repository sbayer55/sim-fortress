//! S01e follow mode: condition, family and location panels.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use crate::sim::creatures::CreatureId;
use crate::sim::{Sim, SpeciesId};
use crate::ui::app::AppState;
use crate::ui::style::{EventKindStyle, SpeciesStyle};
use crate::widgets::map::{self};
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};
use super::WorldMap;

/// The corpse summary shown when the followed creature has died.
#[allow(clippy::too_many_arguments)]
fn corpse_summary(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, c: &crate::sim::creatures::Creature, id: CreatureId) {
    use crate::ui::screens::s03_inspector::killer_and_scavengers;

    let sp = |s: String, st: Style| Span::styled(s, st);
            // Corpse summary: cause, decay, meat, killer and scavengers.
            panel::section(f, inner, row, "Death");
            row += 1;
            let cause = c.death.map_or("unknown", |d| d.cause.label());
            let day = c.death.map(|d| format!("  day {}", d.day)).unwrap_or_default();
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" {} ", glyphs::DEATH), Style::default().fg(theme::BAD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                sp(format!("died: {cause}"), theme::text()),
                sp(day, theme::dim_text()),
            ]));
            row += 1;
            util::line(f, inner, row, Line::from(sp(
                format!(" at ({}, {})  {}", c.x, c.y, sim.world.region_name(c.x, c.y)),
                theme::dim_text(),
            )));
            row += 1;
            bars::labeled(f.buffer_mut(), inner, row, " decay", c.decay, theme::CARCASS, 8, 20);
            row += 1;
            let nutrition = 1.0 - c.decay;
            let kg = crate::cast!((c.genome.size() * 120.0 * nutrition).round() => u32);
            let gone_in = crate::cast!((nutrition * crate::cast!(sim.params.creatures.carcass_decay_days => f32)).ceil() => u32);
            util::line(f, inner, row, Line::from(sp(format!(" {kg} kg of meat; gone in ~{gone_in} days"), theme::dim_text())));
            row += 2;
            row = killer_and_scavengers(f, inner, row, sim, c);
            row += 1;
            WorldMap::follow_events(f, inner, row, sim, id);
}

/// Vitals bars, then the predator Danger line or the Hunt line.
#[allow(clippy::too_many_arguments)]
fn follow_condition(f: &mut Frame<'_>, inner: Rect, mut row: u16, app: &AppState, sim: &Sim, c: &crate::sim::creatures::Creature, id: CreatureId) -> u16 {
    use crate::sim::creatures::HuntPhase;
    use crate::sim::Kind;
    use crate::ui::screens::s03_inspector::{compass, local_forage};

    let sp = |s: String, st: Style| Span::styled(s, st);
    let species_st = |s: SpeciesId| Style::default().fg(s.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD);
        // 3. Vitals, then condition.
        panel::section(f, inner, row, "Vitals");
        row += 1;
        for (label, v, inv) in [("health", c.hp, false), ("hunger", c.hunger, true), ("thirst", c.thirst, true), ("energy", c.energy, false)] {
            bars::labeled(f.buffer_mut(), inner, row, &format!(" {label}"), v, bars::vital_color(v, inv), 9, 20);
            row += 1;
        }
        bars::labeled(f.buffer_mut(), inner, row, " risk", c.predation_risk, bars::vital_color(c.predation_risk, true), 9, 20);
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " forage", local_forage(sim, c.x, c.y), theme::VEGETATION, 9, 20);
        row += 2;

        // 4. Danger line for prey (FR12), hunt line for predators.
        if c.species.kind() == Kind::Prey {
            panel::section(f, inner, row, "Danger");
            row += 1;
            let mut nearest: Option<(usize, &crate::sim::Creature)> = None;
            for p in sim.creatures.living().filter(|p| p.species.kind() == Kind::Predator && p.hunt_target == Some(id)) {
                let d = crate::sim::cheb(c.x, c.y, p.x, p.y);
                if nearest.is_none_or(|n| d < n.0) {
                    nearest = Some((d, p));
                }
            }
            match nearest {
                Some((d, p)) => {
                    let detected = crate::sim::predation::prey_detects_pred(c, p, &app.params.predation);
                    util::line(f, inner, row, Line::from(vec![
                        sp(format!(" {} ", p.species.glyph().to_ascii_uppercase()), species_st(p.species)),
                        sp(format!("{} {}", p.name_str(), p.tag()), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
                        sp(format!("  {} cells {}", d, compass(c.x, c.y, p.x, p.y)), theme::text()),
                        sp(if detected { "  detected".into() } else { "  unseen".into() }, Style::default().fg(if detected { theme::WARN } else { theme::DIM }).bg(theme::PANEL_BG)),
                    ]));
                }
                None => {
                    util::line(f, inner, row, Line::from(vec![
                        sp(" none hunting it".into(), theme::dim_text()),
                        sp(format!("   chased {}  escaped {}", c.chased, c.escaped), theme::dim_text()),
                    ]));
                }
            }
        } else {
            panel::section(f, inner, row, "Hunt");
            row += 1;
            if let Some(t) = c.hunt_target.and_then(|t| sim.creatures.get(t)) {
                let phase = match c.hunt_phase {
                    HuntPhase::Stalk => "stalking",
                    HuntPhase::Chase => "chasing",
                    HuntPhase::Eat => "eating",
                };
                let d = crate::sim::cheb(c.x, c.y, t.x, t.y);
                let seen = t.alive && crate::sim::predation::prey_detects_pred(t, c, &app.params.predation);
                util::line(f, inner, row, Line::from(vec![
                    sp(format!(" {phase} "), theme::text()),
                    sp(format!("{} ", if t.adult { t.species.glyph().to_ascii_uppercase() } else { t.species.glyph() }), species_st(t.species)),
                    sp(format!("{} {}", t.name_str(), t.tag()), theme::title()),
                    sp(format!("  {} cells {}", d, compass(c.x, c.y, t.x, t.y)), theme::text()),
                    sp(if !t.alive { String::new() } else if seen { "  seen".into() } else { "  unseen".into() }, Style::default().fg(if seen { theme::WARN } else { theme::GOOD }).bg(theme::PANEL_BG)),
                ]));
            } else {
                let success = if c.attempts > 0 { crate::cast!(c.kills => f32) / crate::cast!(c.attempts => f32) * 100.0 } else { 0.0 };
                util::line(f, inner, row, Line::from(vec![
                    sp(" not hunting".into(), theme::dim_text()),
                    sp(format!("   kills {}  attempts {}  {:.0}%", c.kills, c.attempts, success), theme::dim_text()),
                ]));
            }
        }
        row += 2;
    row
}

/// Parents, breeding state and nearby kin.
#[allow(clippy::too_many_arguments)]
fn follow_family(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, c: &crate::sim::creatures::Creature, id: CreatureId) -> u16 {
    use crate::ui::screens::s03_inspector::{clip, compass, kin_name};

    let sp = |s: String, st: Style| Span::styled(s, st);
    let species_st = |s: SpeciesId| Style::default().fg(s.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD);
        // 5. Family: parents, breeding state, kin nearby.
        panel::section(f, inner, row, "Family");
        row += 1;
        let (mother, father) = match c.parents {
            Some((m, fa)) => (kin_name(sim, m), kin_name(sim, fa)),
            None => ("founder".to_string(), "founder".to_string()),
        };
        util::line(f, inner, row, Line::from(vec![
            sp(" mother ".into(), theme::dim_text()),
            sp(format!("{:<13}", clip(&mother, 13)), theme::text()),
            sp(" father ".into(), theme::dim_text()),
            sp(clip(&father, 13), theme::text()),
        ]));
        row += 1;
        let breeding = if let Some(due) = c.pregnant_due {
            (format!("pregnant, due in {} h", due.saturating_sub(sim.time.tick)), theme::GOOD)
        } else if !c.adult {
            ("not yet adult".to_string(), theme::DIM)
        } else if sim.time.tick < c.cooldown_until {
            (format!("cooldown {} h", c.cooldown_until - sim.time.tick), theme::DIM)
        } else {
            ("ready to breed".to_string(), theme::TEXT_BRIGHT)
        };
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" offspring {}   ", c.offspring), theme::text()),
            sp(breeding.0, Style::default().fg(breeding.1).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        let sibling_of = |o: &crate::sim::Creature| {
            c.parents.is_some() && o.parents.is_some() && (o.parents.map(|p| p.0) == c.parents.map(|p| p.0) || o.parents.map(|p| p.1) == c.parents.map(|p| p.1))
        };
        let mut kin: Vec<(f32, &crate::sim::Creature, &str)> = Vec::new();
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
            util::line(f, inner, row, Line::from(sp(" no kin within 15 cells".into(), theme::dim_text())));
            row += 1;
        }
        for (d, o, rel) in kin.iter().take(3) {
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" {} ", if o.adult { o.species.glyph().to_ascii_uppercase() } else { o.species.glyph() }), species_st(o.species)),
                sp(format!("{:<8}{:<6}", o.name_str(), o.tag()), theme::text()),
                sp(format!("{:>3.0} cells {:<2} ", d, compass(c.x, c.y, o.x, o.y)), theme::dim_text()),
                sp((*rel).to_string(), theme::label()),
            ]));
            row += 1;
        }
        row += 1;
    row
}

/// Location, goal with its reason, and target with distance and heading.
fn follow_location(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, c: &crate::sim::creatures::Creature) -> u16 {
    use crate::sim::creatures::{Goal, RestReason};
    use crate::ui::screens::s03_inspector::{compass, kin_name};

    let sp = |s: String, st: Style| Span::styled(s, st);
    let age = c.age_days(sim.time.day_index());
        // 2. Location, goal with its reason, target with distance and heading.
        panel::section(f, inner, row, "Location");
        row += 1;
        let cell = sim.world.cell(c.x, c.y);
        let (tg, tfg, _) = map::terrain_cell(cell, false);
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" ({}, {})  ", c.x, c.y), theme::text()),
            sp(sim.world.region_name(c.x, c.y).to_string(), theme::title()),
            sp(format!("  {tg} "), Style::default().fg(tfg).bg(theme::PANEL_BG)),
            sp(cell.terrain.name().to_string(), theme::dim_text()),
        ]));
        row += 1;
        let in_den = sim.world.dens.iter().any(|&(x, y)| x == c.x && y == c.y);
        let goal_label = c.goal.label(in_den);
        let follows_mother = !c.adult
            && c.mother.is_some_and(|m| sim.creatures.get(m).is_some_and(|m| m.alive))
            && age < sim.params.genetics.follow_mother_days;
        let reason = match c.goal {
            Goal::Drink => format!("thirst {:.2}", c.thirst),
            Goal::Graze | Goal::Hunt | Goal::Scavenge => format!("hunger {:.2}", c.hunger),
            Goal::Rest => match c.rest_reason {
                Some(RestReason::Night) => "night".to_string(),
                Some(RestReason::Forced) => "exhausted".to_string(),
                _ => format!("energy {:.2}", c.energy),
            },
            Goal::Mate => match c.mate_id {
                Some(m) => format!("with {}", kin_name(sim, m)),
                None => "seeking a mate".to_string(),
            },
            Goal::Wander if follows_mother => "near its mother".to_string(),
            _ => String::new(),
        };
        util::line(f, inner, row, Line::from(vec![
            sp(" goal: ".into(), theme::dim_text()),
            sp(goal_label.to_string(), theme::text()),
            sp(if reason.is_empty() { String::new() } else { format!("  {} {}", glyphs::DOT, reason) }, theme::dim_text()),
        ]));
        row += 1;
        let target = match c.target {
            Some((tx, ty)) => {
                let d = crate::sim::dist(c.x, c.y, tx, ty);
                format!("{} ({}, {})  {:.0} cells {}", glyphs::DIAMOND, tx, ty, d, compass(c.x, c.y, tx, ty))
            }
            None => "none".to_string(),
        };
        util::line(f, inner, row, Line::from(vec![sp(" target: ".into(), theme::dim_text()), sp(target, theme::label())]));
        row += 2;
    row
}

impl WorldMap {
    /// S01e sidebar: the followed creature — identity, location and intent,
    /// vitals, threat/hunt line, family and its recent events (or the corpse
    /// summary once it has died).
    pub(super) fn follow_sidebar(f: &mut Frame<'_>, area: Rect, app: &AppState, sim: &Sim, id: CreatureId) {
        use crate::sim::Sex;

        let inner = panel::draw(f, area, "Following", panel::Kind::Outer);
        let mut row = 0u16;
        let Some(c) = sim.creatures.get(id) else {
            util::line(f, inner, 0, Line::from(Span::styled(" creature gone", theme::dim_text())));
            return;
        };
        let sp = |s: String, st: Style| Span::styled(s, st);
        let species_st = |s: SpeciesId| Style::default().fg(s.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD);

        // 1. Identity: name, species, sex, stage, generation and age.
        util::line(f, inner, row, Line::from(vec![
            Span::styled(
                format!(" {} ", if c.alive { c.species.glyph().to_ascii_uppercase() } else { glyphs::CARCASS }),
                Style::default().fg(if c.alive { c.species.color() } else { theme::CARCASS }).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD),
            ),
            sp(format!("{} {}", c.name_str(), c.tag()), theme::title()),
            if c.alive { sp(String::new(), theme::text()) } else { sp("  carcass".into(), Style::default().fg(theme::BAD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)) },
        ]));
        row += 1;
        let (sex_g, sex_name) = match c.sex {
            Sex::Male => (glyphs::MALE, "male"),
            Sex::Female => (glyphs::FEMALE, "female"),
        };
        util::line(f, inner, row, Line::from(vec![
            sp("   ".into(), theme::text()),
            sp(c.species.name().to_string(), species_st(c.species)),
            sp(format!("  {sex_g} {sex_name}"), theme::text()),
            sp(format!("  {}", if c.adult { "adult" } else { "juvenile" }), theme::text()),
            sp(format!("  gen {}", c.generation), theme::dim_text()),
        ]));
        row += 1;
        let age = c.age_days(sim.time.day_index());
        let max_age = c.max_age_days(&sim.params.creatures, &sim.params.genetics);
        let age_t = crate::cast!(age => f32) / crate::cast!(max_age.max(1) => f32);
        let age_color = if c.alive { bars::vital_color(1.0 - age_t * 0.8, false) } else { theme::DIM };
        bars::labeled(f.buffer_mut(), inner, row, " age", age_t, age_color, 6, 16);
        f.buffer_mut().set_stringn(inner.x + 24, inner.y + row, format!("{age} / {max_age} days"), 17, theme::dim_text());
        row += 2;

        if !c.alive {
            corpse_summary(f, inner, row, sim, c, id);
            return;
        }

        row = follow_location(f, inner, row, sim, c);

        row = follow_condition(f, inner, row, app, sim, c, id);

        row = follow_family(f, inner, row, sim, c, id);

        // 6. Recent events fill whatever rows remain.
        Self::follow_events(f, inner, row, sim, id);
    }

    /// The followed creature's most recent events, newest first, filling the
    /// panel from `row` to its bottom edge.
    fn follow_events(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, id: CreatureId) {
        use crate::ui::screens::s03_inspector::clip;
        if row + 1 >= inner.height {
            return;
        }
        panel::section(f, inner, row, "Recent events");
        row += 1;
        let take = crate::cast!((inner.height - row) => usize);
        let evs: Vec<_> = sim.events.iter().rev().filter(|e| e.subject == Some(id)).take(take).collect();
        if evs.is_empty() {
            util::line(f, inner, row, Line::from(Span::styled(" no events for this creature", theme::dim_text())));
            return;
        }
        for e in evs {
            let stamp = format!("Y{} D{:<3} {:02}h ", e.year, e.day, e.hour);
            let avail = (crate::cast!(inner.width => usize)).saturating_sub(3 + stamp.len());
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {} ", e.kind.glyph()), Style::default().fg(e.kind.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                Span::styled(stamp, theme::dim_text()),
                Span::styled(clip(&e.text, avail), theme::text()),
            ]));
            row += 1;
        }
    }
}
