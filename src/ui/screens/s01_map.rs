//! S01: the live world map (variants a/b/d — default, wide, winter/night) and
//! the S02a/b/c overlays, which are a *state* of the map.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::{Season, SpeciesId, World};
use crate::ui::app::AppState;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{EventKindStyle, SeasonStyle, SpeciesStyle};
use crate::ui::viewport::{self, GUTTER_W, MAP_CHROME_ROWS, MIN_MAP_W, SIDEBAR_W};
use crate::widgets::map::{self, MapData, MapOptions, Overlay};
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

pub struct WorldMap {
    pub world_name: String,
    /// Sidebar collapsed (S01b wide view).
    pub wide: bool,
    /// Active overlay (None = plain map, S01a).
    pub overlay: Overlay,
}

impl WorldMap {
    pub fn new(world_name: String) -> Self {
        WorldMap { world_name, wide: false, overlay: Overlay::None }
    }

    fn overlay_name(overlay: Overlay) -> &'static str {
        match overlay {
            Overlay::Vegetation => "vegetation",
            Overlay::Pressure => "pressure",
            Overlay::Moisture => "moisture",
            _ => "",
        }
    }
}

impl Screen for WorldMap {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Tab => {
                self.wide = !self.wide;
                Action::None
            }
            KeyCode::Left => {
                app.scroll_viewport(-5, 0);
                Action::None
            }
            KeyCode::Right => {
                app.scroll_viewport(5, 0);
                Action::None
            }
            KeyCode::Up => {
                app.scroll_viewport(0, -5);
                Action::None
            }
            KeyCode::Down => {
                app.scroll_viewport(0, 5);
                Action::None
            }
            KeyCode::Char('o') => {
                self.overlay = match self.overlay {
                    Overlay::None => Overlay::Vegetation,
                    Overlay::Vegetation => Overlay::Pressure,
                    Overlay::Pressure => Overlay::Moisture,
                    Overlay::Moisture => Overlay::None,
                    _ => Overlay::None,
                };
                Action::None
            }
            KeyCode::Char('1') => {
                self.overlay = Overlay::Vegetation;
                Action::None
            }
            KeyCode::Char('2') => {
                self.overlay = Overlay::Pressure;
                Action::None
            }
            KeyCode::Char('3') => {
                self.overlay = Overlay::Moisture;
                Action::None
            }
            KeyCode::Char('4') => Action::None, // sense overlay arrives in C5
            KeyCode::Esc => {
                self.overlay = Overlay::None;
                Action::None
            }
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
        let Some(sim) = &app.sim else {
            return;
        };
        let world = &sim.world;
        let time = &sim.time;
        let overlay_active = self.overlay != Overlay::None;
        // Heatmaps ignore the night and winter tints (FR6).
        let night = !overlay_active && time.is_night();
        let winter = !overlay_active && time.season() == Season::Winter;

        let tw = area.width;
        let th = area.height;
        let map_rows = th.saturating_sub(MAP_CHROME_ROWS);
        let (map_w, side_w) = if self.wide || tw < SIDEBAR_W + MIN_MAP_W {
            (tw.saturating_sub(GUTTER_W), GUTTER_W)
        } else {
            (tw.saturating_sub(SIDEBAR_W), SIDEBAR_W)
        };

        let map_inner_w = map_w.saturating_sub(2) as usize;
        let map_inner_h = map_rows.saturating_sub(2) as usize;
        app.viewport_size.set((map_inner_w, map_inner_h));
        let max = viewport::max_origin(world.width(), world.height(), map_inner_w, map_inner_h);
        let origin = (app.viewport_origin.0.min(max.0), app.viewport_origin.1.min(max.1));

        let title = if overlay_active {
            format!("{} · overlay: {}", self.world_name, Self::overlay_name(self.overlay))
        } else {
            self.world_name.clone()
        };

        let map_area = Rect::new(area.x, area.y, map_w, map_rows);
        let map_inner = panel::draw_with_hint(f, map_area, &title, &map_hint(world, origin, map_inner_w), panel::Kind::Outer);

        let opts = MapOptions {
            overlay: self.overlay,
            night,
            winter,
            cursor: None,
            follow: None,
            origin,
            creatures: true,
            fade_creatures: overlay_active,
        };
        let empty: &[map::MapCreature] = &[];
        let data = MapData { world, creatures: empty, selected: None };
        map::render(f.buffer_mut(), map_inner, &data, &opts);

        if side_w == GUTTER_W {
            let gutter = Rect::new(area.x + map_w, area.y, side_w, map_rows);
            let inner = panel::draw(f, gutter, "", panel::Kind::Outer);
            for (i, ch) in "«SIDEBAR»".chars().enumerate() {
                let y = inner.y + 1 + i as u16;
                if y < inner.bottom() {
                    f.buffer_mut().set_stringn(inner.x, y, ch.to_string(), 1, theme::key());
                }
            }
        } else {
            let side = Rect::new(area.x + map_w, area.y, side_w, map_rows);
            if overlay_active {
                self.overlay_sidebar(f, side, app, world);
            } else {
                self.sidebar(f, side, app, world, time);
            }
        }

        // Ticker row.
        let ticker_row = area.y + map_rows;
        let ticker = Rect::new(area.x, ticker_row, area.width, 1);
        util::fill(f.buffer_mut(), ticker, Style::default().bg(theme::BG));
        if let Some(last) = sim.events.last() {
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
        }

        // Status bar.
        let status_row = area.y + area.height - 1;
        let keys: &[(&str, &str)] = if overlay_active {
            &[("1-3", "overlay"), ("o", "cycle"), ("Esc", "clear"), ("Space", "pause"), ("+/-", "speed"), ("e", "log"), ("y", "ecology"), ("g", "charts")]
        } else {
            &[("Tab", "wide"), ("←→↑↓", "scroll"), ("1-3", "overlay"), ("Space", "pause"), ("+/-", "speed"), ("p", "controls"), ("?", "help"), ("q", "world")]
        };
        let sky = if night { glyphs::MOON } else { glyphs::SUN };
        let skyname = if night { "night" } else { "day" };
        let right = format!("{}  {} {}", time.clock_label(), sky, skyname);
        status::render(f, Rect::new(area.x, status_row, area.width, 1), keys, &right);
    }
}

impl WorldMap {
    fn sidebar(&self, f: &mut Frame, area: Rect, app: &AppState, world: &World, time: &crate::sim::Time) {
        let inner = panel::draw(f, area, "Status", panel::Kind::Outer);
        let mut row = 0u16;
        let night = time.is_night();
        let winter = time.season() == Season::Winter;

        panel::section(f, inner, row, "Clock");
        row += 1;
        let season = time.season();
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" Year {:<3} Day {:<3}", time.year(), time.day_of_season()), theme::text()),
            Span::styled(format!("   {} {}", season.glyph(), season.name()), Style::default().fg(season.color()).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        let sky = if night { (glyphs::MOON, "night") } else { (glyphs::SUN, "day") };
        let (state_glyph, state_color) = if app.paused { (glyphs::PAUSE_STR, theme::WARN) } else { (glyphs::FAST_STR, theme::GOOD) };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {:02}:00  {} {}", time.hour(), sky.0, sky.1), theme::text()),
            Span::styled(format!("      {} x{}  {}", state_glyph, app.speed(), if app.paused { "paused" } else { "running" }), Style::default().fg(state_color).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" tick {}", group(time.tick)), theme::dim_text())));
        row += 2;

        panel::section(f, inner, row, "Population");
        row += 1;
        for id in SpeciesId::ALL {
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {} ", id.glyph().to_ascii_uppercase()), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<6}", id.name()), theme::text()),
                Span::styled(format!("{:>5} ", "—"), theme::text()),
            ]));
            bars::sparkline(f.buffer_mut(), inner.x + 20, inner.y + row, 18, &[], id.color());
            row += 1;
        }
        util::line(f, inner, row, Line::from(Span::styled(" prey —  pred —  ratio —:1", theme::dim_text())));
        row += 2;

        panel::section(f, inner, row, "Resources");
        row += 1;
        let veg = mean_veg_land(world);
        let water_cells = world.cells.iter().filter(|c| c.terrain.is_water()).count();
        let water_level = if world.water_cells_at_generation > 0 {
            water_cells as f32 / world.water_cells_at_generation as f32
        } else {
            0.0
        };
        bars::labeled(f.buffer_mut(), inner, row, " vegetation", veg, theme::VEGETATION, 12, 20);
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " water", water_level, theme::SHALLOW_FG, 12, 20);
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(
            format!(" carcasses {:<3} dens {:<3} regrowth {}", world.carcasses.len(), world.dens.len(), world.seeds.len()),
            theme::text(),
        )));
        row += 1;
        let strained = app.params.ui.scarcity_thresholds.strained;
        let below = world.regions.iter().filter(|r| crate::sim::ecology::region_land_veg_mean(world, r) < strained).count();
        if winter || below > 0 {
            util::line(f, inner, row, Line::from(Span::styled(
                format!(" {} scarcity: {} regions below forage line", glyphs::ALERT, below),
                Style::default().fg(theme::WARN).bg(theme::PANEL_BG),
            )));
            row += 1;
        }
        row += 1;

        panel::section(f, inner, row, "Notable");
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" no creatures yet", theme::dim_text())));
        row += 2;

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

    fn overlay_sidebar(&self, f: &mut Frame, area: Rect, app: &AppState, world: &World) {
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let mut row = 0u16;

        let (name, desc1, desc2, low, high, note) = match self.overlay {
            Overlay::Vegetation => ("Vegetation density", "standing biomass per cell;", "prey graze it down, regrowth (*) restores it.", "bare", "lush", "≈ deep water  ▲ rock (not shaded)"),
            Overlay::Pressure => ("Population pressure", "traffic of prey (x0.5) and predators (x0.7)", "no creatures yet", "quiet", "crowded", "≈ deep water  ▲ rock (not shaded)"),
            Overlay::Moisture => ("Water & moisture", "soil moisture; open water is shown saturated;", "drives regrowth and thirst.", "arid", "wet", "open water counts as 100% moisture"),
            _ => return,
        };

        panel::section(f, inner, row, name);
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {}", desc1), theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {}", desc2), theme::dim_text())));
        row += 1;

        // Legend ramp: a 24-cell sample of the map's shade glyphs + colours.
        panel::section(f, inner, row, "Legend");
        row += 1;
        let ramp = |t: f32| -> Color {
            match self.overlay {
                Overlay::Vegetation => theme::veg(t),
                Overlay::Pressure => theme::heat(t),
                Overlay::Moisture => theme::water(t),
                _ => theme::DIM,
            }
        };
        for i in 0..24 {
            let t = (i as f32 + 0.5) / 24.0;
            let g = glyphs::shade(t);
            if let Some(c) = f.buffer_mut().cell_mut((inner.x + 4 + i, inner.y + row)) {
                c.set_char(g);
                c.set_style(Style::default().fg(ramp(t)).bg(theme::dim(ramp(t), 0.75)));
            }
        }
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled("  0%      25%      50%      75%      100%", theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {low} … {high}"), theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {note}"), theme::dim_text())));
        row += 2;

        panel::section(f, inner, row, "By region");
        row += 1;
        for r in &world.regions {
            let mean = match self.overlay {
                Overlay::Vegetation => crate::sim::ecology::region_land_veg_mean(world, r),
                Overlay::Pressure => 0.0,
                Overlay::Moisture => crate::sim::ecology::region_display_moisture_mean(world, r),
                _ => 0.0,
            };
            bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", r.0), mean, ramp(0.8), 18, 14);
            row += 1;
        }
        row += 1;

        panel::section(f, inner, row, "Overlays");
        row += 1;
        for (key, label, active) in [
            ("1", "vegetation", self.overlay == Overlay::Vegetation),
            ("2", "pressure", self.overlay == Overlay::Pressure),
            ("3", "moisture", self.overlay == Overlay::Moisture),
            ("4", "sense", false),
        ] {
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {key} "), if active { theme::selected() } else { theme::key() }),
                Span::styled(format!("{:<12}", label), if active { theme::selected() } else { theme::text() }),
                Span::styled(if active { "►" } else { " " }, theme::text()),
            ]));
            row += 1;
        }
        row += 1;

        panel::section(f, inner, row, "Reading the map");
        row += 1;
        for note in [" creatures & resources faded", " Esc restores the plain map", " ░ <25%  ▒ <50%  ▓ <75%  █ ≥75%"] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }
        let _ = app;
    }
}

fn map_hint(world: &World, origin: (usize, usize), inner_w: usize) -> String {
    if inner_w < world.width() {
        format!("x {}-{} of {}   ← → scroll", origin.0, origin.0 + inner_w - 1, world.width())
    } else {
        format!("{}x{} cells", world.width(), world.height())
    }
}

fn mean_veg_land(world: &World) -> f32 {
    let mut sum = 0.0f32;
    let mut n = 0usize;
    for cell in &world.cells {
        if !cell.terrain.is_water() {
            sum += cell.vegetation;
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        sum / n as f32
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
