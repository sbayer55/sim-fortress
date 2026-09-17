//! S06: the live ecology / resources screen.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::params::ScarcityThresholds;
use crate::sim::{Season, Terrain, World};
use crate::ui::app::AppState;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{clock_status, SeasonStyle};
use crate::widgets::map;
use crate::widgets::Constraint::Fixed;
use crate::widgets::{bars, panel, util, Bar, Column, Component, Panel, StatusBar, Table, TableCell, TableRow};
use crate::{glyphs, theme};

/// The S06 region status rule (FR7).
pub fn region_status(veg: f32, prey: u32, prey_configured: bool, th: &ScarcityThresholds) -> &'static str {
    let crowded = crate::cast!(prey => f32) > th.crowded_prey_per_veg * veg.max(1e-6);
    if veg < th.scarce {
        "Scarce"
    } else if veg < th.strained || (crowded && veg < th.plenty) {
        "Strained"
    } else if veg >= th.plenty && (!prey_configured || prey >= th.plenty_min_prey) {
        "Plenty"
    } else {
        "Stable"
    }
}

/// Active cases in one region that flip its status word to `Outbreak` (C7 FR13).
pub const OUTBREAK_CASES: u32 = 5;

#[derive(Debug)]
pub struct Ecology {
    sort: usize,
    selected: usize,
}

impl Default for Ecology {
    fn default() -> Self {
        Self::new()
    }
}

impl Ecology {
    pub const fn new() -> Self {
        Self { sort: 0, selected: 1 }
    }

    fn sorted_regions(&self, world: &World) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..world.regions.len()).collect();
        match self.sort {
            1 => idx.sort_by(|&a, &b| {
                let va = crate::sim::ecology::region_land_veg_mean(world, a);
                let vb = crate::sim::ecology::region_land_veg_mean(world, b);
                vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal)
            }),
            2 => idx.sort_by(|&a, &b| {
                let va = crate::sim::ecology::region_display_moisture_mean(world, a);
                let vb = crate::sim::ecology::region_display_moisture_mean(world, b);
                vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal)
            }),
            3 => idx.sort_by_key(|&i| {
                let veg = crate::sim::ecology::region_land_veg_mean(world, i);
                let th = &ScarcityThresholds::default();
                // Sort by status severity: Scarce < Strained < Stable < Plenty.
                match region_status(veg, 0, true, th) {
                    "Scarce" => 0,
                    "Strained" => 1,
                    "Stable" => 2,
                    _ => 3,
                }
            }),
            _ => {}
        }
        idx
    }
}

impl Screen for Ecology {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Char('r') => {
                self.sort = (self.sort + 1) % 4;
                Action::None
            }
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                Action::None
            }
            KeyCode::Down => {
                self.selected = (self.selected + 1).min(7);
                Action::None
            }
            KeyCode::Tab => Action::None,
            KeyCode::Enter => {
                if let Some(sim) = &app.sim {
                    let (cx, cy) = sim.world.region_centre(self.selected);
                    app.centre_viewport_on(cx, cy);
                }
                Action::Pop
            }
            KeyCode::Esc => Action::Pop,
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else {
            return;
        };
        let world = &sim.world;
        let time = &sim.time;
        let ecology = &sim.params.ecology;

        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let totals_area = Rect::new(area.x, area.y, (area.width * 2).div_euclid(3), body_h.div_euclid(2));
        let season_area = Rect::new(area.x + totals_area.width, area.y, area.width - totals_area.width, body_h.div_euclid(2));
        let regions_area = Rect::new(area.x, area.y + totals_area.height, area.width, body_h - totals_area.height);

        totals(f, totals_area, sim, world);
        season(f, season_area, sim, time, ecology);
        regions(f, regions_area, app, world, self);

        let (right, right_fg) = clock_status(time, app.params.ui.day_night_tint);
        StatusBar::new(&[("r", "sort regions"), ("↑↓", "select"), ("Enter", "jump"), ("Esc", "back")]).right(&right).right_color(right_fg).render(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1));
    }
}

fn totals(f: &mut Frame<'_>, area: Rect, sim: &crate::sim::Sim, world: &World) {
    let inner = panel::draw_with_hint(f, area, "Totals", "sparklines = last 240 days", panel::Kind::Outer);
    let mut row = 0u16;
    let series = sim.series.samples();
    let last = sim.series.last();

    panel::section(f, inner, row, "Now");
    row += 1;
    let veg = last.map_or(0.0, |s| s.veg_mean);
    let water = last.map_or(0.0, |s| s.water_level);
    bars::labeled(f.buffer_mut(), inner, row, " vegetation", veg, theme::VEGETATION, 13, 20);
    row += 1;
    bars::labeled(f.buffer_mut(), inner, row, " water", water, theme::SHALLOW_FG, 13, 20);
    row += 1;
    util::line(f, inner, row, Line::from(Span::styled(
        format!(" carcasses {}    dens {}    regrowth {}", world.carcasses.len(), world.dens.len(), world.seeds.len()),
        theme::text(),
    )));
    row += 2;

    panel::section(f, inner, row, "Terrain");
    row += 1;
    let mut counts = [0usize; 10];
    for c in &world.cells {
        counts[crate::cast!(c.terrain => usize)] += 1;
    }
    let total = crate::cast!(world.cells.len().max(1) => f32);
    let mut kinds: Vec<Terrain> = [Terrain::DeepWater, Terrain::ShallowWater, Terrain::Sand, Terrain::Dirt, Terrain::GrassSparse, Terrain::Grass, Terrain::GrassDense, Terrain::Forest, Terrain::Rock, Terrain::Marsh].to_vec();
    kinds.sort_by_key(|t| std::cmp::Reverse(counts[crate::cast!(*t => usize)]));
    let max = crate::cast!(counts.iter().copied().max().unwrap_or(1).max(1) => f32);
    for t in kinds {
        let n = counts[crate::cast!(t => usize)];
        let (g, fg, _bg) = map::terrain_cell(&crate::sim::Cell { terrain: t, biome: crate::sim::world::Biome::Grassland, elevation: 0.0, moisture: 0.0, temperature: 0.5, vegetation: 0.0, prey_pressure: 0.0, pred_pressure: 0.0, dried_from: None, parasite_load: 0.0 }, false);
        let buf = f.buffer_mut();
        let y = inner.y + row;
        buf.set_stringn(inner.x + 1, y, format!("{g} "), 2, Style::default().fg(fg).bg(theme::PANEL_BG));
        buf.set_stringn(inner.x + 3, y, format!("{:<14}", t.name()), 14, theme::text());
        bars::bar(buf, inner.x + 17, y, 24, crate::cast!(n => f32) / max, fg);
        buf.set_stringn(inner.x + 43, y, format!("{:>5} {:>3}%", n, crate::cast!((crate::cast!(n => f32) / total * 100.0).round() => u32)), 9, theme::dim_text());
        row += 1;
    }
    let _ = series;
}

fn season(f: &mut Frame<'_>, area: Rect, sim: &crate::sim::Sim, time: &crate::sim::Time, ecology: &crate::sim::params::EcologyParams) {
    let inner = panel::draw(f, area, "Season", panel::Kind::Outer);
    let mut row = 0u16;
    let season = time.season();

    util::line(f, inner, row, Line::from(vec![
        Span::styled(format!("{} {}  ", season.glyph(), season.name()), Style::default().fg(season.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        Span::styled(format!("day {} of {}  year {}", time.day_of_season(), time.season_days, time.year()), theme::text()),
    ]));
    row += 1;
    let progress = crate::cast!((time.day_of_season() - 1) => f32) / crate::cast!(time.season_days => f32);
    bars::labeled(f.buffer_mut(), inner, row, " progress", progress, season.color(), 9, 30);
    row += 2;

    row = season_modifiers(f, inner, row, season, ecology);
    row = season_forecast(f, inner, row, sim, time, season);
    day_strip(f, inner, row, time);
}

fn season_modifiers(f: &mut Frame<'_>, inner: Rect, mut row: u16, season: Season, ecology: &crate::sim::params::EcologyParams) -> u16 {
    panel::section(f, inner, row, "Modifiers");
    row += 1;
    util::line(f, inner, row, Line::from(Span::styled(" season  regrowth  evap  metabolism  forage", theme::dim_text())));
    row += 1;
    for s in Season::ALL {
        let regrowth = ecology.season_regrowth.get(&s).copied().unwrap_or(1.0);
        let evap = ecology.season_evaporation.get(&s).copied().unwrap_or(1.0);
        let metab = ecology.season_metabolism.get(&s).copied().unwrap_or(1.0);
        let cap = ecology.season_cap.get(&s).copied().unwrap_or(1.0);
        let forage = if cap >= 1.0 { "lush" } else if cap >= 0.9 { "drying" } else if cap >= 0.6 { "fading" } else { "scarce" };
        let active = s == season;
        let sty = if active { theme::selected() } else { theme::text() };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} {}", if active { "►" } else { " " }, s.name()), sty),
            Span::styled(format!("  {regrowth:.1}x"), sty),
            Span::styled(format!("  {evap:.1}x"), sty),
            Span::styled(format!("  {metab:.1}x"), sty),
            Span::styled(format!("  {forage}"), sty),
        ]));
        row += 1;
    }
    row += 1;
    row
}

fn season_forecast(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &crate::sim::Sim, time: &crate::sim::Time, season: Season) -> u16 {
    panel::section(f, inner, row, "Forecast");
    row += 1;
    let days_to_frost = (crate::cast!(Season::Winter => i32) - crate::cast!(season => i32)).rem_euclid(4) * crate::cast!(time.season_days => i32);
    let frost = if season == Season::Winter { " frost now".to_string() } else { format!(" frost in {days_to_frost} days") };
    util::line(f, inner, row, Line::from(Span::styled(frost, theme::text())));
    row += 1;
    let pp = &sim.params.predation;
    util::line(f, inner, row, Line::from(Span::styled(
        format!(" forage line {:.2} — prey herds leave after {} days below it", pp.migrate_veg, pp.migrate_days),
        theme::dim_text(),
    )));
    row += 1;
    util::line(f, inner, row, Line::from(Span::styled(
        format!(" pressure line {:.2} — herds leave; packs leave at <{} prey", pp.migrate_pressure, pp.migrate_prey_min),
        theme::dim_text(),
    )));
    row += 1;
    let year = time.year();
    let migrations = sim.events.iter().filter(|e| e.kind == crate::sim::EventKind::Migration && e.year == year).count();
    util::line(f, inner, row, Line::from(vec![
        Span::styled(format!(" {} ", glyphs::MIGRATION), Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG)),
        Span::styled(format!("{migrations} migrations this year"), theme::text()),
    ]));
    row += 1;
    if let Some(e) = sim.events.iter().rev().find(|e| e.kind == crate::sim::EventKind::Drought || e.kind == crate::sim::EventKind::DroughtEased) {
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} ", glyphs::DROUGHT), Style::default().fg(theme::WARN).bg(theme::PANEL_BG)),
            Span::styled(e.text.clone(), theme::text()),
        ]));
        row += 1;
    }
    row
}

fn day_strip(f: &mut Frame<'_>, inner: Rect, mut row: u16, time: &crate::sim::Time) -> u16 {
    panel::section(f, inner, row, "Day");
    row += 1;
    // 24-cell hour strip.
    for h in 0..24u32 {
        let night = h < time.sunrise_hour || h >= time.sunset_hour;
        let ch = if h == time.hour() { glyphs::FULL_BLOCK } else if night { glyphs::SHADE_1 } else { glyphs::SHADE_3 };
        let color = if night { theme::DEEP_WATER_FG } else { theme::ACCENT };
        if let Some(c) = f.buffer_mut().cell_mut((inner.x + 1 + crate::cast!(h => u16), inner.y + row)) {
            c.set_char(ch);
            c.set_style(Style::default().fg(if h == time.hour() { theme::TEXT_BRIGHT } else { color }).bg(theme::PANEL_BG));
        }
    }
    row += 1;
    let sky = if time.is_night() { glyphs::MOON } else { glyphs::SUN };
    util::line(f, inner, row, Line::from(Span::styled(format!(" now {} {} {}", time.hour_label(), sky, if time.is_night() { "night" } else { "day" }), theme::dim_text())));
    row
}

/// Living prey, predator and sick counts in region `ri`.
fn region_counts(sim: &crate::sim::Sim, world: &World, ri: usize) -> (u32, u32, u32) {
    let (mut prey_n, mut pred_n, mut sick_n) = (0u32, 0u32, 0u32);
    for c in sim.creatures.living() {
        if world.region_index(c.x, c.y) != ri {
            continue;
        }
        if sim.roster().kind(c.species) == crate::sim::Kind::Prey {
            prey_n += 1;
        } else {
            pred_n += 1;
        }
        if c.infection.is_some() {
            sick_n += 1;
        }
    }
    (prey_n, pred_n, sick_n)
}

/// The region columns after the Marker: name, cells, water, the two bars
/// with their values, the three counts, pressure and status.
const REGION_COLUMNS: [Column; 17] = [
    Column::titled("region", Fixed(17)),
    Column::titled("cells", Fixed(6)).right(),
    Column::titled("water", Fixed(7)).right(),
    Column::new(Fixed(1)),
    Column::titled(" vegetation", Fixed(20)),
    Column::new(Fixed(1)),
    Column::new(Fixed(4)),
    Column::new(Fixed(1)),
    Column::titled(" moisture", Fixed(20)),
    Column::new(Fixed(1)),
    Column::new(Fixed(4)),
    Column::new(Fixed(3)),
    Column::titled("prey", Fixed(5)).right(),
    Column::titled("pred", Fixed(5)).right(),
    Column::titled("sick", Fixed(5)).right(),
    Column::titled("pressure", Fixed(10)).right(),
    Column::titled("   status", Fixed(12)),
];

/// One region row: counts, bars and status.
fn region_row(ri: usize, r: &(String, usize, usize, usize, usize), world: &World, app: &AppState, th: &ScarcityThresholds, prey_configured: bool) -> TableRow<'static> {
    let veg = crate::sim::ecology::region_land_veg_mean(world, ri);
    let moist = crate::sim::ecology::region_display_moisture_mean(world, ri);
    let cells = world.region_size(ri);
    let water = region_water_cells(world, ri);
    let (prey_n, pred_n, sick_n) = match &app.sim {
        Some(sim) => region_counts(sim, world, ri),
        None => (0, 0, 0),
    };
    let (label, color) = region_status_label(veg, sick_n, prey_configured, th);
    let pressure = region_pressure(world, ri);
    TableRow::new([
        TableCell::text(r.0.clone()),
        TableCell::text(cells.to_string()),
        TableCell::styled(format!("{:.0}%", crate::cast!(water => f32) / crate::cast!(cells.max(1) => f32) * 100.0), theme::SHALLOW_FG),
        TableCell::Blank,
        TableCell::widget(Bar::new(veg).color(theme::veg(veg))),
        TableCell::Blank,
        TableCell::text(format!("{veg:.2}")),
        TableCell::Blank,
        TableCell::widget(Bar::new(moist).color(theme::water(moist))),
        TableCell::Blank,
        TableCell::text(format!("{moist:.2}")),
        TableCell::Blank,
        TableCell::styled(prey_n.to_string(), theme::PREY),
        TableCell::styled(pred_n.to_string(), theme::PRED),
        TableCell::styled(sick_n.to_string(), if sick_n > 0 { theme::SICK } else { theme::DIM }),
        TableCell::text(format!("{pressure:.2}")),
        TableCell::styled(format!("   {label}"), color),
    ])
}

/// Water cells inside a region rectangle.
fn region_water_cells(world: &World, ri: usize) -> usize {
    world.region_cells(ri).filter(|&(x, y)| world.cell(x, y).terrain.is_water()).count()
}

/// Mean predator pressure over the region's land cells.
fn region_pressure(world: &World, ri: usize) -> f32 {
    let mut psum = 0.0f32;
    let mut pn = 0usize;
    for (xx, yy) in world.region_cells(ri) {
        let cell = world.cell(xx, yy);
        if !cell.terrain.is_water() {
            psum += cell.pred_pressure;
            pn += 1;
        }
    }
    if pn > 0 {
        psum / crate::cast!(pn => f32)
    } else {
        0.0
    }
}

/// The scarcity label and colour, with an outbreak outranking all but Scarce.
fn region_status_label(veg: f32, sick_n: u32, prey_configured: bool, th: &ScarcityThresholds) -> (&'static str, ratatui::style::Color) {
    let status = region_status(veg, 0, prey_configured, th);
    match status {
        "Scarce" => ("Scarce", theme::BAD),
        _ if sick_n >= OUTBREAK_CASES => ("Outbreak", theme::SICK),
        "Strained" => ("Strained", theme::WARN),
        "Plenty" => ("Plenty", theme::GOOD),
        _ => ("Stable", theme::TEXT),
    }
}

const SORT_NAMES: [&str; 4] = ["name", "vegetation", "moisture", "status"];

fn regions(f: &mut Frame<'_>, area: Rect, app: &AppState, world: &World, screen: &Ecology) {
    let buf = f.buffer_mut();
    let sort = SORT_NAMES.get(screen.sort).copied().unwrap_or("name");
    let inner = Panel::new("Regions").info(Table::sort_info(sort)).render(buf, area);
    let th = &app.params.ui.scarcity_thresholds;
    let prey_configured = app.params.species.initial_total(crate::sim::Kind::Prey) > 0;
    let order = screen.sorted_regions(world);
    let rows: Vec<TableRow<'static>> = order.iter().map(|&ri| region_row(ri, &world.regions[ri], world, app, th, prey_configured)).collect();
    Table::new(&REGION_COLUMNS, &rows).selected(Some(screen.selected)).render(buf, inner);
}
