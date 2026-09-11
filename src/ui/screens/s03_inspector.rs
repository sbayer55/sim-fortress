//! S03: the live creature inspector (S03a prey / S03c corpse; S03b predator is
//! placeholder until C5). Mirrors the prototype three-column layout with live data.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::creatures::{Creature, CreatureId, Goal};
use crate::sim::{census, SpeciesId, TRAIT_NAMES};
use crate::ui::app::AppState;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{EventKindStyle, SpeciesStyle};
use crate::widgets::map::{self, MapOptions, Overlay};
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

const LEFT_W: u16 = 52;
const MID_W: u16 = 52;

pub struct Inspector {
    pub id: CreatureId,
}

impl Inspector {
    pub fn new(id: CreatureId) -> Self {
        Inspector { id }
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
            KeyCode::Esc => Action::Pop,
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
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
            &[("f", "follow"), ("Tab", "next creature"), ("Esc", "back")],
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

fn identity(f: &mut Frame, area: Rect, app: &AppState, c: &Creature) {
    let sim = app.sim.as_ref().unwrap();
    let title = if c.alive { "Identity & Vitals" } else { "Identity & Death" };
    let inner = panel::draw(f, area, title, panel::Kind::Outer);
    let mut row = 0u16;

    // Name line.
    let state = if !c.alive {
        sp("  DEAD", Style::default().fg(theme::BAD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
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
        sp(format!("  {} {}", sex_g, sex_name), theme::text()),
        sp(format!("  {}", if c.adult { "adult" } else { "juvenile" }), theme::text()),
        sp(format!("  diet: {}", c.species.diet()), theme::dim_text()),
    ]));
    row += 2;

    // Age bar (live: age from born_day, max from genome).
    let age = c.age_days(sim.time.day_index());
    let max_age = c.max_age_days(&sim.params.creatures);
    let age_t = age as f32 / max_age.max(1) as f32;
    let age_color = if !c.alive { theme::DIM } else { bars::vital_color(1.0 - age_t * 0.8, false) };
    bars::labeled(f.buffer_mut(), inner, row, " age", age_t, age_color, 6, 20);
    f.buffer_mut().set_stringn(inner.x + 34, inner.y + row, format!("{age} / {max_age} days"), 16, theme::dim_text());
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(format!("       {:.1} years old", age as f32 / 360.0), theme::dim_text()),
        sp(format!("   generation {}", c.generation), theme::text()),
    ]));
    row += 2;

    panel::section(f, inner, row, "Family");
    row += 1;
    let (mother, father) = match c.parents {
        Some((m, f)) => (format!("{}", m.0), format!("{}", f.0)),
        None => ("unknown".to_string(), "unknown".to_string()),
    };
    util::line(f, inner, row, Line::from(vec![
        sp(" mother  ", theme::dim_text()),
        sp(mother, theme::text()),
        sp("    father  ", theme::dim_text()),
        sp(father, theme::text()),
    ]));
    row += 2;

    panel::section(f, inner, row, "Location");
    row += 1;
    let cell = sim.world.cell(c.x, c.y);
    let (tg, tfg, _) = map::terrain_cell(cell, false);
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" ({}, {})  ", c.x, c.y), theme::text()),
        sp(sim.world.region_name(c.x, c.y), theme::title()),
        sp(format!("   {} ", tg), Style::default().fg(tfg).bg(theme::PANEL_BG)),
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

    if c.alive {
        panel::section(f, inner, row, "Vitals");
        row += 1;
        for (label, v, inv) in [("health", c.hp, false), ("hunger", c.hunger, true), ("thirst", c.thirst, true), ("energy", c.energy, false)] {
            bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", label), v, bars::vital_color(v, inv), 9, 24);
            row += 1;
        }
        row += 1;
        panel::section(f, inner, row, "Condition");
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " predation risk", 0.0, theme::DIM, 17, 16);
        row += 1;
        // Local forage: mean vegetation within 3 cells.
        let forage = local_forage(sim, c.x, c.y);
        bars::labeled(f.buffer_mut(), inner, row, " local forage", forage, theme::VEGETATION, 17, 16);
        row += 1;
        util::line(f, inner, row, Line::from(vec![sp(" nearest water", theme::dim_text()), sp(" —", theme::text()), sp("   nearest den", theme::dim_text()), sp(" —", theme::text())]));
        row += 2;

        panel::section(f, inner, row, "Behaviour");
        row += 1;
        let (need, value) = match c.goal {
            Goal::Drink => ("thirst", c.thirst),
            Goal::Graze => ("hunger", c.hunger),
            Goal::Rest => ("energy", c.energy),
            _ => ("wander", 0.0),
        };
        util::line(f, inner, row, Line::from(sp(
            format!(" {} because {} = {:.2}", c.goal.label(false), need, value),
            theme::text(),
        )));
    } else {
        panel::section(f, inner, row, "Death");
        row += 1;
        let cause = c.death.map(|d| d.cause.label()).unwrap_or("unknown");
        let age = c.age_days(sim.time.day_index());
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", glyphs::DEATH), Style::default().fg(theme::BAD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{cause}"), theme::text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            sp(format!("   lived {age} of {max_age} days ({}% of lifespan)", (age_t * 100.0).round() as u32), theme::dim_text()),
        ]));
        row += 2;
        bars::labeled(f.buffer_mut(), inner, row, " decay", c.decay, theme::CARCASS, 12, 24);
        row += 1;
        let nutrition = 1.0 - c.decay;
        let kg = (c.genome.size() * 120.0 * nutrition).round() as u32;
        let gone_in = ((1.0 - c.decay) * sim.params.creatures.carcass_decay_days as f32).ceil() as u32;
        util::line(f, inner, row, Line::from(vec![
            sp(format!("   {kg} kg of meat remaining; gone in ~{gone_in} days"), theme::dim_text()),
        ]));
    }
}

fn genome(f: &mut Frame, area: Rect, sim: &crate::sim::Sim, c: &Creature) {
    let inner = panel::draw_with_hint(f, area, "Genome", &format!("vs {} mean", c.species.plural()), panel::Kind::Outer);
    let cen = census(&sim.creatures);
    let i = SpeciesId::ALL.iter().position(|&s| s == c.species).unwrap();
    let mean = cen.genome_mean[i];
    let min = cen.genome_min[i];
    let max = cen.genome_max[i];
    let mut row = 0u16;
    util::line(f, inner, row, Line::from(vec![
        sp(" trait       individual", theme::dim_text()),
        sp("             delta", theme::dim_text()),
        sp("  species", theme::dim_text()),
    ]));
    row += 1;
    for t in 0..8 {
        let v = c.genome.0[t];
        let d = v - mean.0[t];
        let color = trait_color(t);
        bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", TRAIT_NAMES[t]), v, color, 12, 14);
        f.buffer_mut().set_stringn(inner.x + 34, inner.y + row, format!("{}{:+.2}", glyphs::PLUS_MINUS, d), 6, delta_style(d));
        let x = inner.x + 41;
        bars::range(f.buffer_mut(), x, inner.y + row, 9, min.0[t], mean.0[t], max.0[t], color);
        row += 2;
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
    row += 1;

    panel::section(f, inner, row, "Derived");
    row += 1;
    let g = &c.genome;
    let derived: Vec<(String, String)> = vec![
        ("sense range".into(), format!("{} cells", g.sense_cells())),
        ("move speed".into(), format!("{:.1} cells/tick", 0.5 + g.speed() * 2.0)),
        ("daily food need".into(), format!("{:.2} biomass", 24.0 * sim.params.creatures.hunger_per_hour(g.size(), g.metabolism(), 1.0))),
        ("max lifespan".into(), format!("{} days", c.max_age_days(&sim.params.creatures))),
    ];
    for (k, v) in derived {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {:<18}", k), theme::dim_text()),
            sp(v, theme::text()),
        ]));
        row += 1;
    }
}

fn life(f: &mut Frame, area: Rect, app: &AppState, sim: &crate::sim::Sim, c: &Creature, id: CreatureId) {
    let inner = panel::draw(f, area, "Life", panel::Kind::Outer);
    let mut row = 0u16;

    // Mini-map top-right.
    let mm_w = 23u16;
    let mm_h = 9u16;
    let mm = Rect::new(inner.right() - mm_w, inner.y, mm_w, mm_h);
    let mm_inner = panel::draw(f, mm, "Surroundings", panel::Kind::Inner);
    let ox = c.x.saturating_sub(10).min(sim.world.width() - mm_inner.width as usize);
    let oy = c.y.saturating_sub(3).min(sim.world.height() - mm_inner.height as usize);
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
        util::line(f, stats, i as u16, l);
    }
    row += mm_h + 1;

    // Recent events filtered by subject.
    panel::section(f, inner, row, "Recent events");
    row += 1;
    let evs: Vec<_> = sim.events.iter().rev().filter(|e| e.subject == Some(id)).take(10).collect();
    if evs.is_empty() {
        util::line(f, inner, row, Line::from(sp(" no events for this creature", theme::dim_text())));
        row += 1;
    }
    for e in evs {
        let stamp = format!("Y{} D{:<3} {:02}h ", e.year, e.day, e.hour);
        let avail = inner.width as usize - 2 - stamp.len() - 2;
        let text = clip(&e.text, avail);
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", e.kind.glyph()), Style::default().fg(e.kind.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(stamp, theme::dim_text()),
            sp(text, theme::text()),
        ]));
        row += 1;
    }
    let _ = app;
}

fn local_forage(sim: &crate::sim::Sim, x: usize, y: usize) -> f32 {
    let (mut sum, mut n) = (0.0f32, 0usize);
    for dy in -3i32..=3 {
        for dx in -3i32..=3 {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if sim.world.in_bounds(nx, ny) {
                sum += sim.world.cell(nx as usize, ny as usize).vegetation;
                n += 1;
            }
        }
    }
    if n > 0 { sum / n as f32 } else { 0.0 }
}

fn trait_color(t: usize) -> Color {
    match t {
        0 => theme::INFO,
        1 => theme::DEER,
        2 => theme::ACCENT,
        3 => theme::WARN,
        4 => theme::BAD,
        5 => theme::VEGETATION,
        6 => theme::MAGENTA,
        _ => theme::LYNX,
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

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut t: String = text.chars().take(max.saturating_sub(1)).collect();
        t.push(glyphs::DOT);
        t
    }
}
