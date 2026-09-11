//! S01: the live world map (variants a/b/d — default, wide, winter/night), the
//! S02a/b/c/e overlays, S01c look mode and S01e follow mode.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::creatures::CreatureId;
use crate::sim::{Season, Sim, SpeciesId, World};
use crate::ui::screens::s06_ecology::region_status;
use crate::ui::app::AppState;
use crate::ui::screens::s03_inspector::Inspector;
use crate::ui::screens::s13_zoom::Zoom;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{EventKindStyle, SeasonStyle, SpeciesStyle};
use crate::ui::viewport::{self, GUTTER_W, MAP_CHROME_ROWS, MIN_MAP_W, SIDEBAR_W};
use crate::widgets::map::{self, MapOptions, Overlay};
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

pub struct WorldMap {
    pub world_name: String,
    /// Sidebar collapsed (S01b wide view).
    pub wide: bool,
    /// Active overlay (None = plain map, S01a).
    pub overlay: Overlay,
    /// Region highlighted under the regions overlay (S02e).
    pub region_sel: usize,
    /// Selected creature for the sense overlay (S02d).
    pub sense_id: Option<CreatureId>,
}

impl WorldMap {
    pub fn new(world_name: String) -> Self {
        WorldMap { world_name, wide: false, overlay: Overlay::None, region_sel: 0, sense_id: None }
    }

    fn overlay_name(overlay: Overlay) -> &'static str {
        match overlay {
            Overlay::Vegetation => "vegetation",
            Overlay::Pressure => "pressure",
            Overlay::Moisture => "moisture",
            Overlay::Sense(_) => "sense range",
            Overlay::Region => "regions",
            _ => "",
        }
    }

    /// The living creature on the cursor cell, else the nearest within `cheb ≤ 1`.
    pub fn creature_at_cursor(sim: &Sim, x: usize, y: usize) -> Option<CreatureId> {
        if let Some(c) = sim.creatures.living().find(|c| c.alive && c.x == x && c.y == y) {
            return Some(c.id);
        }
        sim.creatures
            .living()
            .filter(|c| c.alive && crate::sim::cheb(c.x, c.y, x, y) <= 1)
            .min_by_key(|c| (crate::sim::cheb(c.x, c.y, x, y), c.id.0))
            .map(|c| c.id)
    }

    /// FR9: the default sense-overlay subject — the look-cursor creature, else the
    /// followed creature (prey or predator), else the living predator with the
    /// most kills (ties by id).
    fn default_sense(app: &AppState) -> Option<CreatureId> {
        let sim = app.sim.as_ref()?;
        if let Some((x, y)) = app.look_cursor {
            if let Some(id) = Self::creature_at_cursor(sim, x, y) {
                if sim.creatures.get(id).is_some_and(|c| c.alive) {
                    return Some(id);
                }
            }
        }
        if let Some(id) = app.follow {
            if sim.creatures.get(id).is_some_and(|c| c.alive) {
                return Some(id);
            }
        }
        sim.creatures
            .living()
            .filter(|c| c.species.kind() == crate::sim::Kind::Predator)
            .max_by_key(|c| (c.kills, std::cmp::Reverse(c.id.0)))
            .map(|c| c.id)
    }

    /// `Tab` with the sense overlay active: select the next living predator
    /// (FR9). Returns false when the overlay is not active so the caller can
    /// give `Tab` its plain-map meaning.
    fn cycle_sense(&mut self, app: &AppState) -> bool {
        let Overlay::Sense(id) = self.overlay else { return false };
        if let Some(next) = Self::next_predator(app, id) {
            self.overlay = Overlay::Sense(next);
            self.sense_id = Some(next);
        }
        true
    }

    /// The next living predator by id ascending, wrapping (FR9).
    fn next_predator(app: &AppState, current: CreatureId) -> Option<CreatureId> {
        let sim = app.sim.as_ref()?;
        let mut ids: Vec<CreatureId> = sim
            .creatures
            .living()
            .filter(|c| c.species.kind() == crate::sim::Kind::Predator)
            .map(|c| c.id)
            .collect();
        ids.sort_unstable();
        if ids.is_empty() {
            return None;
        }
        let pos = ids.iter().position(|&x| x == current).unwrap_or(0);
        Some(ids[(pos + 1) % ids.len()])
    }
}

impl Screen for WorldMap {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        // Follow mode (S01e) takes precedence.
        if app.follow.is_some() {
            return self.handle_follow_key(key, app);
        }
        // Look mode (S01c).
        if app.look_cursor.is_some() {
            return self.handle_look_key(key, app);
        }
        self.handle_plain_key(key, app)
    }

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
        let Some(sim) = &app.sim else {
            return;
        };
        let world = &sim.world;
        let time = &sim.time;
        // If the sense-overlay selection died, revert to the plain map (FR9).
        let overlay = match self.overlay {
            Overlay::Sense(id) if sim.creatures.get(id).is_none_or(|c| !c.alive) => Overlay::None,
            o => o,
        };
        let overlay_active = overlay != Overlay::None;
        let night = !overlay_active && time.is_night() && app.params.ui.day_night_tint;
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

        // Follow mode centres the viewport on the followed creature.
        let mut origin = (app.viewport_origin.0.min(max.0), app.viewport_origin.1.min(max.1));
        if let Some(id) = app.follow {
            if let Some(c) = sim.creatures.get(id) {
                origin = (
                    c.x.saturating_sub(map_inner_w / 2).min(max.0),
                    c.y.saturating_sub(map_inner_h / 2).min(max.1),
                );
            }
        }

        let title = if let Some(id) = app.follow {
            let name = sim.creatures.get(id).map(|c| c.name_str()).unwrap_or("?");
            format!("{} · following {}", self.world_name, name)
        } else if app.look_cursor.is_some() {
            format!("{} · look", self.world_name)
        } else if overlay_active {
            format!("{} · overlay: {}", self.world_name, Self::overlay_name(overlay))
        } else {
            self.world_name.clone()
        };

        let map_area = Rect::new(area.x, area.y, map_w, map_rows);
        let map_inner = panel::draw_with_hint(f, map_area, &title, &map_hint(world, origin, map_inner_w), panel::Kind::Outer);

        let opts = MapOptions {
            overlay,
            night,
            winter,
            cursor: app.look_cursor,
            follow: app.follow,
            origin,
            creatures: true,
            fade_creatures: overlay_active && overlay != Overlay::Region && !matches!(overlay, Overlay::Sense(_)),
            selected_region: if overlay == Overlay::Region { Some(self.region_sel) } else { None },
        };
        map::render(f.buffer_mut(), map_inner, sim, &opts);

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
            if let Some(id) = app.follow {
                self.follow_sidebar(f, side, app, sim, id);
            } else if app.look_cursor.is_some() {
                self.look_sidebar(f, side, app, sim, world);
            } else if let Overlay::Sense(id) = overlay {
                self.sense_sidebar(f, side, app, sim, id);
            } else if overlay == Overlay::Region {
                self.region_sidebar(f, side, app, sim);
            } else if overlay_active {
                self.overlay_sidebar(f, side, app, world);
            } else {
                self.sidebar(f, side, app, sim, world, time);
            }
        }

        // Ticker row.
        let ticker_row = area.y + map_rows;
        let ticker = Rect::new(area.x, ticker_row, area.width, 1);
        util::fill(f.buffer_mut(), ticker, Style::default().bg(theme::BG));
        // C4 FR11: births and mutations reach the ticker only when `log_births` is on.
        let log_births = app.params.ui.log_births;
        let last = sim.events.iter().rev().find(|e| log_births || !matches!(e.kind, crate::sim::EventKind::Birth | crate::sim::EventKind::Mutation));
        if let Some(last) = last {
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
        let keys: &[(&str, &str)] = if app.follow.is_some() {
            &[("n", "next"), ("i", "inspect"), ("c", "centre"), ("Esc", "stop")]
        } else if app.look_cursor.is_some() {
            &[("↑↓←→", "move"), ("Enter", "inspect"), ("f", "follow"), ("z", "zoom"), ("Esc", "exit look")]
        } else if self.overlay == Overlay::Region {
            &[("5", "regions"), ("o", "cycle"), ("↑↓", "region"), ("Enter", "jump"), ("←→", "scroll"), ("Esc", "clear"), ("Space", "pause"), ("y", "ecology")]
        } else if let Overlay::Sense(_) = overlay {
            &[("Tab", "next predator"), ("i", "inspect"), ("f", "follow"), ("4", "sense"), ("Esc", "clear"), ("Space", "pause")]
        } else if overlay_active {
            &[("1-4 5", "overlay"), ("o", "cycle"), ("Esc", "clear"), ("Space", "pause"), ("+/-", "speed"), ("e", "log"), ("y", "ecology"), ("g", "charts")]
        } else {
            &[("k", "look"), ("Tab", "wide"), ("←→↑↓", "scroll"), ("1-3 5", "overlay"), ("Space", "pause"), ("+/-", "speed"), ("p", "controls"), ("?", "help"), ("q", "world")]
        };
        let sky = if night { glyphs::MOON } else { glyphs::SUN };
        let skyname = if night { "night" } else { "day" };
        let right = format!("{}  {} {}", time.clock_label(), sky, skyname);
        status::render(f, Rect::new(area.x, status_row, area.width, 1), keys, &right);
    }
}

impl WorldMap {
    fn region_count(&self, app: &AppState) -> usize {
        app.sim.as_ref().map(|s| s.world.regions.len()).unwrap_or(0)
    }

    // ---- plain mode (S01a/b/d + overlays) ----
    fn handle_plain_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Char('k') => {
                let (vw, vh) = app.viewport_size.get();
                app.look_cursor = Some((app.viewport_origin.0 + vw / 2, app.viewport_origin.1 + vh / 2));
                Action::None
            }
            KeyCode::Tab => {
                if !self.cycle_sense(app) {
                    self.wide = !self.wide;
                }
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
                if self.overlay == Overlay::Region {
                    let n = self.region_count(app);
                    self.region_sel = (self.region_sel + n.saturating_sub(1)) % n.max(1);
                } else {
                    app.scroll_viewport(0, -5);
                }
                Action::None
            }
            KeyCode::Down => {
                if self.overlay == Overlay::Region {
                    let n = self.region_count(app);
                    self.region_sel = (self.region_sel + 1) % n.max(1);
                } else {
                    app.scroll_viewport(0, 5);
                }
                Action::None
            }
            KeyCode::Enter if self.overlay == Overlay::Region => {
                if let Some(sim) = &app.sim {
                    if let Some(r) = sim.world.regions.get(self.region_sel) {
                        let (cx, cy) = ((r.1 + r.3) / 2, (r.2 + r.4) / 2);
                        app.centre_viewport_on(cx, cy);
                    }
                }
                Action::None
            }
            // S02d: the status bar offers [i] inspect / [f] follow for the sense subject.
            KeyCode::Char('i') if matches!(self.overlay, Overlay::Sense(_)) => {
                let Overlay::Sense(id) = self.overlay else { return Action::None };
                Action::Push(Box::new(Inspector::new(id)))
            }
            KeyCode::Char('f') if matches!(self.overlay, Overlay::Sense(_)) => {
                if let Overlay::Sense(id) = self.overlay {
                    app.follow = Some(id);
                    app.follow_death_tick = None;
                }
                Action::None
            }
            KeyCode::Char('o') => {
                self.overlay = match self.overlay {
                    Overlay::None => Overlay::Vegetation,
                    Overlay::Vegetation => Overlay::Pressure,
                    Overlay::Pressure => Overlay::Moisture,
                    Overlay::Moisture => match Self::default_sense(app) {
                        Some(id) => Overlay::Sense(id),
                        None => Overlay::Region,
                    },
                    Overlay::Sense(_) => Overlay::Region,
                    Overlay::Region => Overlay::None,
                };
                if let Overlay::Sense(id) = self.overlay {
                    self.sense_id = Some(id);
                }
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
            KeyCode::Char('4') => {
                if let Some(id) = Self::default_sense(app) {
                    self.overlay = Overlay::Sense(id);
                    self.sense_id = Some(id);
                }
                Action::None
            }
            KeyCode::Char('5') => {
                self.overlay = Overlay::Region;
                Action::None
            }
            KeyCode::Esc => {
                self.overlay = Overlay::None;
                Action::None
            }
            _ => Action::Unhandled,
        }
    }

    // ---- look mode (S01c) ----
    fn handle_look_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        let Some(sim) = &app.sim else { return Action::None };
        let step = if key.modifiers.contains(KeyModifiers::SHIFT) { 10 } else { 1 };
        match key.code {
            KeyCode::Left => {
                if let Some((x, y)) = app.look_cursor {
                    app.look_cursor = Some((x.saturating_sub(step), y));
                }
                Action::None
            }
            KeyCode::Right => {
                if let Some((x, y)) = app.look_cursor {
                    app.look_cursor = Some(((x + step).min(sim.world.width() - 1), y));
                }
                Action::None
            }
            KeyCode::Up => {
                if let Some((x, y)) = app.look_cursor {
                    app.look_cursor = Some((x, y.saturating_sub(step)));
                }
                Action::None
            }
            KeyCode::Down => {
                if let Some((x, y)) = app.look_cursor {
                    app.look_cursor = Some((x, (y + step).min(sim.world.height() - 1)));
                }
                Action::None
            }
            KeyCode::Enter => {
                if let Some((x, y)) = app.look_cursor {
                    if let Some(id) = Self::creature_at_cursor(sim, x, y) {
                        return Action::Push(Box::new(Inspector::new(id)));
                    }
                }
                Action::None
            }
            KeyCode::Char('f') => {
                if let Some((x, y)) = app.look_cursor {
                    if let Some(id) = Self::creature_at_cursor(sim, x, y) {
                        app.follow = Some(id);
                    }
                }
                Action::None
            }
            KeyCode::Char('z') => Action::Push(Box::new(Zoom::new())),
            KeyCode::Tab => {
                self.cycle_sense(app);
                Action::None
            }
            KeyCode::Char('4') => {
                if let Some(id) = Self::default_sense(app) {
                    self.overlay = Overlay::Sense(id);
                    self.sense_id = Some(id);
                }
                Action::None
            }
            KeyCode::Esc => {
                app.look_cursor = None;
                Action::None
            }
            _ => Action::Unhandled,
        }
    }

    // ---- follow mode (S01e) ----
    fn handle_follow_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Esc => {
                app.follow = None;
                app.follow_death_tick = None;
                Action::None
            }
            KeyCode::Char('i') => {
                if let Some(id) = app.follow {
                    return Action::Push(Box::new(Inspector::new(id)));
                }
                Action::None
            }
            KeyCode::Char('c') => Action::None, // centred every frame already
            KeyCode::Char('n') => {
                if let Some(sim) = &app.sim {
                    let ids = sim.creatures.living_ids();
                    if !ids.is_empty() {
                        let cur = app.follow.and_then(|id| ids.iter().position(|&x| x == id)).unwrap_or(0);
                        app.follow = Some(ids[(cur + 1) % ids.len()]);
                        app.follow_death_tick = None;
                    }
                }
                Action::None
            }
            KeyCode::Tab => {
                if !self.cycle_sense(app) {
                    self.wide = !self.wide;
                }
                Action::None
            }
            KeyCode::Char('4') => {
                if let Some(id) = Self::default_sense(app) {
                    self.overlay = Overlay::Sense(id);
                    self.sense_id = Some(id);
                }
                Action::None
            }
            _ => Action::Unhandled,
        }
    }

    /// The `Overlays` selector shared by the heatmap and region sidebars.
    fn overlays_selector(&self, f: &mut Frame, inner: Rect, mut row: u16) -> u16 {
        panel::section(f, inner, row, "Overlays");
        row += 1;
        for (key, label, active) in [
            ("1", "vegetation", self.overlay == Overlay::Vegetation),
            ("2", "pressure", self.overlay == Overlay::Pressure),
            ("3", "moisture", self.overlay == Overlay::Moisture),
            ("4", "sense", matches!(self.overlay, Overlay::Sense(_))),
            ("5", "regions", self.overlay == Overlay::Region),
        ] {
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {key} "), if active { theme::selected() } else { theme::key() }),
                Span::styled(format!("{:<12}", label), if active { theme::selected() } else { theme::text() }),
                Span::styled(if active { "►" } else { " " }, theme::text()),
            ]));
            row += 1;
        }
        row
    }

    /// S02e sidebar: the region table, the selected region and the selector.
    fn region_sidebar(&self, f: &mut Frame, area: Rect, app: &AppState, sim: &Sim) {
        let world = &sim.world;
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let mut row = 0u16;

        panel::section(f, inner, row, "Regions");
        row += 1;
        for note in [" named areas of the valley; rain and", " drought are tracked per region (y)"] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }
        row += 1;

        panel::section(f, inner, row, "By region");
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {:<2} {:<16}{:>5}{:>6}  status", "▪", "region", "veg", "moist"), theme::dim_text())));
        row += 1;
        let th = &app.params.ui.scarcity_thresholds;
        let prey_configured = app.params.creatures.initial_counts.iter().filter(|(id, _)| id.kind() == crate::sim::Kind::Prey).map(|(_, n)| *n).sum::<u32>() > 0;
        for (i, r) in world.regions.iter().enumerate() {
            let veg = crate::sim::ecology::region_land_veg_mean(world, r);
            let moist = crate::sim::ecology::region_display_moisture_mean(world, r);
            let status = region_status(veg, 0, prey_configured, th);
            let status_color = match status {
                "Scarce" => theme::BAD,
                "Strained" => theme::WARN,
                "Plenty" => theme::GOOD,
                _ => theme::TEXT,
            };
            let selected = i == self.region_sel;
            let bg = if selected { theme::SELECT_BG } else { theme::PANEL_BG };
            let text = if selected { theme::selected() } else { theme::text() };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(if selected { "►" } else { " " }, text),
                Span::styled(format!("{}{} ", glyphs::FULL_BLOCK, glyphs::FULL_BLOCK), Style::default().fg(theme::region(i)).bg(bg)),
                Span::styled(format!("{:<16}", r.0), text),
                Span::styled(format!("{:>4}%{:>5}%  ", (veg * 100.0).round() as u32, (moist * 100.0).round() as u32), text),
                Span::styled(status, Style::default().fg(status_color).bg(bg).add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() })),
            ]));
            row += 1;
        }
        row += 1;

        panel::section(f, inner, row, "Selected");
        row += 1;
        if let Some(r) = world.regions.get(self.region_sel) {
            let cells = (r.3 - r.1) * (r.4 - r.2);
            let water = (r.2..r.4).flat_map(|y| (r.1..r.3).map(move |x| (x, y))).filter(|&(x, y)| world.cell(x, y).terrain.is_water()).count();
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {}", glyphs::FULL_BLOCK), Style::default().fg(theme::region(self.region_sel)).bg(theme::PANEL_BG)),
                Span::styled(format!(" {}", r.0), theme::title()),
            ]));
            row += 1;
            util::line(f, inner, row, Line::from(Span::styled(format!(" x {}–{}  y {}–{}", r.1, r.3.saturating_sub(1), r.2, r.4.saturating_sub(1)), theme::text())));
            row += 1;
            util::line(f, inner, row, Line::from(Span::styled(format!(" {cells} cells, {water} water"), theme::text())));
            row += 1;
            let drought = sim.drought.get(self.region_sel).copied().unwrap_or(false);
            if drought {
                util::line(f, inner, row, Line::from(Span::styled(format!(" {} drought", glyphs::DROUGHT), Style::default().fg(theme::WARN).bg(theme::PANEL_BG))));
            } else {
                util::line(f, inner, row, Line::from(Span::styled(" no drought", theme::dim_text())));
            }
            row += 1;
            util::line(f, inner, row, Line::from(vec![
                Span::styled(" Enter", theme::key()),
                Span::styled(" centres the map", theme::dim_text()),
            ]));
            row += 1;
        }
        row += 1;

        row = self.overlays_selector(f, inner, row);
        row += 1;

        panel::section(f, inner, row, "Reading the map");
        row += 1;
        for note in [" tint = region, bright = selected", " labels are clipped at the edge", " Esc restores the plain map"] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }
    }

    /// S02d sidebar: the selected predator, the ring contents and the detected-prey table.
    fn sense_sidebar(&self, f: &mut Frame, area: Rect, app: &AppState, sim: &Sim, id: CreatureId) {
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
            Span::styled(format!(" {} ", if c.adult { c.species.glyph().to_ascii_uppercase() } else { c.species.glyph() }), Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{} {}", c.name_str(), c.tag()), theme::title()),
        ]));
        row += 1;
        let (sex_g, _) = match c.sex {
            crate::sim::Sex::Male => (glyphs::MALE, "male"),
            crate::sim::Sex::Female => (glyphs::FEMALE, "female"),
        };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(c.species.name(), Style::default().fg(c.species.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
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
        let r_f = r as f32;
        let mut prey = 0u32;
        let mut pred = 0u32;
        let mut tally = [0u32; 6];
        let mut dens = 0usize;
        let mut carcasses = 0usize;
        let mut water = 0usize;
        for o in sim.creatures.living() {
            if o.id == id || crate::sim::dist(c.x, c.y, o.x, o.y) > r_f {
                continue;
            }
            if o.species.kind() == crate::sim::Kind::Prey {
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
        let r_i = r as i32;
        for dy in -r_i..=r_i {
            for dx in -2 * r_i..=2 * r_i {
                let nx = c.x as i32 + dx;
                let ny = c.y as i32 + dy;
                if sim.world.in_bounds(nx, ny) && sim.world.cell(nx as usize, ny as usize).terrain.is_water() {
                    water += 1;
                }
            }
        }
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} creatures: ", prey + pred), theme::text()),
            Span::styled(format!("{prey} prey"), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
            Span::styled(format!(" {pred} predators"), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        let mut listed = Vec::new();
        for (i, id2) in crate::sim::SpeciesId::ALL.iter().enumerate() {
            if tally[i] > 0 {
                listed.push(format!("{}{} {}", id2.glyph().to_ascii_uppercase(), id2.glyph(), tally[i]));
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
        let subject_is_prey = c.species.kind() == crate::sim::Kind::Prey;
        panel::section(f, inner, row, if subject_is_prey { "Detected predators" } else { "Detected prey" });
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" tag name        dist camo status", theme::dim_text())));
        row += 1;
        let mut rows: Vec<(f32, &crate::sim::Creature, &'static str)> = Vec::new();
        for o in sim.creatures.living() {
            if o.species.kind() == c.species.kind() || crate::sim::dist(c.x, c.y, o.x, o.y) > r_f {
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
            } else if o.id == c.hunt_target.unwrap_or(crate::sim::CreatureId(u32::MAX)) {
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
                Span::styled(format!(" {} ", o.species.glyph()), Style::default().fg(o.species.color()).bg(theme::PANEL_BG)),
                Span::styled(format!("{:<5}{:<12}", o.tag(), o.name_str()), theme::text()),
                Span::styled(format!("{:>4.1} ", d), theme::text()),
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

        row = self.overlays_selector(f, inner, row);
        row += 1;

        panel::section(f, inner, row, "Reading the map");
        row += 1;
        for note in [" ° ring edge  W selected creature", " tinted cells are within sense range", " [Tab] cycles through living predators"] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }
    }

    fn sidebar(&self, f: &mut Frame, area: Rect, app: &AppState, sim: &Sim, world: &World, time: &crate::sim::Time) {
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

        // Live population (FR15).
        panel::section(f, inner, row, "Population");
        row += 1;
        let samples = sim.series.samples();
        let mut prey = 0u32;
        let mut pred = 0u32;
        for (i, id) in SpeciesId::ALL.iter().enumerate() {
            let count = sim.creatures.living().filter(|c| c.species == *id).count() as u32;
            if id.kind() == crate::sim::Kind::Prey {
                prey += count;
            } else {
                pred += count;
            }
            let arrow = trend_arrow(samples, i);
            let arrow_color = match arrow {
                glyphs::UP => theme::GOOD,
                glyphs::DOWN => theme::BAD,
                _ => theme::DIM,
            };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {} ", id.glyph().to_ascii_uppercase()), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<6}", id.name()), theme::text()),
                Span::styled(format!("{:>5} ", count), theme::text()),
                Span::styled(arrow.to_string(), Style::default().fg(arrow_color).bg(theme::PANEL_BG)),
                Span::styled("  ", theme::text()),
            ]));
            let spark = sparkline(samples, i);
            bars::sparkline(f.buffer_mut(), inner.x + 20, inner.y + row, 18, &spark, id.color());
            row += 1;
        }
        let ratio = prey as f32 / pred.max(1) as f32;
        util::line(f, inner, row, Line::from(Span::styled(format!(" prey {prey}  pred {pred}  ratio {ratio:.1}:1"), theme::dim_text())));
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
        util::line(f, inner, row, Line::from(Span::styled(" [k] look · [e] events · [g] charts", theme::dim_text())));
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

    /// S01c sidebar: the cursor cell readout.
    fn look_sidebar(&self, f: &mut Frame, area: Rect, app: &AppState, sim: &Sim, world: &World) {
        let inner = panel::draw(f, area, "Look", panel::Kind::Outer);
        let mut row = 0u16;
        let Some((cx, cy)) = app.look_cursor else { return };
        let cell = world.cell(cx, cy);
        panel::section(f, inner, row, "Cursor");
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" ({}, {})  {}", cx, cy, world.region_name(cx, cy)), theme::text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {}  elev {:.2}  veg {:.2}", cell.terrain.name(), cell.elevation, cell.vegetation), theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" Enter opens the creature inspector", theme::dim_text())));
        row += 2;
        let here = sim.creatures.living().filter(|c| c.x == cx && c.y == cy).count();
        util::line(f, inner, row, Line::from(Span::styled(format!(" creatures here: {here}"), theme::text())));
        row += 2;
        panel::section(f, inner, row, "Keys");
        row += 1;
        for (k, v) in [("Enter", "inspect"), ("f", "follow"), ("z", "zoom"), ("Esc", "exit look")] {
            util::line(f, inner, row, Line::from(vec![Span::styled(format!(" {k} "), theme::key()), Span::styled(v, theme::text())]));
            row += 1;
        }
    }

    /// S01e sidebar: the followed creature — identity, location and intent,
    /// vitals, threat/hunt line, family and its recent events (or the corpse
    /// summary once it has died).
    fn follow_sidebar(&self, f: &mut Frame, area: Rect, app: &AppState, sim: &Sim, id: CreatureId) {
        use crate::sim::creatures::{Goal, HuntPhase, RestReason};
        use crate::sim::{Kind, Sex};
        use crate::ui::screens::s03_inspector::{clip, compass, killer_and_scavengers, kin_name, local_forage};

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
            sp(format!("  {} {}", sex_g, sex_name), theme::text()),
            sp(format!("  {}", if c.adult { "adult" } else { "juvenile" }), theme::text()),
            sp(format!("  gen {}", c.generation), theme::dim_text()),
        ]));
        row += 1;
        let age = c.age_days(sim.time.day_index());
        let max_age = c.max_age_days(&sim.params.creatures);
        let age_t = age as f32 / max_age.max(1) as f32;
        let age_color = if c.alive { bars::vital_color(1.0 - age_t * 0.8, false) } else { theme::DIM };
        bars::labeled(f.buffer_mut(), inner, row, " age", age_t, age_color, 6, 16);
        f.buffer_mut().set_stringn(inner.x + 24, inner.y + row, format!("{age} / {max_age} days"), 17, theme::dim_text());
        row += 2;

        if !c.alive {
            // Corpse summary: cause, decay, meat, killer and scavengers.
            panel::section(f, inner, row, "Death");
            row += 1;
            let cause = c.death.map(|d| d.cause.label()).unwrap_or("unknown");
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
            let kg = (c.genome.size() * 120.0 * nutrition).round() as u32;
            let gone_in = (nutrition * sim.params.creatures.carcass_decay_days as f32).ceil() as u32;
            util::line(f, inner, row, Line::from(sp(format!(" {kg} kg of meat; gone in ~{gone_in} days"), theme::dim_text())));
            row += 2;
            row = killer_and_scavengers(f, inner, row, sim, c);
            row += 1;
            Self::follow_events(f, inner, row, sim, id);
            return;
        }

        // 2. Location, goal with its reason, target with distance and heading.
        panel::section(f, inner, row, "Location");
        row += 1;
        let cell = sim.world.cell(c.x, c.y);
        let (tg, tfg, _) = map::terrain_cell(cell, false);
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" ({}, {})  ", c.x, c.y), theme::text()),
            sp(sim.world.region_name(c.x, c.y).to_string(), theme::title()),
            sp(format!("  {} ", tg), Style::default().fg(tfg).bg(theme::PANEL_BG)),
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
            Goal::Graze => format!("hunger {:.2}", c.hunger),
            Goal::Rest => match c.rest_reason {
                Some(RestReason::Night) => "night".to_string(),
                Some(RestReason::Forced) => "exhausted".to_string(),
                _ => format!("energy {:.2}", c.energy),
            },
            Goal::Hunt => format!("hunger {:.2}", c.hunger),
            Goal::Scavenge => format!("hunger {:.2}", c.hunger),
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

        // 3. Vitals, then condition.
        panel::section(f, inner, row, "Vitals");
        row += 1;
        for (label, v, inv) in [("health", c.hp, false), ("hunger", c.hunger, true), ("thirst", c.thirst, true), ("energy", c.energy, false)] {
            bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", label), v, bars::vital_color(v, inv), 9, 20);
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
            match c.hunt_target.and_then(|t| sim.creatures.get(t)) {
                Some(t) => {
                    let phase = match c.hunt_phase {
                        HuntPhase::Stalk => "stalking",
                        HuntPhase::Chase => "chasing",
                        HuntPhase::Eat => "eating",
                    };
                    let d = crate::sim::cheb(c.x, c.y, t.x, t.y);
                    let seen = t.alive && crate::sim::predation::prey_detects_pred(t, c, &app.params.predation);
                    util::line(f, inner, row, Line::from(vec![
                        sp(format!(" {} ", phase), theme::text()),
                        sp(format!("{} ", if t.adult { t.species.glyph().to_ascii_uppercase() } else { t.species.glyph() }), species_st(t.species)),
                        sp(format!("{} {}", t.name_str(), t.tag()), theme::title()),
                        sp(format!("  {} cells {}", d, compass(c.x, c.y, t.x, t.y)), theme::text()),
                        sp(if !t.alive { String::new() } else if seen { "  seen".into() } else { "  unseen".into() }, Style::default().fg(if seen { theme::WARN } else { theme::GOOD }).bg(theme::PANEL_BG)),
                    ]));
                }
                None => {
                    let success = if c.attempts > 0 { c.kills as f32 / c.attempts as f32 * 100.0 } else { 0.0 };
                    util::line(f, inner, row, Line::from(vec![
                        sp(" not hunting".into(), theme::dim_text()),
                        sp(format!("   kills {}  attempts {}  {:.0}%", c.kills, c.attempts, success), theme::dim_text()),
                    ]));
                }
            }
        }
        row += 2;

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
                sp(rel.to_string(), theme::label()),
            ]));
            row += 1;
        }
        row += 1;

        // 6. Recent events fill whatever rows remain.
        Self::follow_events(f, inner, row, sim, id);
    }

    /// The followed creature's most recent events, newest first, filling the
    /// panel from `row` to its bottom edge.
    fn follow_events(f: &mut Frame, inner: Rect, mut row: u16, sim: &Sim, id: CreatureId) {
        use crate::ui::screens::s03_inspector::clip;
        if row + 1 >= inner.height {
            return;
        }
        panel::section(f, inner, row, "Recent events");
        row += 1;
        let take = (inner.height - row) as usize;
        let evs: Vec<_> = sim.events.iter().rev().filter(|e| e.subject == Some(id)).take(take).collect();
        if evs.is_empty() {
            util::line(f, inner, row, Line::from(Span::styled(" no events for this creature", theme::dim_text())));
            return;
        }
        for e in evs {
            let stamp = format!("Y{} D{:<3} {:02}h ", e.year, e.day, e.hour);
            let avail = (inner.width as usize).saturating_sub(3 + stamp.len());
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {} ", e.kind.glyph()), Style::default().fg(e.kind.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                Span::styled(stamp, theme::dim_text()),
                Span::styled(clip(&e.text, avail), theme::text()),
            ]));
            row += 1;
        }
    }

    fn overlay_sidebar(&self, f: &mut Frame, area: Rect, app: &AppState, world: &World) {
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let mut row = 0u16;

        let (name, desc1, desc2, low, high, note) = match self.overlay {
            Overlay::Vegetation => ("Vegetation density", "standing biomass per cell;", "prey graze it down, regrowth (*) restores it.", "bare", "lush", "≈ deep water  ▲ rock (not shaded)"),
            Overlay::Pressure => ("Population pressure", "traffic of prey (x0.5) and predators (x0.7)", "prey leave pressure as they move", "quiet", "crowded", "≈ deep water  ▲ rock (not shaded)"),
            Overlay::Moisture => ("Water & moisture", "soil moisture; open water is shown saturated;", "drives regrowth and thirst.", "arid", "wet", "open water counts as 100% moisture"),
            _ => return,
        };

        panel::section(f, inner, row, name);
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {}", desc1), theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {}", desc2), theme::dim_text())));
        row += 1;

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

        row = self.overlays_selector(f, inner, row);
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

/// 30-day trend arrow per the FR15 rule: > +3% ↑, < −3% ↓, else ↔.
fn trend_arrow(samples: &[crate::sim::Sample], i: usize) -> char {
    if samples.len() < 2 {
        return glyphs::FLAT;
    }
    let a = samples[samples.len().saturating_sub(30).min(samples.len() - 1)].population[i] as f32;
    let b = samples.last().unwrap().population[i] as f32;
    let pct = if a > 0.0 { (b - a) / a * 100.0 } else if b > 0.0 { f32::INFINITY } else { 0.0 };
    if pct > 3.0 {
        glyphs::UP
    } else if pct < -3.0 {
        glyphs::DOWN
    } else {
        glyphs::FLAT
    }
}

/// Last 30 days of per-species counts, for sparklines.
fn sparkline(samples: &[crate::sim::Sample], i: usize) -> Vec<u16> {
    let start = samples.len().saturating_sub(30);
    samples[start..].iter().map(|s| s.population[i].min(u16::MAX as u32) as u16).collect()
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
