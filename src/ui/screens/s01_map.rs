//! S01: the live world map (variants a/b/d — default, wide, winter/night), the
//! S02a/b/c/e/f/g/h/i overlays, S01c look mode and S01e follow mode.

use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;
use crate::sim::creatures::CreatureId;
use crate::sim::disease::PathogenId;
use crate::sim::params::DayNightTint;
use crate::sim::{Season, Sim, SpeciesId, World};
use crate::ui::app::AppState;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{clock_status, EventKindStyle, SpeciesStyle};
use crate::ui::viewport::{self, GUTTER_W, MAP_CHROME_ROWS, MIN_MAP_W, SIDEBAR_W};
use crate::widgets::map::{self, MapOptions, Overlay, OverlayStack};
use crate::widgets::{panel, Component, Divider, Legend, Panel, Spacer, StatusBar, Text, Ticker, VStack};
use crate::theme;

use base::{clock_section, population_section, resources_section};
use disease_overlay::disease_tints;
use parasites::parasite_tints;

#[derive(Debug)]
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
    /// Last species shown by the species-density overlay (S02f), restored on re-entry.
    pub species_sel: SpeciesId,
}

impl WorldMap {
    pub const fn new(world_name: String) -> Self {
        Self { world_name, wide: false, overlay: Overlay::None, region_sel: 0, sense_id: None, species_sel: SpeciesId(0) }
    }

    fn overlay_name(overlay: Overlay, roster: &crate::sim::Roster) -> String {
        match overlay {
            Overlay::Vegetation => "vegetation".into(),
            Overlay::Pressure => "pressure".into(),
            Overlay::Moisture => "moisture".into(),
            Overlay::Sense(_) => "sense range".into(),
            Overlay::Region => "regions".into(),
            Overlay::Species(sp) => roster.plural(sp).to_lowercase(),
            Overlay::Health => "health".into(),
            Overlay::Disease(_) => "disease".into(),
            Overlay::Parasites => "parasites".into(),
            Overlay::None => String::new(),
        }
    }

    /// S02f: the species shown when the overlay opens — the look-cursor creature's
    /// species, else the followed creature's, else the first species with a
    /// living population, else the last species shown.
    pub(super) fn default_species(&self, app: &AppState) -> SpeciesId {
        let Some(sim) = app.sim.as_ref() else { return self.species_sel };
        if let Some((x, y)) = app.look_cursor {
            if let Some(c) = Self::creature_at_cursor(sim, x, y).and_then(|id| sim.creatures.get(id)) {
                return c.species;
            }
        }
        if let Some(c) = app.follow.and_then(|id| sim.creatures.get(id)) {
            return c.species;
        }
        let alive = |sp: SpeciesId| sim.creatures.living().any(|c| c.species == sp);
        if alive(self.species_sel) {
            return self.species_sel;
        }
        sim.roster().ids().find(|&sp| alive(sp)).unwrap_or(self.species_sel)
    }

    pub(super) fn open_species(&mut self, app: &AppState) {
        let sp = self.default_species(app);
        self.species_sel = sp;
        self.overlay = Overlay::Species(sp);
    }

    /// `Tab` / `BackTab` with the species overlay active: show the next /
    /// previous species in roster order, wrapping, extinct species included.
    /// Returns false when the overlay is not active.
    pub(super) fn cycle_species(&mut self, app: &AppState, backwards: bool) -> bool {
        let Overlay::Species(sp) = self.overlay else { return false };
        let n = app.params.species.len().max(1);
        let i = sp.index();
        let next = SpeciesId::from_index(if backwards { (i + n - 1) % n } else { (i + 1) % n });
        self.species_sel = next;
        self.overlay = Overlay::Species(next);
        true
    }

    /// `Tab` / `BackTab` with the disease overlay active (S02h): show every
    /// pathogen (`all`), then each live slot in turn, wrapping. Extinct strains
    /// are skipped. Returns false when the overlay is not active.
    pub(super) fn cycle_pathogen(&mut self, app: &AppState, backwards: bool) -> bool {
        let Overlay::Disease(cur) = self.overlay else { return false };
        let Some(sim) = app.sim.as_ref() else { return true };
        let mut stops: Vec<Option<PathogenId>> = vec![None];
        stops.extend(sim.disease.pathogens.iter().enumerate().filter(|(_, p)| !p.extinct).map(|(i, _)| Some(PathogenId(crate::cast!(i => u8)))));
        let n = stops.len();
        let i = stops.iter().position(|&s| s == cur).unwrap_or(0);
        self.overlay = Overlay::Disease(stops[if backwards { (i + n - 1) % n } else { (i + 1) % n }]);
        true
    }

    /// C7 FR9: the S12b "Show outbreak" button leaves a pathogen slot on the
    /// app; the next key or frame opens the disease overlay on it.
    const fn take_pending(&mut self, app: &mut AppState) {
        if let Some(p) = app.pending_overlay.take() {
            self.overlay = Overlay::Disease(Some(p));
        }
    }

    /// The overlay to draw this frame: `render` cannot take the pending slot,
    /// so it is honoured here until the next key consumes it.
    const fn effective_overlay(&self, app: &AppState) -> Overlay {
        match app.pending_overlay {
            Some(p) => Overlay::Disease(Some(p)),
            None => self.overlay,
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
    pub(super) fn default_sense(app: &AppState) -> Option<CreatureId> {
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
            .filter(|c| sim.roster().kind(c.species) == crate::sim::Kind::Predator)
            .max_by_key(|c| (c.kills, std::cmp::Reverse(c.id.0)))
            .map(|c| c.id)
    }

    /// `Tab` with the sense overlay active: select the next living predator
    /// (FR9). Returns false when the overlay is not active so the caller can
    /// give `Tab` its plain-map meaning.
    pub(super) fn cycle_sense(&mut self, app: &AppState) -> bool {
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
            .filter(|c| sim.roster().kind(c.species) == crate::sim::Kind::Predator)
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
        self.take_pending(app);
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

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else {
            return;
        };
        let world = &sim.world;
        let time = &sim.time;
        // If the sense-overlay selection died, revert to the plain map (FR9).
        let overlay = match self.effective_overlay(app) {
            Overlay::Sense(id) if sim.creatures.get(id).is_none_or(|c| !c.alive) => Overlay::None,
            // A slot that no longer exists falls back to every pathogen.
            Overlay::Disease(Some(p)) if sim.disease.pathogen(p).is_none() => Overlay::Disease(None),
            o => o,
        };
        let overlay_active = overlay != Overlay::None;
        let tw = area.width;
        let th = area.height;
        let map_rows = th.saturating_sub(MAP_CHROME_ROWS);
        let (map_w, side_w) = if self.wide || tw < SIDEBAR_W + MIN_MAP_W {
            (tw.saturating_sub(GUTTER_W), GUTTER_W)
        } else {
            (tw.saturating_sub(SIDEBAR_W), SIDEBAR_W)
        };

        let map_inner_w = crate::cast!(map_w.saturating_sub(2) => usize);
        let map_inner_h = crate::cast!(map_rows.saturating_sub(2) => usize);
        app.viewport_size.set((map_inner_w, map_inner_h));
        let max = viewport::max_origin(world.width(), world.height(), map_inner_w, map_inner_h);

        let (origin, title) = map_origin_title(app, sim, map_inner_w, map_inner_h, max, overlay, overlay_active, &self.world_name);

        let map_area = Rect::new(area.x, area.y, map_w, map_rows);
        let map_inner = panel::draw_with_hint(f, map_area, &title, &map_hint(world, origin, map_inner_w), panel::Kind::Outer);

        let opts = map_options(sim, app, overlay, origin, time, self.region_sel, overlay_active);
        map::render(f.buffer_mut(), map_inner, sim, &opts);

        self.draw_map_sidebar(f, area, map_w, map_rows, side_w, app, sim, world, time, overlay, overlay_active);

        draw_ticker(f, area, map_rows, sim, app);

        // Status bar.
        let status_row = area.y + area.height - 1;
        let keys = status_keys(app, self.overlay, overlay);
        let (right, right_fg) = clock_status(time, app.params.ui.day_night_tint);
        StatusBar::new(keys).right(&right).right_color(right_fg).note(app.ai.status_note()).render(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1));
    }
}

/// The key hints shown in the status bar, by current mode.
fn status_keys(app: &AppState, screen: Overlay, overlay: Overlay) -> &'static [(&'static str, &'static str)] {
    if app.follow.is_some() {
        &[("n", "next"), ("i", "inspect"), ("c", "centre"), ("Esc", "stop")]
    } else if app.look_cursor.is_some() {
        &[("↑↓←→", "move"), ("Enter", "inspect"), ("f", "follow"), ("z", "zoom"), ("Esc", "exit look")]
    } else if screen == Overlay::Region {
        &[("1-9", "overlay"), ("o", "cycle"), ("↑↓", "region"), ("Enter", "jump"), ("←→", "scroll"), ("Esc", "clear"), ("Space", "pause"), ("y", "ecology")]
    } else if let Overlay::Sense(_) = overlay {
        &[("Tab", "next predator"), ("i", "inspect"), ("f", "follow"), ("1-9", "overlay"), ("o", "cycle"), ("Esc", "clear"), ("Space", "pause")]
    } else if let Overlay::Species(_) = overlay {
        &[("Tab", "next species"), ("Shift+Tab", "previous"), ("←→↑↓", "scroll"), ("1-9", "overlay"), ("o", "cycle"), ("Esc", "clear"), ("Space", "pause")]
    } else if overlay == Overlay::Health {
        &[("←→↑↓", "scroll"), ("k", "look"), ("1-9", "overlay"), ("o", "cycle"), ("Esc", "clear"), ("Space", "pause"), ("e", "log"), ("s", "species")]
    } else if let Overlay::Disease(_) = overlay {
        &[("Tab", "next pathogen"), ("Shift+Tab", "previous"), ("←→↑↓", "scroll"), ("k", "look"), ("1-9", "overlay"), ("o", "cycle"), ("Esc", "clear"), ("Space", "pause")]
    } else if overlay == Overlay::Parasites {
        &[("←→↑↓", "scroll"), ("k", "look"), ("1-9", "overlay"), ("o", "cycle"), ("Esc", "clear"), ("Space", "pause"), ("e", "log"), ("y", "ecology")]
    } else if overlay != Overlay::None {
        &[("1-9", "overlay"), ("o", "cycle"), ("Esc", "clear"), ("Space", "pause"), ("+/-", "speed"), ("e", "log"), ("y", "ecology"), ("g", "charts")]
    } else {
        &[("k", "look"), ("Tab", "wide"), ("←→↑↓", "scroll"), ("1-9", "overlay"), ("Space", "pause"), ("+/-", "speed"), ("p", "controls"), ("?", "help"), ("q", "world")]
    }
}

/// The one-line event ticker under the map.
fn draw_ticker(f: &mut Frame<'_>, area: Rect, map_rows: u16, sim: &Sim, app: &AppState) {
    let ticker = Rect::new(area.x, area.y + map_rows, area.width, 1);
    // C4 FR11: births and mutations reach the ticker only when `log_births` is on.
    let log_births = app.params.ui.log_births;
    let last = sim.events.iter().rev().find(|e| log_births || !matches!(e.kind, crate::sim::EventKind::Birth | crate::sim::EventKind::Mutation));
    Ticker::new(last.map(|e| (e.kind.glyph(), e.kind.color(), e.text.as_str()))).render(f.buffer_mut(), ticker);
}

/// The `MapOptions` for the current frame.
#[allow(clippy::too_many_arguments)]
fn map_options(sim: &Sim, app: &AppState, overlay: Overlay, origin: (usize, usize), time: &crate::sim::Time, region_sel: usize, overlay_active: bool) -> MapOptions {
    let night = !overlay_active && time.is_night() && app.params.ui.day_night_tint == DayNightTint::Map;
    let winter = !overlay_active && time.season() == Season::Winter;
    let stack: OverlayStack = overlay.into();
    MapOptions {
            stack,
            night,
            winter,
            cursor: app.look_cursor,
            follow: app.follow,
            origin,
            creatures: true,
            selected_region: if overlay == Overlay::Region { Some(region_sel) } else { None },
            species_color: match overlay {
                Overlay::Species(sp) => sim.roster().color(sp),
                _ => theme::TEXT,
            },
            creature_tint: match overlay {
                Overlay::Disease(shown) => Some(disease_tints(sim, shown)),
                Overlay::Parasites => Some(parasite_tints(sim)),
                _ => None,
            },
    }
}

/// The viewport origin (follow-centre or clamped scroll) and the map title.
#[allow(clippy::too_many_arguments)]
fn map_origin_title(app: &AppState, sim: &Sim, map_inner_w: usize, map_inner_h: usize, max: (usize, usize), overlay: Overlay, overlay_active: bool, world_name: &str) -> ((usize, usize), String) {
        // Follow mode centres the viewport on the followed creature.
        let mut origin = (app.viewport_origin.0.min(max.0), app.viewport_origin.1.min(max.1));
        if let Some(id) = app.follow {
            if let Some(c) = sim.creatures.get(id) {
                origin = (
                    c.x.saturating_sub(map_inner_w.div_euclid(2)).min(max.0),
                    c.y.saturating_sub(map_inner_h.div_euclid(2)).min(max.1),
                );
            }
        }

        let title = if let Some(id) = app.follow {
            let name = sim.creatures.get(id).map_or("?", |c| c.name_str(sim.roster()));
            format!("{world_name} · following {name}")
        } else if app.look_cursor.is_some() {
            format!("{world_name} · look")
        } else if overlay_active {
            format!("{world_name} · overlay: {}", WorldMap::overlay_name(overlay, sim.roster()))
        } else {
            world_name.to_string()
        };
    (origin, title)
}

fn map_hint(world: &World, origin: (usize, usize), inner_w: usize) -> String {
    if inner_w < world.width() {
        format!("x {}-{} of {}   ← → scroll", origin.0, origin.0 + inner_w - 1, world.width())
    } else {
        format!("{}x{} cells", world.width(), world.height())
    }
}

/// Thousands separator for tick counters.
pub fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

impl WorldMap {
    pub(super) fn sidebar(f: &mut Frame<'_>, area: Rect, app: &AppState, sim: &Sim, world: &World, time: &crate::sim::Time) {
        let buf = f.buffer_mut();
        let inner = Panel::new("Status").render(buf, area);
        let mut rows = clock_section(app, time);
        rows.extend(population_section(sim));
        rows.extend(resources_section(app, world, time));
        rows.push(Box::new(Divider::new("Notable")));
        rows.push(Box::new(Text::new(" [k] look · [e] events · [g] charts").style(theme::dim_text())));
        rows.push(Box::new(Spacer::rows(1)));
        rows.push(Box::new(Divider::new("Legend")));
        rows.push(Box::new(Legend::map().species(sim.roster())));
        VStack::from_boxes(&rows).render(buf, inner);
    }
}

mod base;
mod input;
mod overlays;
mod look;
mod follow;
mod sense;
mod species;
mod health;
mod disease_overlay;
mod parasites;
mod regions;
#[cfg(test)]
mod tests;
