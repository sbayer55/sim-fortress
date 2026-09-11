//! S03: creature inspector (prey, predator, corpse variants).

#[allow(unused_imports)]
use crate::fixtures::{EventKindStyle as _, SeasonStyle as _, SpeciesStyle as _};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::Prototype;
use crate::fixtures::{self, Creature, Fixtures, Kind, Sex, SpeciesId, TRAIT_NAMES};
use crate::widgets::map::{self, MapOptions, Overlay};
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Prey,
    Predator,
    Corpse,
}

pub struct Inspector {
    pub variant: Variant,
}

pub fn all() -> Vec<Box<dyn Prototype>> {
    vec![
        Box::new(Inspector { variant: Variant::Prey }),
        Box::new(Inspector { variant: Variant::Predator }),
        Box::new(Inspector { variant: Variant::Corpse }),
    ]
}

const LEFT_W: u16 = 52;
const MID_W: u16 = 52;

fn sp(s: impl Into<String>, st: Style) -> Span<'static> {
    Span::styled(s.into(), st)
}

fn species_style(id: SpeciesId) -> Style {
    Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)
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

impl Prototype for Inspector {
    fn id(&self) -> &'static str {
        match self.variant {
            Variant::Prey => "S03a",
            Variant::Predator => "S03b",
            Variant::Corpse => "S03c",
        }
    }
    fn name(&self) -> &'static str {
        "Creature Inspector"
    }
    fn variant(&self) -> &'static str {
        match self.variant {
            Variant::Prey => "prey (Hare)",
            Variant::Predator => "predator (Wolf)",
            Variant::Corpse => "corpse",
        }
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        let fx = fixtures::get();
        let idx = match self.variant {
            Variant::Prey => fx.hero_prey,
            Variant::Predator => fx.hero_pred,
            Variant::Corpse => fx.corpse,
        };
        let c = &fx.creatures[idx];
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;

        let left = Rect::new(area.x, area.y, LEFT_W, body_h);
        let mid = Rect::new(area.x + LEFT_W, area.y, MID_W, body_h);
        let right = Rect::new(area.x + LEFT_W + MID_W, area.y, area.width - LEFT_W - MID_W, body_h);

        self.identity(f, left, fx, c);
        genome(f, mid, fx, c);
        self.life(f, right, fx, c, idx);

        let right_text = format!("{} {}  {}", c.name, c.tag(), fx.clock.label());
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("f", "follow"), ("l", "lineage"), ("Tab", "next creature"), ("Esc", "back")],
            &right_text,
        );
    }
}

impl Inspector {
    fn identity(&self, f: &mut Frame, area: Rect, fx: &Fixtures, c: &Creature) {
        let title = if c.alive { "Identity & Vitals" } else { "Identity & Death" };
        let inner = panel::draw(f, area, title, panel::Kind::Outer);
        let mut row = 0u16;

        // Name line.
        let state = if !c.alive {
            sp("  DEAD", Style::default().fg(theme::BAD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
        } else if c.kind() == Kind::Predator {
            sp("  predator", Style::default().fg(theme::WOLF).bg(theme::PANEL_BG))
        } else {
            sp("  prey", Style::default().fg(theme::GOOD).bg(theme::PANEL_BG))
        };
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", c.glyph()), species_style(c.species)),
            sp(c.name.clone(), theme::title()),
            sp(format!("  {}", c.tag()), theme::label()),
            state,
        ]));
        row += 1;
        let (sex_g, sex_name) = match c.sex {
            Sex::Male => (glyphs::MALE, "male"),
            Sex::Female => (glyphs::FEMALE, "female"),
        };
        util::line(f, inner, row, Line::from(vec![
            sp("   ", theme::text()),
            sp(c.species.name(), species_style(c.species)),
            sp(format!("  {} {}", sex_g, sex_name), theme::text()),
            sp(format!("  {}", if c.adult { "adult" } else { "juvenile" }), theme::text()),
            sp(format!("  diet: {}", c.species.diet()), theme::dim_text()),
        ]));
        row += 2;

        // Age bar.
        let age_t = c.age_days as f32 / c.max_age_days.max(1) as f32;
        let age_color = if !c.alive { theme::DIM } else { bars::vital_color(1.0 - age_t * 0.8, false) };
        bars::labeled(f.buffer_mut(), inner, row, " age", age_t, age_color, 6, 20);
        f.buffer_mut().set_stringn(inner.x + 34, inner.y + row, format!("{} / {} days", c.age_days, c.max_age_days), 16, theme::dim_text());
        row += 1;
        let years = c.age_days as f32 / 360.0;
        util::line(f, inner, row, Line::from(vec![
            sp(format!("       {:.1} years old", years), theme::dim_text()),
            sp(format!("   generation {}", c.generation), theme::text()),
        ]));
        row += 2;

        panel::section(f, inner, row, "Family");
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            sp(" mother  ", theme::dim_text()),
            sp(c.parents.0.clone(), theme::text()),
            sp("    father  ", theme::dim_text()),
            sp(c.parents.1.clone(), theme::text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" offspring {}", c.offspring), theme::text()),
            sp(format!("   {} lineage: [l]", glyphs::NOTE), theme::dim_text()),
        ]));
        row += 2;

        panel::section(f, inner, row, "Location");
        row += 1;
        let cell = fx.world.cell(c.x, c.y);
        let (tg, tfg, _) = map::terrain_cell(cell, false);
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" ({}, {})  ", c.x, c.y), theme::text()),
            sp(fx.world.region_name(c.x, c.y), theme::title()),
            sp(format!("   {} ", tg), Style::default().fg(tfg).bg(theme::PANEL_BG)),
            sp(cell.terrain.name(), theme::dim_text()),
        ]));
        row += 1;
        if c.alive {
            util::line(f, inner, row, Line::from(vec![
                sp(" goal    ", theme::dim_text()),
                sp(c.goal.clone(), theme::text()),
            ]));
            row += 1;
            let target = match c.target {
                Some((tx, ty)) => {
                    let d = dist(c.x, c.y, tx, ty);
                    format!("{} ({}, {})  {} cells {}", glyphs::DIAMOND, tx, ty, d, compass(c.x, c.y, tx, ty))
                }
                None => "none".to_string(),
            };
            util::line(f, inner, row, Line::from(vec![
                sp(" target  ", theme::dim_text()),
                sp(target, theme::label()),
            ]));
            row += 1;
            let trail: Vec<String> = c.trail.iter().rev().take(5).map(|(x, y)| format!("({},{})", x, y)).collect();
            util::line(f, inner, row, Line::from(vec![
                sp(" trail   ", theme::dim_text()),
                sp(if trail.is_empty() { "no recent movement".to_string() } else { trail.join(" ") }, theme::dim_text()),
            ]));
            row += 1;
        } else {
            util::line(f, inner, row, Line::from(vec![
                sp(" carcass marked ", theme::dim_text()),
                sp(glyphs::CARCASS.to_string(), Style::default().fg(theme::CARCASS).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                sp(" on the map", theme::dim_text()),
            ]));
            row += 1;
        }
        row += 1;

        if c.alive {
            panel::section(f, inner, row, "Vitals");
            row += 1;
            for (label, v, inv) in [("health", c.hp, false), ("hunger", c.hunger, true), ("thirst", c.thirst, true), ("energy", c.energy, false)] {
                bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", label), v, bars::vital_color(v, inv), 9, 24);
                let note = vital_note(label, v);
                f.buffer_mut().set_stringn(inner.x + 40, inner.y + row, note, 10, Style::default().fg(bars::vital_color(v, inv)).bg(theme::PANEL_BG));
                row += 1;
            }
            row += 1;
            panel::section(f, inner, row, "Condition");
            row += 1;
            let risk = if c.kind() == Kind::Prey { 0.62 } else { 0.18 };
            bars::labeled(f.buffer_mut(), inner, row, " predation risk", risk, bars::vital_color(risk, true), 17, 16);
            row += 1;
            let forage = cell.vegetation;
            bars::labeled(f.buffer_mut(), inner, row, " local forage", forage, theme::VEGETATION, 17, 16);
            row += 1;
            let water_d = if c.kind() == Kind::Prey { 10 } else { 4 };
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" nearest water {} cells   nearest den {} cells", water_d, 3 + c.id % 7), theme::dim_text()),
            ]));
            row += 2;

            panel::section(f, inner, row, "Behaviour");
            row += 1;
            let lines: Vec<(String, Style)> = if c.kind() == Kind::Predator {
                vec![
                    (format!(" {} stalking: closing on Bramble h#217", glyphs::ALERT), Style::default().fg(theme::WARN).bg(theme::PANEL_BG)),
                    ("   target undetected (camo .19 vs sense .71)".into(), theme::dim_text()),
                    ("   will strike within 6 cells; energy 55% ok".into(), theme::dim_text()),
                    (format!(" {} hunger 74% drives aggressive pursuit", glyphs::NOTE), theme::text()),
                ]
            } else {
                vec![
                    (format!(" {} Ashfang w#042 stalking, 11 cells NW", glyphs::ALERT), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
                    ("   undetected (camo .19 vs sense .71)".into(), theme::dim_text()),
                    ("   flee threshold: predator within 7 cells".into(), theme::dim_text()),
                    (format!(" {} heading to water; 14 cells to go", glyphs::NOTE), theme::text()),
                ]
            };
            for (t, st) in lines {
                util::line(f, inner, row, Line::from(sp(t, st)));
                row += 1;
            }
            row += 1;
        } else {
            panel::section(f, inner, row, "Death");
            row += 1;
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" {} ", glyphs::DEATH), Style::default().fg(theme::BAD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                sp(c.cause_of_death.clone().unwrap_or_else(|| "unknown".into()), theme::text()),
            ]));
            row += 1;
            util::line(f, inner, row, Line::from(vec![
                sp("   died 2 days ago  (Year 12, Day 2, 13:00)", theme::dim_text()),
            ]));
            row += 1;
            util::line(f, inner, row, Line::from(vec![
                sp(format!("   lived {} of {} days ({}% of lifespan)", c.age_days, c.max_age_days, (age_t * 100.0).round() as u32), theme::dim_text()),
            ]));
            row += 2;
            bars::labeled(f.buffer_mut(), inner, row, " decay", c.decay, theme::CARCASS, 12, 24);
            row += 1;
            let nutrition = 1.0 - c.decay;
            bars::labeled(f.buffer_mut(), inner, row, " nutrition", nutrition, theme::WARN, 12, 24);
            row += 1;
            let kg = (c.genome.size() * 120.0 * nutrition).round() as u32;
            util::line(f, inner, row, Line::from(vec![
                sp(format!("   {} kg of meat remaining; gone in ~{} days", kg, ((1.0 - c.decay) * 6.0).ceil() as u32), theme::dim_text()),
            ]));
            row += 2;

            panel::section(f, inner, row, "Scavengers nearby");
            row += 1;
            let mut foxes: Vec<&Creature> = fx.creatures.iter().filter(|o| o.alive && o.species == SpeciesId::Fox).collect();
            foxes.sort_by_key(|o| dist(c.x, c.y, o.x, o.y));
            for o in foxes.iter().take(2) {
                let d = dist(c.x, c.y, o.x, o.y);
                util::line(f, inner, row, Line::from(vec![
                    sp(format!(" {} ", o.glyph()), species_style(o.species)),
                    sp(format!("{:<8} {:<6}", o.name, o.tag()), theme::text()),
                    sp(format!(" {} cells {}   {}", d, compass(c.x, c.y, o.x, o.y), o.goal), theme::dim_text()),
                ]));
                row += 1;
            }
            let ravens = 2 + (c.id % 3);
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" {} crows circling ({} within sight)", glyphs::NOTE, ravens), theme::dim_text()),
            ]));
            row += 2;

            panel::section(f, inner, row, "Killer");
            row += 1;
            let k = &fx.creatures[fx.hero_pred];
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" {} ", k.glyph()), species_style(k.species)),
                sp(format!("{} {}", k.name, k.tag()), theme::text()),
                sp(format!("  {} kills, chase 41 ticks", k.kills), theme::dim_text()),
            ]));
            row += 2;
        }
        row += 1;

        // Life timeline: milestones derived from age and recorded mutations.
        panel::section(f, inner, row, "Timeline");
        row += 1;
        let born_day = fx.clock.year as i64 * 360 + fx.clock.day as i64 - c.age_days as i64;
        let stamp = |d: i64| format!("Y{} D{:<3}", d / 360, d % 360);
        let mut events: Vec<(char, Color, i64, String)> = vec![
            (glyphs::BIRTH, theme::GOOD, born_day, format!("born to {} and {}", c.parents.0, c.parents.1)),
            (glyphs::UP, theme::INFO, born_day + 90, "reached adulthood".into()),
        ];
        if c.offspring > 0 {
            events.push((glyphs::BIRTH, theme::GOOD, born_day + 160, format!("first litter ({} young)", 1 + c.offspring % 3)));
        }
        if c.kind() == Kind::Predator {
            events.push((glyphs::DEATH, theme::WOLF, born_day + 140, "first kill (a vole)".into()));
            events.push((glyphs::DIAMOND, theme::TITLE, born_day + 700, "became pack leader".into()));
        } else {
            events.push((glyphs::ALERT, theme::WARN, born_day + 210, "escaped a fox in the Long Meadow".into()));
        }
        events.push((glyphs::MIGRATION, theme::ACCENT, born_day + (c.age_days as i64 / 2), format!("migrated to {}", fx.world.region_name(c.x, c.y))));
        if !c.alive {
            events.push((glyphs::DEATH, theme::BAD, 12 * 360 + 2, "killed by Ashfang w#042".into()));
        }
        events.sort_by_key(|e| e.2);
        for (g, color, day, what) in events {
            let when = stamp(day);
            if row >= inner.height {
                break;
            }
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" {} ", g), Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                sp(format!("{:<9}", when), theme::dim_text()),
                sp(what, theme::text()),
            ]));
            row += 1;
        }
    }

    fn life(&self, f: &mut Frame, area: Rect, fx: &Fixtures, c: &Creature, idx: usize) {
        let inner = panel::draw(f, area, "Life", panel::Kind::Outer);
        let mut row = 0u16;

        // Mini map in the top-right corner of the column.
        let mm_w = 23u16;
        let mm_h = 9u16;
        let mm = Rect::new(inner.right() - mm_w, inner.y, mm_w, mm_h);
        let mm_inner = panel::draw(f, mm, "Surroundings", panel::Kind::Inner);
        let ox = c.x.saturating_sub(10).min(fx.world.width() - mm_inner.width as usize);
        let oy = c.y.saturating_sub(3).min(fx.world.height() - mm_inner.height as usize);
        let opts = MapOptions {
            overlay: Overlay::None,
            night: false,
            winter: false,
            cursor: Some((c.x, c.y)),
            follow: if c.alive { Some(crate::sim::creatures::CreatureId(c.id)) } else { None },
            origin: (ox, oy),
            creatures: true,
            fade_creatures: false,
            selected_region: None,
        };
        map::render(f.buffer_mut(), mm_inner, fx, &opts);

        // Stats to the left of the mini map.
        let stats = Rect::new(inner.x, inner.y, inner.width - mm_w - 1, mm_h);
        let days_alive = c.age_days;
        let mut lines: Vec<Line> = vec![Line::from(vec![sp(" days alive  ", theme::dim_text()), sp(format!("{}", days_alive), theme::text())])];
        if c.kind() == Kind::Predator {
            lines.push(Line::from(vec![sp(" kills       ", theme::dim_text()), sp(format!("{}", c.kills), theme::title())]));
        } else {
            lines.push(Line::from(vec![sp(" escapes     ", theme::dim_text()), sp(format!("{}", 3 + c.id % 9), theme::title())]));
        }
        lines.push(Line::from(vec![sp(" offspring   ", theme::dim_text()), sp(format!("{}", c.offspring), theme::text())]));
        lines.push(Line::from(vec![sp(" grandkids   ", theme::dim_text()), sp(format!("{}", c.offspring * 2 + c.id % 5), theme::text())]));
        lines.push(Line::from(vec![sp(" mates       ", theme::dim_text()), sp(format!("{}", 1 + c.offspring / 3), theme::text())]));
        lines.push(Line::from(vec![sp(" distance    ", theme::dim_text()), sp(format!("{} cells", c.age_days * 7), theme::text())]));
        lines.push(Line::from(vec![sp(" regions     ", theme::dim_text()), sp(format!("{} visited", 2 + c.generation % 4), theme::text())]));
        for (i, l) in lines.into_iter().enumerate() {
            util::line(f, stats, i as u16, l);
        }
        row += mm_h + 1;

        // Hunt stats for the predator.
        if self.variant == Variant::Predator {
            panel::section(f, inner, row, "Hunt stats");
            row += 1;
            let attempts = 158u32;
            let rate = c.kills as f32 / attempts as f32;
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" kills {}  attempts {}  ", c.kills, attempts), theme::text()),
                sp(format!("success {:.0}%", rate * 100.0), theme::title()),
            ]));
            row += 1;
            bars::labeled(f.buffer_mut(), inner, row, " success rate", rate, theme::WOLF, 14, 20);
            row += 1;
            util::line(f, inner, row, Line::from(sp(" preferred prey", theme::dim_text())));
            row += 1;
            for (id, share) in [(SpeciesId::Deer, 0.48f32), (SpeciesId::Hare, 0.39), (SpeciesId::Vole, 0.13)] {
                let label = format!("  {} {:<5}", id.glyph().to_ascii_uppercase(), id.name());
                f.buffer_mut().set_stringn(inner.x, inner.y + row, &label, 10, species_style(id));
                bars::bar(f.buffer_mut(), inner.x + 10, inner.y + row, 20, share, id.color());
                f.buffer_mut().set_stringn(inner.x + 31, inner.y + row, format!("{:>3}%  {} kills", (share * 100.0).round() as u32, (share * c.kills as f32).round() as u32), 16, theme::text());
                row += 1;
            }
            util::line(f, inner, row, Line::from(vec![
                sp(" last kill  ", theme::dim_text()),
                sp("Thistle d#133", theme::text()),
                sp("  2 days ago, Fenlands", theme::dim_text()),
            ]));
            row += 1;
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" {} current target: ", glyphs::DIAMOND), theme::label()),
                sp("Bramble h#217", Style::default().fg(theme::HARE).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                sp(", 11 cells", theme::text()),
            ]));
            row += 1;
            util::line(f, inner, row, Line::from(sp("   avg chase 23 ticks; longest 71 ticks (Year 9)", theme::dim_text())));
            row += 2;
        } else if self.variant == Variant::Prey {
            panel::section(f, inner, row, "Survival");
            row += 1;
            util::line(f, inner, row, Line::from(vec![
                sp(" chased 12 times, escaped 11  ", theme::text()),
                sp("(92%)", theme::title()),
            ]));
            row += 1;
            bars::labeled(f.buffer_mut(), inner, row, " escape rate", 0.92, theme::GOOD, 14, 20);
            row += 1;
            util::line(f, inner, row, Line::from(sp(" threats seen", theme::dim_text())));
            row += 1;
            for (id, share) in [(SpeciesId::Fox, 0.58f32), (SpeciesId::Wolf, 0.25), (SpeciesId::Lynx, 0.17)] {
                let label = format!("  {} {:<5}", id.glyph().to_ascii_uppercase(), id.name());
                f.buffer_mut().set_stringn(inner.x, inner.y + row, &label, 10, species_style(id));
                bars::bar(f.buffer_mut(), inner.x + 10, inner.y + row, 20, share, id.color());
                f.buffer_mut().set_stringn(inner.x + 31, inner.y + row, format!("{:>3}%", (share * 100.0).round() as u32), 5, theme::text());
                row += 1;
            }
            util::line(f, inner, row, Line::from(vec![
                sp(" litters 4  ", theme::text()),
                sp("last: Y11 D302 (3 young, 2 survived)", theme::dim_text()),
            ]));
            row += 2;
        } else {
            panel::section(f, inner, row, "Legacy");
            row += 1;
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" {} offspring alive, {} descendants", c.offspring, c.offspring * 2 + 1), theme::text()),
            ]));
            row += 1;
            util::line(f, inner, row, Line::from(vec![
                sp(" Size +0.08 mutation carried by 4 descendants", theme::dim_text()),
            ]));
            row += 1;
            util::line(f, inner, row, Line::from(vec![
                sp(" carcass feeds: ", theme::dim_text()),
                sp("wolves 1  foxes 2  soil regrowth +0.12", theme::text()),
            ]));
            row += 2;
        }

        // Recent events mentioning this creature.
        panel::section(f, inner, row, "Recent events");
        row += 1;
        let mut evs: Vec<&fixtures::Event> = fx.events.iter().filter(|e| e.text.contains(&c.name)).collect();
        let want = 10usize;
        if evs.len() < want {
            // Pad with the most recent world events so the list stays full.
            for e in fx.events.iter().rev() {
                if evs.len() >= want {
                    break;
                }
                if !evs.iter().any(|x| std::ptr::eq(*x, e)) {
                    evs.insert(0, e);
                }
            }
            evs.sort_by_key(|e| (e.year, e.day, e.hour));
        }
        let n = evs.len();
        let max_rows = (inner.height - row) as usize;
        for e in evs.iter().skip(n.saturating_sub(max_rows.min(want))) {
            let stamp = format!("Y{} D{:<3} {:02}h ", e.year, e.day, e.hour);
            let avail = inner.width as usize - 2 - stamp.len() - 2;
            let text: String = if e.text.chars().count() > avail {
                let mut t: String = e.text.chars().take(avail - 1).collect();
                t.push(glyphs::DOT);
                t
            } else {
                e.text.clone()
            };
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" {} ", e.kind.glyph()), Style::default().fg(e.kind.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                sp(stamp, theme::dim_text()),
                sp(text, theme::text()),
            ]));
            row += 1;
            if row >= inner.height {
                break;
            }
        }
        if row + 1 < inner.height {
            util::line(f, inner, row, Line::from(sp("   [e] open full log filtered to this creature", theme::dim_text())));
            row += 2;
        }

        // Kin nearby: same-species creatures closest to this one.
        panel::section(f, inner, row, "Kin nearby");
        row += 1;
        let mut kin: Vec<(usize, &Creature)> = fx.creatures.iter().enumerate().filter(|(i, o)| *i != idx && o.alive && o.species == c.species).collect();
        kin.sort_by_key(|(_, o)| dist(c.x, c.y, o.x, o.y));
        for (i, (_, o)) in kin.iter().take(5).enumerate() {
            if row >= inner.height {
                break;
            }
            let d = dist(c.x, c.y, o.x, o.y);
            let rel = ["sibling", "offspring", "cousin", "unrelated", "offspring"][i];
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" {} ", o.glyph()), species_style(o.species)),
                sp(format!("{:<8} {:<6}", o.name, o.tag()), theme::text()),
                sp(format!(" {:>3} cells {:<4} ", d, compass(c.x, c.y, o.x, o.y)), theme::dim_text()),
                sp(format!("{:<10}", rel), theme::label()),
            ]));
            row += 1;
        }
    }
}

fn genome(f: &mut Frame, area: Rect, fx: &Fixtures, c: &Creature) {
    let inner = panel::draw_with_hint(f, area, "Genome", &format!("vs {} mean", c.species.plural()), panel::Kind::Outer);
    let sp_stats = fx.species.iter().find(|s| s.id == c.species).unwrap();
    let mut row = 0u16;
    util::line(f, inner, row, Line::from(vec![
        sp(" trait       individual", theme::dim_text()),
        sp("             delta", theme::dim_text()),
        sp("  species", theme::dim_text()),
    ]));
    row += 1;
    let mut above = 0;
    let mut most: (usize, f32) = (0, 0.0);
    for t in 0..8 {
        let v = c.genome.0[t];
        let mean = sp_stats.mean.0[t];
        let d = v - mean;
        if d > 0.0 {
            above += 1;
        }
        if d.abs() > most.1.abs() {
            most = (t, d);
        }
        let color = trait_color(t);
        // Row A: individual value bar + delta.
        bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", TRAIT_NAMES[t]), v, color, 12, 14);
        f.buffer_mut().set_stringn(inner.x + 34, inner.y + row, format!("{}{:+.2}", glyphs::PLUS_MINUS, d), 6, delta_style(d));
        let x = inner.x + 41;
        bars::range(f.buffer_mut(), x, inner.y + row, 9, sp_stats.min.0[t], mean, sp_stats.max.0[t], color);
        // Row B: numbers for the species distribution.
        util::line(f, inner, row + 1, Line::from(vec![
            sp(format!("{:>34}", format!("{:.2}", v)), theme::dim_text()),
            sp(format!("  {:.2}/{:.2}/{:.2}", sp_stats.min.0[t], mean, sp_stats.max.0[t]), theme::dim_text()),
        ]));
        row += 2;
    }
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} traits above species mean, {} below", above, 8 - above), theme::dim_text()),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" most divergent: ", theme::dim_text()),
        sp(TRAIT_NAMES[most.0], theme::text()),
        sp(format!(" {:+.2}", most.1), delta_style(most.1)),
    ]));
    row += 1;

    panel::section(f, inner, row, "Mutation history");
    row += 1;
    if c.mutations.is_empty() {
        util::line(f, inner, row, Line::from(sp(" none recorded", theme::dim_text())));
        row += 1;
    }
    for m in &c.mutations {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", glyphs::MUTATION), Style::default().fg(theme::INFO).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(m.clone(), theme::text()),
        ]));
        row += 1;
    }
    util::line(f, inner, row, Line::from(vec![
        sp(format!("   from {} lines; rate 0.04 per trait per birth", 2 + c.mutations.len()), theme::dim_text()),
    ]));
    row += 1;

    panel::section(f, inner, row, "Derived");
    row += 1;
    let g = &c.genome;
    let derived: Vec<(String, String)> = vec![
        ("sense range".into(), format!("{} cells", g.sense_cells())),
        ("move speed".into(), format!("{:.1} cells/tick", 0.5 + g.speed() * 2.5)),
        ("daily food need".into(), format!("{:.2} biomass", 0.2 + g.metabolism() * 0.8 + g.size() * 0.4)),
        ("max lifespan".into(), format!("{} days", c.max_age_days)),
        ("litter size".into(), format!("{} young", 1 + (g.fertility() * 4.0).round() as u32)),
        ("detection chance".into(), format!("{:.0}% at 5 cells", (1.0 - g.camouflage()) * 100.0)),
    ];
    for (k, v) in derived {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {:<18}", k), theme::dim_text()),
            sp(v, theme::text()),
        ]));
        row += 1;
    }
    row += 1;

    // Expected offspring genome if this creature bred with an average mate.
    panel::section(f, inner, row, "Offspring forecast (with an average mate)");
    row += 1;
    for t in 0..8 {
        if row >= inner.height {
            break;
        }
        let v = c.genome.0[t];
        let mean = sp_stats.mean.0[t];
        let child = (v + mean) / 2.0;
        let y = inner.y + row;
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x, y, format!(" {:<12}", TRAIT_NAMES[t]), 13, theme::text());
        bars::range(buf, inner.x + 13, y, 22, (child - 0.06).max(0.0), child, (child + 0.06).min(1.0), trait_color(t));
        buf.set_stringn(inner.x + 36, y, format!("{:.2} {}0.06", child, glyphs::PLUS_MINUS), 12, theme::dim_text());
        row += 1;
    }
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

fn vital_note(label: &str, v: f32) -> &'static str {
    match label {
        "health" => {
            if v > 0.6 { "healthy" } else if v > 0.3 { "injured" } else { "critical" }
        }
        "hunger" => {
            if v < 0.4 { "sated" } else if v < 0.7 { "peckish" } else { "starving" }
        }
        "thirst" => {
            if v < 0.4 { "fine" } else if v < 0.7 { "thirsty" } else { "parched" }
        }
        _ => {
            if v > 0.6 { "rested" } else if v > 0.3 { "tiring" } else { "exhausted" }
        }
    }
}

/// Map distance in cells (cells are ~2:1, so x counts half).
fn dist(x0: usize, y0: usize, x1: usize, y1: usize) -> u32 {
    let dx = (x0 as f32 - x1 as f32) / 2.0;
    let dy = y0 as f32 - y1 as f32;
    (dx * dx + dy * dy).sqrt().round() as u32
}

fn compass(x0: usize, y0: usize, x1: usize, y1: usize) -> &'static str {
    let dx = x1 as i32 - x0 as i32;
    let dy = y1 as i32 - y0 as i32;
    let ns = if dy < -1 { "N" } else if dy > 1 { "S" } else { "" };
    let ew = if dx < -2 { "W" } else if dx > 2 { "E" } else { "" };
    match (ns, ew) {
        ("N", "W") => "NW",
        ("N", "E") => "NE",
        ("S", "W") => "SW",
        ("S", "E") => "SE",
        ("N", _) => "N",
        ("S", _) => "S",
        (_, "W") => "W",
        (_, "E") => "E",
        _ => "here",
    }
}
