//! S02d sense overlay.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use crate::sim::creatures::CreatureId;
use crate::sim::Sim;
use crate::ui::app::AppState;
use crate::ui::style::{SpeciesStyle};
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};
use super::WorldMap;

/// What is inside the sense ring: prey/predator counts, per-species tally,
/// dens, carcasses and water cells.
fn ring_tally(sim: &Sim, c: &crate::sim::creatures::Creature, r_f: f32, r: u16) -> (u32, u32, [u32; 6], usize, usize, usize) {
    let mut prey = 0u32;
    let mut pred = 0u32;
    let mut tally = [0u32; 6];
    let mut dens = 0usize;
    let mut carcasses = 0usize;
    let mut water = 0usize;
    for o in sim.creatures.living() {
        if o.id == c.id || crate::sim::dist(c.x, c.y, o.x, o.y) > r_f {
            continue;
        }
        if sim.roster().kind(o.species) == crate::sim::Kind::Prey {
            prey += 1;
        } else {
            pred += 1;
        }
        tally[o.species.index()] += 1;
    }
    for &(dx, dy) in &sim.world.dens {
        if crate::sim::dist(c.x, c.y, dx, dy) <= r_f {
            dens += 1;
        }
    }
    for &(dx, dy) in &sim.world.carcasses {
        if crate::sim::dist(c.x, c.y, dx, dy) <= r_f {
            carcasses += 1;
        }
    }
    let r_i = i32::from(r);
    for dy in -r_i..=r_i {
        for dx in -2 * r_i..=2 * r_i {
            let nx = crate::cast!(c.x => i32) + dx;
            let ny = crate::cast!(c.y => i32) + dy;
            if sim.world.in_bounds(nx, ny) && sim.world.cell(crate::cast!(nx => usize), crate::cast!(ny => usize)).terrain.is_water() {
                water += 1;
            }
        }
    }
    (prey, pred, tally, dens, carcasses, water)
}

/// The detected-prey (predator subject) or detected-predator (prey subject)
/// table for the S01 sense overlay.
#[allow(clippy::too_many_arguments)]
fn detected_table(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, c: &crate::sim::creatures::Creature, pp: &crate::sim::params::PredationParams, r_f: f32, subject_is_prey: bool) -> u16 {
        panel::section(f, inner, row, if subject_is_prey { "Detected predators" } else { "Detected prey" });
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" tag name        dist camo status", theme::dim_text())));
        row += 1;
        let mut rows: Vec<(f32, &crate::sim::Creature, &'static str)> = Vec::new();
        for o in sim.creatures.living() {
            if sim.roster().kind(o.species) == sim.roster().kind(c.species) || crate::sim::dist(c.x, c.y, o.x, o.y) > r_f {
                continue;
            }
            let status = if subject_is_prey {
                if o.hunt_target == Some(c.id) {
                    "hunting me"
                } else if crate::sim::predation::prey_detects_pred(c, o, pp) {
                    "seen"
                } else {
                    "hidden"
                }
            } else if o.id == c.hunt_target.unwrap_or(CreatureId(u32::MAX)) {
                "target"
            } else if crate::sim::predation::can_detect(c, o, &sim.world, pp) {
                "seen"
            } else {
                "hidden"
            };
            rows.push((crate::sim::dist(c.x, c.y, o.x, o.y), o, status));
        }
        rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.id.cmp(&b.1.id)));
        if rows.is_empty() {
            util::line(f, inner, row, Line::from(Span::styled(if subject_is_prey { " no predators within range" } else { " no prey within range" }, theme::dim_text())));
            row += 1;
        }
        for (d, o, status) in rows.iter().take(8) {
            let st = match *status {
                "hidden" => theme::dim_text(),
                "target" => Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG),
                "hunting me" => Style::default().fg(theme::BAD).bg(theme::PANEL_BG),
                _ => Style::default().fg(theme::GOOD).bg(theme::PANEL_BG),
            };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {} ", sim.roster().glyph(o.species)), Style::default().fg(sim.roster().color(o.species)).bg(theme::PANEL_BG)),
                Span::styled(format!("{:<5}{:<12}", o.tag(sim.roster()), o.name_str(sim.roster())), theme::text()),
                Span::styled(format!("{d:>4.1} "), theme::text()),
                Span::styled(format!("{:.2} ", o.genome.camouflage()), theme::dim_text()),
                Span::styled(*status, st),
            ]));
            row += 1;
        }
        if rows.len() > 8 {
            util::line(f, inner, row, Line::from(Span::styled(format!(" … and {} more", rows.len() - 8), theme::dim_text())));
            row += 1;
        }
        row += 1;
    row
}

impl WorldMap {
    /// S02d sidebar: the selected predator, the ring contents and the detected-prey table.
    pub(super) fn sense_sidebar(&self, f: &mut Frame<'_>, area: Rect, app: &AppState, sim: &Sim, id: CreatureId) {
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let mut row = 0u16;
        let pp = &app.params.predation;
        let Some(c) = sim.creatures.get(id) else {
            util::line(f, inner, 0, Line::from(Span::styled(" no selection", theme::dim_text())));
            return;
        };

        panel::section(f, inner, row, "Sense range");
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" how far this creature can see, hear", theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" or smell other creatures.", theme::dim_text())));
        row += 2;

        panel::section(f, inner, row, "Selected");
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} ", if c.adult { sim.roster().adult_glyph(c.species) } else { sim.roster().glyph(c.species) }), Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled(c.label(sim.roster()), theme::title()),
        ]));
        row += 1;
        let (sex_g, _) = match c.sex {
            crate::sim::Sex::Male => (glyphs::MALE, "male"),
            crate::sim::Sex::Female => (glyphs::FEMALE, "female"),
        };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(sim.roster().display_name(c.species), Style::default().fg(sim.roster().color(c.species)).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  {} {}", sex_g, if c.adult { "adult" } else { "juvenile" }), theme::text()),
            Span::styled(format!("  ({}, {})  {}", c.x, c.y, sim.world.region_name(c.x, c.y)), theme::dim_text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" goal: ", theme::dim_text()),
            Span::styled(c.goal.plain(), theme::text()),
        ]));
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " sense", c.genome.sense(), theme::ACCENT, 20, 20);
        row += 1;
        let r = c.genome.sense_cells();
        util::line(f, inner, row, Line::from(Span::styled(
            format!(" radius {} cells   ring {}×{} on screen", r, 4 * r + 1, 2 * r + 1),
            theme::dim_text(),
        )));
        row += 2;

        // Inside the ring.
        panel::section(f, inner, row, "Inside the ring");
        row += 1;
        let r_f = f32::from(r);
        let (prey, pred, tally, dens, carcasses, water) = ring_tally(sim, c, r_f, r);
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} creatures: ", prey + pred), theme::text()),
            Span::styled(format!("{prey} prey"), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
            Span::styled(format!(" {pred} predators"), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        let mut listed = Vec::new();
        for (i, id2) in sim.roster().ids().map(|id| (id.index(), id)) {
            if tally[i] > 0 {
                listed.push(format!("{}{} {}", sim.roster().adult_glyph(id2), sim.roster().glyph(id2), tally[i]));
            }
        }
        if listed.is_empty() {
            util::line(f, inner, row, Line::from(Span::styled(" nothing living in range", theme::dim_text())));
        } else {
            util::line(f, inner, row, Line::from(Span::styled(listed.join("  "), theme::text())));
        }
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(
            format!(" {} {} dens  {} {} carcasses  ~ {} water", glyphs::DEN, dens, glyphs::CARCASS, carcasses, water),
            theme::dim_text(),
        )));
        row += 2;

        // Detected prey table (predator subject) or detected predators (prey
        // subject, FR9: the prey rule, halved range while resting).
        let subject_is_prey = sim.roster().kind(c.species) == crate::sim::Kind::Prey;
        row = detected_table(f, inner, row, sim, c, pp, r_f, subject_is_prey);

        row = self.overlays_selector(f, inner, row);
        row += 1;

        panel::section(f, inner, row, "Reading the map");
        row += 1;
        for note in [" ° ring edge  W selected creature", " tinted cells are within sense range", " [Tab] cycles through living predators"] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }
    }
}
