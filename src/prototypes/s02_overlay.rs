//! S02: map overlays (vegetation, pressure, moisture heatmaps and the
//! sense-range ring for the selected predator) with an explanatory sidebar.

#[allow(unused_imports)]
use crate::fixtures::{EventKindStyle as _, SeasonStyle as _, SpeciesStyle as _};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::s01_map::{self, MAP_PANEL_W, MAP_ROWS, ORIGIN, SIDEBAR_W};
use super::Prototype;
use crate::fixtures::{self, Creature, Fixtures, Kind, Sex};
use crate::widgets::map::{MapOptions, Overlay};
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Vegetation,
    Pressure,
    Moisture,
    Sense,
}

pub struct MapOverlay {
    pub variant: Variant,
}

pub fn all() -> Vec<Box<dyn Prototype>> {
    vec![
        Box::new(MapOverlay { variant: Variant::Vegetation }),
        Box::new(MapOverlay { variant: Variant::Pressure }),
        Box::new(MapOverlay { variant: Variant::Moisture }),
        Box::new(MapOverlay { variant: Variant::Sense }),
    ]
}

const SELECTOR: [(&str, &str); 4] = [
    ("1", "Vegetation"),
    ("2", "Pressure"),
    ("3", "Moisture"),
    ("4", "Sense"),
];

impl Prototype for MapOverlay {
    fn id(&self) -> &'static str {
        match self.variant {
            Variant::Vegetation => "S02a",
            Variant::Pressure => "S02b",
            Variant::Moisture => "S02c",
            Variant::Sense => "S02d",
        }
    }
    fn name(&self) -> &'static str {
        "Map Overlay"
    }
    fn variant(&self) -> &'static str {
        match self.variant {
            Variant::Vegetation => "vegetation density",
            Variant::Pressure => "population pressure",
            Variant::Moisture => "water & moisture",
            Variant::Sense => "sense range of selected predator",
        }
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        let fx = fixtures::get();
        let v = self.variant;
        let overlay = match v {
            Variant::Vegetation => Overlay::Vegetation,
            Variant::Pressure => Overlay::Pressure,
            Variant::Moisture => Overlay::Moisture,
            Variant::Sense => Overlay::Sense(fx.hero_pred),
        };
        let opts = MapOptions {
            overlay,
            night: false,
            winter: false,
            cursor: None,
            follow: None,
            origin: ORIGIN,
            creatures: true,
            fade_creatures: v != Variant::Sense,
            selected_region: None,
        };

        let title = match v {
            Variant::Vegetation => "The Valley of Sunfall  · overlay: vegetation",
            Variant::Pressure => "The Valley of Sunfall  · overlay: pressure",
            Variant::Moisture => "The Valley of Sunfall  · overlay: moisture",
            Variant::Sense => "The Valley of Sunfall  · overlay: sense range",
        };
        let map_area = Rect::new(area.x, area.y, MAP_PANEL_W, MAP_ROWS);
        s01_map::map_panel(f, map_area, title, &opts);
        let side = Rect::new(area.x + MAP_PANEL_W, area.y, SIDEBAR_W, MAP_ROWS);
        match v {
            Variant::Sense => self.sense_sidebar(f, side, fx),
            _ => self.heat_sidebar(f, side, fx),
        }

        // Ticker row: latest event, as on S01a.
        let ticker_row = area.y + MAP_ROWS;
        let status_row = area.y + area.height - 1;
        let last = fx.events.last().unwrap();
        let ticker = Rect::new(area.x, ticker_row, area.width, 1);
        util::fill(f.buffer_mut(), ticker, Style::default().bg(theme::BG));
        util::line(
            f,
            ticker,
            0,
            Line::from(vec![
                Span::styled(format!(" {} ", last.kind.glyph()), Style::default().fg(last.kind.color()).bg(theme::BG).add_modifier(Modifier::BOLD)),
                Span::styled(last.text.clone(), Style::default().fg(theme::TEXT).bg(theme::BG)),
                Span::styled("   (e: full log)", Style::default().fg(theme::DIM).bg(theme::BG)),
            ]),
        );

        let keys: &[(&str, &str)] = match v {
            Variant::Sense => &[("o", "next overlay"), ("1-4", "pick"), ("Tab", "next predator"), ("i", "inspect"), ("f", "follow"), ("Esc", "close overlay")],
            _ => &[("o", "next overlay"), ("1-4", "pick"), ("k", "look"), ("Space", "pause"), ("+/-", "speed"), ("Esc", "close overlay"), ("?", "help")],
        };
        let right = format!("{}  {} day", fx.clock.label(), glyphs::SUN);
        status::render(f, Rect::new(area.x, status_row, area.width, 1), keys, &right);
    }
}

/// (title, description lines, ramp function, bar color, per-cell value)
struct HeatSpec {
    title: &'static str,
    desc: [&'static str; 2],
    ramp: fn(f32) -> Color,
    low: &'static str,
    high: &'static str,
}

fn spec(v: Variant) -> HeatSpec {
    match v {
        Variant::Vegetation => HeatSpec {
            title: "Vegetation density",
            desc: [" standing biomass per cell; prey graze", " it down, regrowth (*) restores it"],
            ramp: theme::veg,
            low: "bare",
            high: "lush",
        },
        Variant::Pressure => HeatSpec {
            title: "Population pressure",
            desc: [" traffic of prey (x0.5) and predators", " (x0.7) through each cell, last 30 days"],
            ramp: theme::heat,
            low: "quiet",
            high: "crowded",
        },
        _ => HeatSpec {
            title: "Water & moisture",
            desc: [" soil moisture; open water is shown", " saturated. Drives regrowth and thirst"],
            ramp: theme::water,
            low: "arid",
            high: "wet",
        },
    }
}

fn cell_value(v: Variant, cell: &fixtures::Cell) -> f32 {
    match v {
        Variant::Vegetation => cell.vegetation,
        Variant::Pressure => (cell.pred_pressure * 0.7 + cell.prey_pressure * 0.5).min(1.0),
        Variant::Moisture => {
            if cell.terrain.is_water() {
                1.0
            } else {
                cell.moisture
            }
        }
        Variant::Sense => 0.0,
    }
}

impl MapOverlay {
    fn heat_sidebar(&self, f: &mut Frame, area: Rect, fx: &Fixtures) {
        let v = self.variant;
        let sp = spec(v);
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let mut row = 0u16;

        // ---- what this overlay shows
        panel::section(f, inner, row, sp.title);
        row += 1;
        for d in sp.desc {
            util::line(f, inner, row, Line::from(Span::styled(d, theme::dim_text())));
            row += 1;
        }
        row += 1;

        // ---- legend ramp
        panel::section(f, inner, row, "Legend");
        row += 1;
        let ramp_w = 32u16;
        let rx = inner.x + 4;
        {
            let buf = f.buffer_mut();
            let y = inner.y + row;
            for i in 0..ramp_w {
                let t = i as f32 / (ramp_w - 1) as f32;
                let color = (sp.ramp)(t);
                let g = glyphs::shade(t);
                let g = if g == ' ' { glyphs::DIRT } else { g };
                buf.set_stringn(rx + i, y, g.to_string(), 1, Style::default().fg(color).bg(theme::dim(color, 0.75)));
            }
        }
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled("    0%", theme::text()),
            Span::styled("      25%      50%      75%    100%", theme::text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!("    {:<14}", sp.low), theme::dim_text()),
            Span::styled(format!("{:>18}", sp.high), theme::dim_text()),
        ]));
        row += 1;
        let extra = match v {
            Variant::Vegetation => format!(" {} deep water  {} rock (not shaded)", glyphs::DEEP_WATER, glyphs::ROCK),
            Variant::Pressure => format!(" {} deep water  {} rock (not shaded)", glyphs::DEEP_WATER, glyphs::ROCK),
            _ => " open water counts as 100% moisture".to_string(),
        };
        util::line(f, inner, row, Line::from(Span::styled(extra, theme::dim_text())));
        row += 2;

        // ---- per-region summary
        panel::section(f, inner, row, "By region");
        row += 1;
        let mut region_stats: Vec<(&str, f32)> = Vec::new();
        for r in &fx.world.regions {
            let (name, x0, y0, x1, y1) = (&r.0, r.1, r.2, r.3, r.4);
            let mut sum = 0.0f32;
            let mut n = 0usize;
            for y in y0..y1 {
                for x in x0..x1 {
                    sum += cell_value(v, fx.world.cell(x, y));
                    n += 1;
                }
            }
            region_stats.push((name.as_str(), if n > 0 { sum / n as f32 } else { 0.0 }));
        }
        let bar_color = (sp.ramp)(0.8);
        for (name, mean) in &region_stats {
            bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", name), *mean, bar_color, 18, 14);
            row += 1;
        }
        // World stats.
        let mut total = 0.0f32;
        let mut lo = 1.0f32;
        let mut hi = 0.0f32;
        let mut high_cells = 0usize;
        let n = fx.world.width() * fx.world.height();
        for y in 0..fx.world.height() {
            for x in 0..fx.world.width() {
                let t = cell_value(v, fx.world.cell(x, y));
                total += t;
                lo = lo.min(t);
                hi = hi.max(t);
                if t >= 0.6 {
                    high_cells += 1;
                }
            }
        }
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" mean {:>3}  min {:>3}  max {:>3}  ", util::pct(total / n as f32).trim(), util::pct(lo).trim(), util::pct(hi).trim()), theme::dim_text()),
        ]));
        row += 1;
        let (best, worst) = {
            let mut sorted = region_stats.clone();
            sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            (sorted[0], sorted[sorted.len() - 1])
        };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} cells ({}%) above 60%", high_cells, high_cells * 100 / n), theme::dim_text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" highest ", theme::dim_text()),
            Span::styled(format!("{} ({})", best.0, util::pct(best.1).trim()), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" lowest  ", theme::dim_text()),
            Span::styled(format!("{} ({})", worst.0, util::pct(worst.1).trim()), Style::default().fg(theme::WARN).bg(theme::PANEL_BG)),
        ]));
        row += 2;

        // ---- selector
        row = self.selector(f, inner, row);

        // ---- footer notes
        row += 1;
        panel::section(f, inner, row, "Reading the map");
        row += 1;
        let notes: [&str; 6] = [
            " creatures and resources are faded so",
            " the heatmap reads; [Esc] restores them.",
            " shade glyph = value:",
            "   ░ under 25%   ▒ under 50%",
            "   ▓ under 75%   █ 75% and above",
            " [k] look mode shows the exact value",
        ];
        for n in notes {
            util::line(f, inner, row, Line::from(Span::styled(n, theme::dim_text())));
            row += 1;
        }
    }

    fn selector(&self, f: &mut Frame, inner: Rect, mut row: u16) -> u16 {
        panel::section(f, inner, row, "Overlays");
        row += 1;
        let active = match self.variant {
            Variant::Vegetation => 0,
            Variant::Pressure => 1,
            Variant::Moisture => 2,
            Variant::Sense => 3,
        };
        for (i, (key, label)) in SELECTOR.iter().enumerate() {
            let swatch = match i {
                0 => theme::veg(0.8),
                1 => theme::heat(0.8),
                2 => theme::water(0.8),
                _ => theme::ACCENT,
            };
            let body = format!(" {}  {:<30}", key, label);
            let style = if i == active { theme::selected() } else { theme::text() };
            let mark = if i == active { glyphs::PLAY } else { ' ' };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {}", mark), Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG)),
                Span::styled(format!("{}{}", glyphs::SHADE_4, glyphs::SHADE_4), Style::default().fg(swatch).bg(if i == active { theme::SELECT_BG } else { theme::PANEL_BG })),
                Span::styled(body, style),
            ]));
            row += 1;
        }
        row
    }

    fn sense_sidebar(&self, f: &mut Frame, area: Rect, fx: &Fixtures) {
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let c = &fx.creatures[fx.hero_pred];
        let r = c.genome.sense_cells();
        let mut row = 0u16;

        panel::section(f, inner, row, "Sense range");
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" ring = how far the selected creature", theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" can see, hear or smell other creatures", theme::dim_text())));
        row += 2;

        // ---- selected creature
        panel::section(f, inner, row, "Selected");
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} ", c.glyph()), Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{} {}", c.name, c.tag()), theme::title()),
            Span::styled(format!("  {} adult {}", c.species.name(), if c.sex == Sex::Female { glyphs::FEMALE } else { glyphs::MALE }), theme::text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" at ({}, {})  {}", c.x, c.y, fx.world.region_name(c.x, c.y)), theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" goal: ", theme::dim_text()),
            Span::styled(c.goal.clone(), theme::text()),
        ]));
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " sense", c.genome.sense(), theme::ACCENT, 9, 20);
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" radius ", theme::dim_text()),
            Span::styled(format!("{} cells", r), theme::text()),
            Span::styled(format!("   ring {}x{} on screen", 4 * r + 1, 2 * r + 1), theme::dim_text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" (map cells are 2:1, so the ring is oval)", theme::dim_text())));
        row += 2;

        // ---- what is inside the ring
        let inside: Vec<(usize, &Creature, f32)> = fx
            .creatures
            .iter()
            .enumerate()
            .filter(|(i, o)| *i != fx.hero_pred && o.alive)
            .filter_map(|(i, o)| {
                let d = ring_dist(c, o.x, o.y);
                if d < r as f32 {
                    Some((i, o, d))
                } else {
                    None
                }
            })
            .collect();
        let dens = fx.world.dens.iter().filter(|&&(x, y)| ring_dist(c, x, y) < r as f32).count();
        let carcasses = fx.world.carcasses.iter().filter(|&&(x, y)| ring_dist(c, x, y) < r as f32).count();
        let water = {
            let mut n = 0;
            for wy in (c.y as i32 - r as i32)..=(c.y as i32 + r as i32) {
                for wx in (c.x as i32 - 2 * r as i32)..=(c.x as i32 + 2 * r as i32) {
                    if fx.world.in_bounds(wx, wy) && ring_dist(c, wx as usize, wy as usize) < r as f32 && fx.world.cell(wx as usize, wy as usize).terrain.is_water() {
                        n += 1;
                    }
                }
            }
            n
        };

        panel::section(f, inner, row, "Inside the ring");
        row += 1;
        let prey_n = inside.iter().filter(|(_, o, _)| o.kind() == Kind::Prey).count();
        let pred_n = inside.len() - prey_n;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} creatures: ", inside.len()), theme::text()),
            Span::styled(format!("{} prey", prey_n), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
            Span::styled("  ", theme::text()),
            Span::styled(format!("{} predators", pred_n), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        let mut by_species: Vec<(fixtures::SpeciesId, usize)> = Vec::new();
        for id in fixtures::SpeciesId::ALL {
            let n = inside.iter().filter(|(_, o, _)| o.species == id).count();
            if n > 0 {
                by_species.push((id, n));
            }
        }
        let mut spans = vec![Span::styled(" ", theme::text())];
        for (id, n) in &by_species {
            spans.push(Span::styled(format!("{}", id.glyph().to_ascii_uppercase()), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)));
            spans.push(Span::styled(format!(" {} {}   ", n, id.name()), theme::dim_text()));
        }
        if by_species.is_empty() {
            spans.push(Span::styled("nothing living in range", theme::dim_text()));
        }
        util::line(f, inner, row, Line::from(spans));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} ", glyphs::DEN), Style::default().fg(theme::DEN).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{} dens   ", dens), theme::dim_text()),
            Span::styled(format!("{} ", glyphs::CARCASS), Style::default().fg(theme::CARCASS).bg(theme::PANEL_BG)),
            Span::styled(format!("{} carcasses   ", carcasses), theme::dim_text()),
            Span::styled(format!("{} ", glyphs::SHALLOW_WATER), Style::default().fg(theme::SHALLOW_FG).bg(theme::PANEL_BG)),
            Span::styled(format!("{} water", water), theme::dim_text()),
        ]));
        row += 2;

        // ---- detected prey
        panel::section(f, inner, row, "Detected prey");
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled("   tag    name      dist  camo   status", theme::label())));
        row += 1;
        let mut prey: Vec<&(usize, &Creature, f32)> = inside.iter().filter(|(_, o, _)| o.kind() == Kind::Prey).collect();
        prey.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
        let max_rows = 8usize;
        if prey.is_empty() {
            util::line(f, inner, row, Line::from(Span::styled("   no prey within range", theme::dim_text())));
            row += 1;
        }
        for (_, o, d) in prey.iter().take(max_rows) {
            let hidden = o.genome.camouflage() > c.genome.sense() * 0.8;
            let (st, status_txt) = if hidden {
                (Style::default().fg(theme::DIM).bg(theme::PANEL_BG), "hidden")
            } else if o.id == fx.creatures[fx.hero_prey].id {
                (Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG), "target")
            } else {
                (Style::default().fg(theme::GOOD).bg(theme::PANEL_BG), "seen")
            };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {} ", o.glyph()), Style::default().fg(o.species.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<6} {:<9} {:>4.1}  {:.2}   ", o.tag(), o.name, d, o.genome.camouflage()), theme::text()),
                Span::styled(status_txt, st),
            ]));
            row += 1;
        }
        if prey.len() > max_rows {
            util::line(f, inner, row, Line::from(Span::styled(format!("   … and {} more", prey.len() - max_rows), theme::dim_text())));
            row += 1;
        }
        row += 1;

        // ---- selector
        row = self.selector(f, inner, row);
        row += 1;
        panel::section(f, inner, row, "Reading the map");
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} ", glyphs::RING), Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG)),
            Span::styled("ring edge   ", theme::dim_text()),
            Span::styled(" W ", Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled("selected creature", theme::dim_text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" tinted cells are within sense range", theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" [Tab] cycles through living predators", theme::dim_text())));
    }
}

/// Distance in the map's 2:1 ellipse metric (same as the sense ring in widgets/map.rs).
fn ring_dist(c: &Creature, x: usize, y: usize) -> f32 {
    let dx = (x as i32 - c.x as i32) as f32 / 2.0;
    let dy = (y as i32 - c.y as i32) as f32;
    (dx * dx + dy * dy).sqrt()
}
