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
use crate::ui::style::SeasonStyle;
use crate::widgets::map;
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

/// The S06 region status rule (FR7).
pub fn region_status(veg: f32, prey: u32, prey_configured: bool, th: &ScarcityThresholds) -> &'static str {
    let crowded = prey as f32 > th.crowded_prey_per_veg * veg.max(1e-6);
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
    pub fn new() -> Self {
        Ecology { sort: 0, selected: 1 }
    }

    fn sorted_regions(&self, world: &World) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..world.regions.len()).collect();
        match self.sort {
            1 => idx.sort_by(|&a, &b| {
                let va = crate::sim::ecology::region_land_veg_mean(world, &world.regions[a]);
                let vb = crate::sim::ecology::region_land_veg_mean(world, &world.regions[b]);
                vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal)
            }),
            2 => idx.sort_by(|&a, &b| {
                let va = crate::sim::ecology::region_display_moisture_mean(world, &world.regions[a]);
                let vb = crate::sim::ecology::region_display_moisture_mean(world, &world.regions[b]);
                vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal)
            }),
            3 => idx.sort_by_key(|&i| {
                let veg = crate::sim::ecology::region_land_veg_mean(world, &world.regions[i]);
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
                    let r = &sim.world.regions[self.selected];
                    let (cx, cy) = ((r.1 + r.3) / 2, (r.2 + r.4) / 2);
                    app.centre_viewport_on(cx, cy);
                }
                Action::Pop
            }
            KeyCode::Esc => Action::Pop,
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
        let Some(sim) = &app.sim else {
            return;
        };
        let world = &sim.world;
        let time = &sim.time;
        let ecology = &sim.params.ecology;

        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let totals_area = Rect::new(area.x, area.y, area.width * 2 / 3, body_h / 2);
        let season_area = Rect::new(area.x + totals_area.width, area.y, area.width - totals_area.width, body_h / 2);
        let regions_area = Rect::new(area.x, area.y + totals_area.height, area.width, body_h - totals_area.height);

        totals(f, totals_area, sim, world);
        season(f, season_area, sim, time, ecology);
        regions(f, regions_area, app, world, time, self);

        let sky = if time.is_night() { glyphs::MOON } else { glyphs::SUN };
        let right = format!("{}  {} {}", time.clock_label(), sky, if time.is_night() { "night" } else { "day" });
        status::render(f, Rect::new(area.x, status_row, area.width, 1), &[("r", "sort regions"), ("↑↓", "select"), ("Enter", "jump"), ("Esc", "back")], &right);
    }
}

fn totals(f: &mut Frame, area: Rect, sim: &crate::sim::Sim, world: &World) {
    let inner = panel::draw_with_hint(f, area, "Totals", "sparklines = last 240 days", panel::Kind::Outer);
    let mut row = 0u16;
    let series = sim.series.samples();
    let last = sim.series.last();

    panel::section(f, inner, row, "Now");
    row += 1;
    let veg = last.map(|s| s.veg_mean).unwrap_or(0.0);
    let water = last.map(|s| s.water_level).unwrap_or(0.0);
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
    let mut counts = [0usize; 9];
    for c in &world.cells {
        counts[c.terrain as usize] += 1;
    }
    let total = world.cells.len().max(1) as f32;
    let mut kinds: Vec<Terrain> = [Terrain::DeepWater, Terrain::ShallowWater, Terrain::Sand, Terrain::Dirt, Terrain::GrassSparse, Terrain::Grass, Terrain::GrassDense, Terrain::Forest, Terrain::Rock].to_vec();
    kinds.sort_by_key(|t| std::cmp::Reverse(counts[*t as usize]));
    let max = counts.iter().copied().max().unwrap_or(1).max(1) as f32;
    for t in kinds {
        let n = counts[t as usize];
        let (g, fg, _bg) = map::terrain_cell(&crate::sim::Cell { terrain: t, elevation: 0.0, moisture: 0.0, vegetation: 0.0, prey_pressure: 0.0, pred_pressure: 0.0, dried_from: None }, false);
        let buf = f.buffer_mut();
        let y = inner.y + row;
        buf.set_stringn(inner.x + 1, y, format!("{} ", g), 2, Style::default().fg(fg).bg(theme::PANEL_BG));
        buf.set_stringn(inner.x + 3, y, format!("{:<14}", t.name()), 14, theme::text());
        bars::bar(buf, inner.x + 17, y, 24, n as f32 / max, fg);
        buf.set_stringn(inner.x + 43, y, format!("{:>5} {:>3}%", n, (n as f32 / total * 100.0).round() as u32), 9, theme::dim_text());
        row += 1;
    }
    let _ = series;
}

fn season(f: &mut Frame, area: Rect, sim: &crate::sim::Sim, time: &crate::sim::Time, ecology: &crate::sim::params::EcologyParams) {
    let inner = panel::draw(f, area, "Season", panel::Kind::Outer);
    let mut row = 0u16;
    let season = time.season();

    util::line(f, inner, row, Line::from(vec![
        Span::styled(format!("{} {}  ", season.glyph(), season.name()), Style::default().fg(season.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        Span::styled(format!("day {} of {}  year {}", time.day_of_season(), time.season_days, time.year()), theme::text()),
    ]));
    row += 1;
    let progress = (time.day_of_season() - 1) as f32 / time.season_days as f32;
    bars::labeled(f.buffer_mut(), inner, row, " progress", progress, season.color(), 9, 30);
    row += 2;

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
            Span::styled(format!("  {:.1}x", regrowth), sty),
            Span::styled(format!("  {:.1}x", evap), sty),
            Span::styled(format!("  {:.1}x", metab), sty),
            Span::styled(format!("  {forage}"), sty),
        ]));
        row += 1;
    }
    row += 1;

    panel::section(f, inner, row, "Forecast");
    row += 1;
    let days_to_frost = (Season::Winter as i32 - season as i32).rem_euclid(4) * time.season_days as i32;
    let frost = if season == Season::Winter { " frost now".to_string() } else { format!(" frost in {} days", days_to_frost) };
    util::line(f, inner, row, Line::from(Span::styled(frost, theme::text())));
    row += 1;
    util::line(f, inner, row, Line::from(Span::styled(" forage line 0.25 — regions below it start migrations".to_string(), theme::dim_text())));
    row += 1;
    if let Some(e) = sim.events.iter().rev().find(|e| e.kind == crate::sim::EventKind::Drought || e.kind == crate::sim::EventKind::DroughtEased) {
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} ", glyphs::DROUGHT), Style::default().fg(theme::WARN).bg(theme::PANEL_BG)),
            Span::styled(e.text.clone(), theme::text()),
        ]));
        row += 1;
    }

    panel::section(f, inner, row, "Day");
    row += 1;
    // 24-cell hour strip.
    for h in 0..24u32 {
        let night = h < time.sunrise_hour || h >= time.sunset_hour;
        let ch = if h == time.hour() { glyphs::FULL_BLOCK } else if night { glyphs::SHADE_1 } else { glyphs::SHADE_3 };
        let color = if night { theme::DEEP_WATER_FG } else { theme::ACCENT };
        if let Some(c) = f.buffer_mut().cell_mut((inner.x + 1 + h as u16, inner.y + row)) {
            c.set_char(ch);
            c.set_style(Style::default().fg(if h == time.hour() { theme::TEXT_BRIGHT } else { color }).bg(theme::PANEL_BG));
        }
    }
    row += 1;
    let sky = if time.is_night() { glyphs::MOON } else { glyphs::SUN };
    util::line(f, inner, row, Line::from(Span::styled(format!(" now {} {} {}", time.hour_label(), sky, if time.is_night() { "night" } else { "day" }), theme::dim_text())));
}

fn regions(f: &mut Frame, area: Rect, app: &AppState, world: &World, time: &crate::sim::Time, screen: &Ecology) {
    let inner = panel::draw_with_hint(f, area, "Regions", "sorted by name   [r] cycle sort", panel::Kind::Outer);
    let mut row = 0u16;
    let th = &app.params.ui.scarcity_thresholds;
    let prey_configured = app.params.creatures.initial_counts.iter().filter(|(id, _)| id.kind() == crate::sim::Kind::Prey).map(|(_, n)| *n).sum::<u32>() > 0;

    util::line(f, inner, row, Line::from(Span::styled(
        format!("{:<17}{:>6}{:>7}   {:<26}{:<26}{:>5}{:>5}{:>10}   {}", " region", "cells", "water", " vegetation", " moisture", "prey", "pred", "pressure", "status"),
        theme::dim_text(),
    )));
    row += 1;

    let order = screen.sorted_regions(world);
    for (i, &ri) in order.iter().enumerate() {
        let r = &world.regions[ri];
        let veg = crate::sim::ecology::region_land_veg_mean(world, r);
        let moist = crate::sim::ecology::region_display_moisture_mean(world, r);
        let cells = (r.3 - r.1) * (r.4 - r.2);
        let water = world.cells.iter().enumerate().filter(|(idx, c)| {
            let (x, y) = (idx % world.width, idx / world.width);
            x >= r.1 && x < r.3 && y >= r.2 && y < r.4 && c.terrain.is_water()
        }).count();
        let status = region_status(veg, 0, prey_configured, th);
        let (label, color) = match status {
            "Scarce" => ("Scarce", theme::BAD),
            "Strained" => ("Strained", theme::WARN),
            "Plenty" => ("Plenty", theme::GOOD),
            _ => ("Stable", theme::TEXT),
        };
        let selected = i == screen.selected;
        let y = inner.y + row;
        let buf = f.buffer_mut();
        if selected {
            for x in inner.x..inner.right() {
                if let Some(c) = buf.cell_mut((x, y)) {
                    c.set_bg(theme::SELECT_BG);
                }
            }
        }
        let text = if selected { theme::TEXT_BRIGHT } else { theme::TEXT };
        buf.set_stringn(inner.x, y, format!("{}{:<16}", if selected { glyphs::PLAY } else { ' ' }, r.0), 18, Style::default().fg(text).bg(if selected { theme::SELECT_BG } else { theme::PANEL_BG }));
        buf.set_stringn(inner.x + 18, y, format!("{:>6}", cells), 6, Style::default().fg(text).bg(if selected { theme::SELECT_BG } else { theme::PANEL_BG }));
        buf.set_stringn(inner.x + 24, y, format!("{:>6.0}%", water as f32 / cells.max(1) as f32 * 100.0), 7, Style::default().fg(theme::SHALLOW_FG).bg(if selected { theme::SELECT_BG } else { theme::PANEL_BG }));
        let mut x = inner.x + 32;
        for (v, ramp) in [(veg, theme::veg(veg)), (moist, theme::water(moist))] {
            bars::bar(buf, x, y, 20, v, ramp);
            buf.set_stringn(x + 21, y, format!("{:.2}", v), 4, Style::default().fg(text).bg(if selected { theme::SELECT_BG } else { theme::PANEL_BG }));
            x += 26;
        }
        buf.set_stringn(inner.x + 86, y, format!("{:>5}", 0), 5, Style::default().fg(theme::HARE).bg(if selected { theme::SELECT_BG } else { theme::PANEL_BG }));
        buf.set_stringn(inner.x + 91, y, format!("{:>5}", 0), 5, Style::default().fg(theme::WOLF).bg(if selected { theme::SELECT_BG } else { theme::PANEL_BG }));
        buf.set_stringn(inner.x + 98, y, format!("{:>4.2} ", 0.0), 5, Style::default().fg(text).bg(if selected { theme::SELECT_BG } else { theme::PANEL_BG }));
        buf.set_stringn(inner.x + 104, y, format!("{:<9}", label), 9, Style::default().fg(color).bg(if selected { theme::SELECT_BG } else { theme::PANEL_BG }).add_modifier(Modifier::BOLD));
        row += 1;
    }
    let _ = time;
}
