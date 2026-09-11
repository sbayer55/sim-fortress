//! S01: the main world-map view and its variants. `render_base` is reused
//! by the overlay/modal prototypes (S10-S12) as the dimmed backdrop.

#[allow(unused_imports)]
use crate::fixtures::{EventKindStyle as _, SeasonStyle as _, SpeciesStyle as _};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::Prototype;
use crate::fixtures::{self, Fixtures, Kind, Season, SpeciesId};
use crate::widgets::map::{self, MapOptions, Overlay};
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Default,
    Wide,
    Look,
    WinterNight,
    Follow,
}

pub struct WorldMap {
    pub variant: Variant,
}

/// Default viewport origin: shifted right so the hero creatures are in view.
pub const ORIGIN: (usize, usize) = (20, 0);
pub const MAP_PANEL_W: u16 = 112; // inner 110
pub const SIDEBAR_W: u16 = 43;
pub const MAP_ROWS: u16 = 42; // inner 40 = world height
pub const LOOK_CURSOR: (usize, usize) = (84, 21);

pub fn all() -> Vec<Box<dyn Prototype>> {
    vec![
        Box::new(WorldMap { variant: Variant::Default }),
        Box::new(WorldMap { variant: Variant::Wide }),
        Box::new(WorldMap { variant: Variant::Look }),
        Box::new(WorldMap { variant: Variant::WinterNight }),
        Box::new(WorldMap { variant: Variant::Follow }),
    ]
}

/// Render the S01a screen (used as the backdrop for modals).
pub fn render_base(f: &mut Frame, area: Rect) {
    WorldMap { variant: Variant::Default }.render(f, area)
}

/// Render the map panel with the given options and return the map's inner rect.
pub fn map_panel(f: &mut Frame, area: Rect, title: &str, opts: &MapOptions) -> Rect {
    let fx = fixtures::get();
    let w = fx.world.width();
    let inner_w = area.width.saturating_sub(2) as usize;
    let hint = if inner_w < w {
        format!("x {}-{} of {}   ← → scroll", opts.origin.0, opts.origin.0 + inner_w - 1, w)
    } else {
        format!("{}x{} cells", w, fx.world.height())
    };
    let inner = panel::draw_with_hint(f, area, title, &hint, panel::Kind::Outer);
    let creatures = fx.map_creatures();
    let data = map::MapData { world: &fx.world, creatures: &creatures, selected: None };
    map::render(f.buffer_mut(), inner, &data, opts);
    inner
}

impl Prototype for WorldMap {
    fn id(&self) -> &'static str {
        match self.variant {
            Variant::Default => "S01a",
            Variant::Wide => "S01b",
            Variant::Look => "S01c",
            Variant::WinterNight => "S01d",
            Variant::Follow => "S01e",
        }
    }
    fn name(&self) -> &'static str {
        "World Map"
    }
    fn variant(&self) -> &'static str {
        match self.variant {
            Variant::Default => "default",
            Variant::Wide => "wide map, sidebar collapsed",
            Variant::Look => "look / cursor mode",
            Variant::WinterNight => "winter, night",
            Variant::Follow => "following Bramble h#217",
        }
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        let fx = fixtures::get();
        let v = self.variant;
        let opts = MapOptions {
            overlay: Overlay::None,
            night: v == Variant::WinterNight,
            winter: v == Variant::WinterNight,
            cursor: if v == Variant::Look { Some(LOOK_CURSOR) } else { None },
            follow: if v == Variant::Follow { Some(fx.hero_prey) } else { None },
            origin: if v == Variant::Wide { (0, 0) } else { ORIGIN },
            creatures: true,
            fade_creatures: false,
            selected_region: None,
        };

        let ticker_row = area.y + MAP_ROWS;
        let status_row = area.y + area.height - 1;

        if v == Variant::Wide {
            let map_area = Rect::new(area.x, area.y, area.width - 3, MAP_ROWS);
            map_panel(f, map_area, "The Valley of Sunfall", &opts);
            // Collapsed sidebar gutter.
            let gutter = Rect::new(area.x + area.width - 3, area.y, 3, MAP_ROWS);
            let inner = panel::draw(f, gutter, "", panel::Kind::Outer);
            let buf = f.buffer_mut();
            for (i, ch) in "«SIDEBAR»".chars().enumerate() {
                let y = inner.y + 1 + i as u16;
                if y < inner.bottom() {
                    buf.set_stringn(inner.x, y, ch.to_string(), 1, theme::key());
                }
            }
        } else {
            let map_area = Rect::new(area.x, area.y, MAP_PANEL_W, MAP_ROWS);
            let map_inner = map_panel(f, map_area, "The Valley of Sunfall", &opts);
            let side = Rect::new(area.x + MAP_PANEL_W, area.y, SIDEBAR_W, MAP_ROWS);
            self.sidebar(f, side, fx);
            if v == Variant::Look {
                self.look_tooltip(f, map_inner, fx, &opts);
            }
        }

        // Ticker row: latest event.
        let last = fx.events.last().unwrap();
        let ticker = Rect::new(area.x, ticker_row, area.width, 1);
        util::fill(f.buffer_mut(), ticker, Style::default().bg(theme::BG));
        let ticker_text = if v == Variant::WinterNight {
            (glyphs::WINTER, theme::INFO, "Winter, Day 41 of 90: vegetation regrowth halved; 3 water cells frozen in Northmarch".to_string())
        } else {
            (last.kind.glyph(), last.kind.color(), last.text.clone())
        };
        util::line(
            f,
            ticker,
            0,
            Line::from(vec![
                Span::styled(format!(" {} ", ticker_text.0), Style::default().fg(ticker_text.1).bg(theme::BG).add_modifier(Modifier::BOLD)),
                Span::styled(ticker_text.2, Style::default().fg(theme::TEXT).bg(theme::BG)),
                Span::styled("   (e: full log)", Style::default().fg(theme::DIM).bg(theme::BG)),
            ]),
        );

        // Status bar.
        let keys: &[(&str, &str)] = match v {
            Variant::Look => &[("↑↓←→", "move cursor"), ("Enter", "inspect"), ("f", "follow"), ("Esc", "exit look")],
            Variant::Follow => &[("Esc", "stop following"), ("i", "inspect"), ("c", "center"), ("Tab", "next creature")],
            _ => &[("k", "look"), ("o", "overlay"), ("Space", "pause"), ("+/-", "speed"), ("s", "species"), ("g", "graphs"), ("e", "events"), ("?", "help")],
        };
        let right = match v {
            Variant::WinterNight => format!("{}  {} night", "Year 12, Day 41 of Winter  02:00", glyphs::MOON),
            _ => format!("{}  {} day", fx.clock.label(), glyphs::SUN),
        };
        status::render(f, Rect::new(area.x, status_row, area.width, 1), keys, &right);
    }
}

impl WorldMap {
    fn sidebar(&self, f: &mut Frame, area: Rect, fx: &Fixtures) {
        let inner = panel::draw(f, area, "Status", panel::Kind::Outer);
        let winter = self.variant == Variant::WinterNight;
        let mut row = 0u16;

        // ---- clock
        panel::section(f, inner, row, "Clock");
        row += 1;
        let (season, day, hour, sky) = if winter {
            (Season::Winter, 41, 2, (glyphs::MOON, "night"))
        } else {
            (fx.clock.season, fx.clock.day, fx.clock.hour, (glyphs::SUN, "day"))
        };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" Year {:<3} Day {:<3}", fx.clock.year, day), theme::text()),
            Span::styled(format!("   {} {}", season.glyph(), season.name()), Style::default().fg(season.color()).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {:02}:00  {} {}", hour, sky.0, sky.1), theme::text()),
            Span::styled(format!("      {} x{}  running", glyphs::FAST_STR, fx.clock.speed), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" tick {}", group(fx.clock.tick)), theme::dim_text())));
        row += 2;

        // ---- population
        panel::section(f, inner, row, "Population");
        row += 1;
        let mut prey = 0;
        let mut pred = 0;
        for sp in &fx.species {
            let count = if winter { (sp.count as f32 * 0.7) as u32 } else { sp.count };
            match sp.id.kind() {
                Kind::Prey => prey += count,
                Kind::Predator => pred += count,
            }
            let arrow = if winter { glyphs::DOWN } else { sp.trend_arrow() };
            let arrow_color = match arrow {
                glyphs::UP => theme::GOOD,
                glyphs::DOWN => theme::BAD,
                _ => theme::DIM,
            };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {} ", sp.id.glyph().to_ascii_uppercase()), Style::default().fg(sp.id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<6}", sp.id.name()), theme::text()),
                Span::styled(format!("{:>5} ", count), theme::text()),
                Span::styled(arrow.to_string(), Style::default().fg(arrow_color).bg(theme::PANEL_BG)),
                Span::styled("  ", theme::text()),
            ]));
            bars::sparkline(f.buffer_mut(), inner.x + 20, inner.y + row, 18, &sp.trend, sp.id.color());
            row += 1;
        }
        let ratio = prey as f32 / pred.max(1) as f32;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" prey {}  pred {}  ratio {:.1}:1", prey, pred, ratio), theme::dim_text()),
        ]));
        row += 2;

        // ---- resources
        panel::section(f, inner, row, "Resources");
        row += 1;
        let veg = if winter { 0.29 } else { *fx.series.vegetation.last().unwrap() };
        let water = if winter { 0.48 } else { *fx.series.water.last().unwrap() };
        bars::labeled(f.buffer_mut(), inner, row, " vegetation", veg, theme::VEGETATION, 12, 20);
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " water", water, theme::SHALLOW_FG, 12, 20);
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" carcasses {:<3} dens {:<3} regrowth {}", fx.world.carcasses.len(), fx.world.dens.len(), fx.world.seeds.len()), theme::text()),
        ]));
        row += 1;
        if winter {
            util::line(f, inner, row, Line::from(Span::styled(format!(" {} scarcity: 4 regions below forage line", glyphs::ALERT), Style::default().fg(theme::WARN).bg(theme::PANEL_BG))));
            row += 1;
        }
        row += 1;

        // ---- context section
        match self.variant {
            Variant::Follow => {
                let c = &fx.creatures[fx.hero_prey];
                panel::section(f, inner, row, "Following");
                row += 1;
                util::line(f, inner, row, Line::from(vec![
                    Span::styled(format!(" {} ", c.glyph()), Style::default().fg(c.species.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{} {}", c.name, c.tag()), theme::title()),
                    Span::styled(format!("  {} adult {}", c.species.name(), if c.sex == fixtures::Sex::Female { glyphs::FEMALE } else { glyphs::MALE }), theme::text()),
                ]));
                row += 1;
                util::line(f, inner, row, Line::from(Span::styled(format!(" at ({}, {})  {}", c.x, c.y, fx.world.region_name(c.x, c.y)), theme::dim_text())));
                row += 1;
                util::line(f, inner, row, Line::from(vec![
                    Span::styled(" goal: ", theme::dim_text()),
                    Span::styled(c.goal.clone(), theme::text()),
                    Span::styled(format!("  {} target", glyphs::DIAMOND), theme::label()),
                ]));
                row += 1;
                for (label, v, inv) in [("health", c.hp, false), ("hunger", c.hunger, true), ("thirst", c.thirst, true), ("energy", c.energy, false)] {
                    bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", label), v, bars::vital_color(v, inv), 9, 20);
                    row += 1;
                }
                util::line(f, inner, row, Line::from(vec![
                    Span::styled(format!(" {} ", glyphs::ALERT), Style::default().fg(theme::BAD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                    Span::styled("Ashfang w#042 stalking, 11 cells NW", Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
                ]));
                row += 1;
                util::line(f, inner, row, Line::from(Span::styled("   undetected (camo .19 vs sense .71)", theme::dim_text())));
                row += 2;
            }
            Variant::Look => {
                panel::section(f, inner, row, "Cursor");
                row += 1;
                let (cx, cy) = LOOK_CURSOR;
                let cell = fx.world.cell(cx, cy);
                util::line(f, inner, row, Line::from(Span::styled(format!(" ({}, {})  {}", cx, cy, fx.world.region_name(cx, cy)), theme::text())));
                row += 1;
                util::line(f, inner, row, Line::from(Span::styled(format!(" {}  elev {:.2}  veg {:.2}", cell.terrain.name(), cell.elevation, cell.vegetation), theme::dim_text())));
                row += 1;
                util::line(f, inner, row, Line::from(Span::styled(" Enter opens the creature inspector", theme::dim_text())));
                row += 2;
            }
            _ => {
                panel::section(f, inner, row, "Notable");
                row += 1;
                let w = &fx.creatures[fx.hero_pred];
                let h = &fx.creatures[fx.hero_prey];
                for c in [w, h] {
                    util::line(f, inner, row, Line::from(vec![
                        Span::styled(format!(" {} ", c.glyph()), Style::default().fg(c.species.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                        Span::styled(format!("{:<9}{:<7}", c.name, c.tag()), theme::text()),
                        Span::styled(c.goal.chars().take(17).collect::<String>(), theme::dim_text()),
                    ]));
                    row += 1;
                }
                util::line(f, inner, row, Line::from(vec![
                    Span::styled(format!(" {} ", glyphs::EXTINCTION), Style::default().fg(theme::MAGENTA).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                    Span::styled("Lynx: 5 left, at risk", Style::default().fg(theme::MAGENTA).bg(theme::PANEL_BG)),
                ]));
                row += 2;
            }
        }

        // ---- legend (compact)
        panel::section(f, inner, row, "Legend");
        row += 1;
        let legend = map::legend();
        for pair in legend.chunks(2) {
            let mut spans = vec![Span::styled(" ", theme::text())];
            for (g, color, label) in pair {
                spans.push(Span::styled(g.to_string(), Style::default().fg(*color).bg(theme::PANEL_BG)));
                spans.push(Span::styled(format!(" {:<17}", label), theme::dim_text()));
            }
            util::line(f, inner, row, Line::from(spans));
            row += 1;
        }
        for chunk in SpeciesId::ALL.chunks(3) {
            let mut spans = vec![Span::styled(" ", theme::text())];
            for id in chunk {
                spans.push(Span::styled(format!("{}{}", id.glyph().to_ascii_uppercase(), id.glyph()), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)));
                spans.push(Span::styled(format!(" {:<10}", id.name()), theme::dim_text()));
            }
            util::line(f, inner, row, Line::from(spans));
            row += 1;
        }
        util::line(f, inner, row, Line::from(Span::styled(" UPPER adult   lower juvenile", theme::dim_text())));
    }

    fn look_tooltip(&self, f: &mut Frame, map_inner: Rect, fx: &Fixtures, opts: &MapOptions) {
        let (cx, cy) = LOOK_CURSOR;
        let cell = fx.world.cell(cx, cy);
        let sx = map_inner.x + (cx - opts.origin.0) as u16;
        let sy = map_inner.y + (cy - opts.origin.1) as u16;
        let w = 34u16;
        let h = 9u16;
        // Place the tooltip to the right of the cursor, or left if it would overflow.
        let x = if sx + 3 + w <= map_inner.right() { sx + 3 } else { sx.saturating_sub(w + 2) };
        let y = if sy + 1 + h <= map_inner.bottom() { sy + 1 } else { sy.saturating_sub(h + 1) };
        let area = Rect::new(x, y, w, h);
        let inner = panel::draw(f, area, &format!("({}, {})", cx, cy), panel::Kind::Focus);
        let mut row = 0;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} ", map::terrain_cell(cell, false).0), Style::default().fg(map::terrain_cell(cell, false).1).bg(theme::PANEL_BG)),
            Span::styled(cell.terrain.name(), theme::title()),
            Span::styled(format!("  {}", fx.world.region_name(cx, cy)), theme::dim_text()),
        ]));
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " veg", cell.vegetation, theme::VEGETATION, 7, 16);
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " moist", cell.moisture, theme::SHALLOW_FG, 7, 16);
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" elevation {:.2}   prey pressure {:.2}", cell.elevation, cell.prey_pressure), theme::dim_text())));
        row += 1;
        // Creatures within 4 cells.
        let near: Vec<&fixtures::Creature> = fx
            .creatures
            .iter()
            .filter(|c| c.alive && (c.x as i32 - cx as i32).abs() <= 6 && (c.y as i32 - cy as i32).abs() <= 3)
            .take(3)
            .collect();
        util::line(f, inner, row, Line::from(Span::styled(format!(" nearby ({}):", near.len()), theme::label())));
        row += 1;
        for c in near {
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!("  {} ", c.glyph()), Style::default().fg(c.species.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{} {}  {}", c.name, c.tag(), c.goal), theme::text()),
            ]));
            row += 1;
        }
    }
}

/// Thousands separator for tick counters.
pub fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
