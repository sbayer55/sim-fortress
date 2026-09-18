//! S01: the live world map (variants a/b/d — default, wide, winter/night), the
//! S02 overlays composed through the S14 stack on `AppState`, S01c look mode
//! and S01e follow mode.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use crate::sim::creatures::CreatureId;
use crate::sim::disease::PathogenId;
use crate::sim::params::DayNightTint;
use crate::sim::{Season, Sim, SpeciesId, World};
use crate::ui::app::AppState;
use crate::ui::screens::s14_switcher::OverlaySwitcher;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{clock_status, EventKindStyle, SpeciesStyle};
use crate::ui::viewport::{self, GUTTER_W, MAP_CHROME_ROWS, MIN_MAP_W, SIDEBAR_W};
use crate::widgets::map::{self, Base, Disease, MapOptions, OverlayStack};
use crate::widgets::{panel, Component, Divider, Legend, Panel, Spacer, StatusBar, Text, Ticker, VStack};
use crate::theme;

use base::{clock_section, population_section, resources_section};
pub use stack_sidebar::{layer_legend, stack_section, sub_pick_text};
use disease_overlay::disease_tints;
use parasites::parasite_tints;

#[derive(Debug)]
pub struct WorldMap {
    pub world_name: String,
    /// Sidebar collapsed (S01b wide view).
    pub wide: bool,
    /// Region highlighted under the Regions mark (S02e).
    pub region_sel: usize,
}

impl WorldMap {
    pub const fn new(world_name: String) -> Self {
        Self { world_name, wide: false, region_sel: 0 }
    }

    /// S02f: the species the Species base shows when it comes on — the
    /// look-cursor creature's species, else the followed creature's, else the
    /// remembered species while it lives, else the first species with a
    /// living population, else the remembered species.
    pub fn default_species(app: &AppState) -> SpeciesId {
        let remembered = app.overlay.species;
        let Some(sim) = app.sim.as_ref() else { return remembered };
        if let Some((x, y)) = app.look_cursor {
            if let Some(c) = Self::creature_at_cursor(sim, x, y).and_then(|id| sim.creatures.get(id)) {
                return c.species;
            }
        }
        if let Some(c) = app.follow.and_then(|id| sim.creatures.get(id)) {
            return c.species;
        }
        let alive = |sp: SpeciesId| sim.creatures.living().any(|c| c.species == sp);
        if alive(remembered) {
            return remembered;
        }
        sim.roster().ids().find(|&sp| alive(sp)).unwrap_or(remembered)
    }

    /// `Tab` / `BackTab` with the Species base on: show the next / previous
    /// species in roster order, wrapping, extinct species included. Returns
    /// false when the base is not Species.
    pub(super) fn cycle_species(app: &mut AppState, backwards: bool) -> bool {
        if app.overlay.base != Base::Species {
            return false;
        }
        let n = app.params.species.len().max(1);
        let i = app.overlay.species.index();
        app.overlay.species = SpeciesId::from_index(if backwards { (i + n - 1) % n } else { (i + 1) % n });
        true
    }

    /// `Tab` / `BackTab` with the Disease mark on (S02h): every pathogen,
    /// then each live slot in turn, wrapping. Returns false when the mark is off.
    pub(super) fn cycle_pathogen(app: &mut AppState, backwards: bool) -> bool {
        let Disease::On(cur) = app.overlay.disease else { return false };
        let Some(sim) = app.sim.as_ref() else { return true };
        let stops = pathogen_stops(sim);
        let n = stops.len();
        let i = stops.iter().position(|&s| s == cur).unwrap_or(0);
        app.overlay.show_disease(stops[if backwards { (i + n - 1) % n } else { (i + 1) % n }]);
        true
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

    /// FR9: the default sense subject — the look-cursor creature, else the
    /// followed creature (prey or predator), else the living predator with the
    /// most kills (ties by id).
    pub fn default_sense(app: &AppState) -> Option<CreatureId> {
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

    /// Turn the Sense mark on, replacing a dead or absent subject by the S02d
    /// default rule (S14 edge cases). Returns false, leaving the mark off,
    /// when no predator lives.
    pub fn turn_sense_on(app: &mut AppState) -> bool {
        if !subject_alive(app, app.overlay.sense_subject) {
            app.overlay.sense_subject = Self::default_sense(app);
        }
        app.overlay.sense = app.overlay.sense_subject.is_some();
        app.overlay.sense
    }

    /// `Tab` with the Sense mark on: select the next living predator (FR9).
    /// Returns false when the mark is off so the caller can give `Tab` its
    /// plain-map meaning.
    pub(super) fn cycle_sense(app: &mut AppState) -> bool {
        if !app.overlay.sense {
            return false;
        }
        if let Some(next) = app.overlay.sense_subject.and_then(|id| Self::next_predator(app, id)) {
            app.overlay.sense_subject = Some(next);
        }
        true
    }

    /// The next living predator by id ascending, wrapping (FR9).
    fn next_predator(app: &AppState, current: CreatureId) -> Option<CreatureId> {
        let ids = living_predators(app.sim.as_ref()?);
        if ids.is_empty() {
            return None;
        }
        let pos = ids.iter().position(|&x| x == current).unwrap_or(0);
        Some(ids[(pos + 1) % ids.len()])
    }
}

/// Every living predator, id ascending.
pub fn living_predators(sim: &Sim) -> Vec<CreatureId> {
    let mut ids: Vec<CreatureId> = sim.creatures.living().filter(|c| sim.roster().kind(c.species) == crate::sim::Kind::Predator).map(|c| c.id).collect();
    ids.sort_unstable();
    ids
}

/// Whether `id` names a living creature.
pub fn subject_alive(app: &AppState, id: Option<CreatureId>) -> bool {
    id.is_some_and(|id| app.sim.as_ref().and_then(|sim| sim.creatures.get(id)).is_some_and(|c| c.alive))
}

/// The Disease sub-pick stops (S02h): every pathogen (`None`), then each
/// non-extinct slot in slot order.
pub fn pathogen_stops(sim: &Sim) -> Vec<Option<PathogenId>> {
    let mut stops: Vec<Option<PathogenId>> = vec![None];
    stops.extend(sim.disease.pathogens.iter().enumerate().filter(|(_, p)| !p.extinct).map(|(i, _)| Some(PathogenId(crate::cast!(i => u8)))));
    stops
}

/// The stack as it is drawn this frame: a dead sense subject turns the mark
/// off and clears the subject, and a vanished pathogen slot falls back to
/// every pathogen (S14 edge cases).
pub fn live_stack(sim: &Sim, stack: OverlayStack) -> OverlayStack {
    let mut s = stack;
    if s.sense && !s.sense_subject.is_some_and(|id| sim.creatures.get(id).is_some_and(|c| c.alive)) {
        s.sense = false;
        s.sense_subject = None;
    }
    if let Disease::On(Some(p)) = s.disease {
        if sim.disease.pathogen(p).is_none() {
            s.show_disease(None);
        }
    }
    s
}

impl Screen for WorldMap {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        if let Some(sim) = app.sim.as_ref() {
            app.overlay = live_stack(sim, app.overlay);
        }
        // `o` opens the switcher from every mode (S14).
        if key.code == KeyCode::Char('o') {
            return Action::Push(Box::new(OverlaySwitcher::open(app)));
        }
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
        let stack = live_stack(sim, app.overlay);
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

        let hint = map_hint(world, app, max, map_inner_w, map_inner_h);
        let (origin, title) = map_origin_title(app, sim, max, &stack, &self.world_name, map_w, &hint.1);

        let map_area = Rect::new(area.x, area.y, map_w, map_rows);
        let map_inner = panel::draw_with_hint(f, map_area, &title, &hint.1, panel::Kind::Outer);

        let opts = map_options(sim, app, &stack, hint.0.unwrap_or(origin), time, self.region_sel);
        map::render(f.buffer_mut(), map_inner, sim, &opts);

        self.draw_map_sidebar(f, area, map_w, map_rows, side_w, app, sim, world, time, &stack);

        draw_ticker(f, area, map_rows, sim, app);

        // Status bar.
        let status_row = area.y + area.height - 1;
        let keys = status_keys(app, &stack);
        let (right, right_fg) = clock_status(time, app.params.ui.day_night_tint);
        StatusBar::new(keys).right(&right).right_color(right_fg).note(app.ai.status_note()).render(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1));
    }
}

/// The key hints shown in the status bar, by current mode and stack (S14 item 21).
fn status_keys(app: &AppState, stack: &OverlayStack) -> &'static [(&'static str, &'static str)] {
    if app.follow.is_some() {
        &[("n", "next"), ("i", "inspect"), ("c", "centre"), ("Esc", "stop")]
    } else if app.look_cursor.is_some() {
        &[("↑↓←→", "move"), ("Enter", "inspect"), ("f", "follow"), ("z", "zoom"), ("Esc", "exit look")]
    } else if stack.sense {
        &[("Tab", "next predator"), ("i", "inspect"), ("f", "follow"), ("o", "overlay"), ("Esc", "clear"), ("Space", "pause")]
    } else if stack.base == Base::Species {
        &[("Tab", "next species"), ("Shift+Tab", "previous"), ("←→↑↓", "scroll"), ("o", "overlay"), ("Esc", "clear"), ("Space", "pause")]
    } else if stack.disease.is_on() {
        &[("Tab", "next pathogen"), ("Shift+Tab", "previous"), ("←→↑↓", "scroll"), ("k", "look"), ("o", "overlay"), ("Esc", "clear"), ("Space", "pause")]
    } else if stack.regions {
        &[("o", "overlay"), ("↑↓", "region"), ("Enter", "jump"), ("←→", "scroll"), ("Esc", "clear"), ("Space", "pause"), ("y", "ecology")]
    } else if stack.health {
        &[("←→↑↓", "scroll"), ("k", "look"), ("o", "overlay"), ("Esc", "clear"), ("Space", "pause"), ("e", "log"), ("s", "species")]
    } else if stack.base == Base::Parasites {
        &[("←→↑↓", "scroll"), ("k", "look"), ("o", "overlay"), ("Esc", "clear"), ("Space", "pause"), ("e", "log"), ("y", "ecology")]
    } else if !stack.is_empty() {
        &[("o", "overlay"), ("Esc", "clear"), ("Space", "pause"), ("+/-", "speed"), ("e", "log"), ("y", "ecology"), ("g", "charts")]
    } else {
        &[("k", "look"), ("Tab", "wide"), ("←→↑↓", "scroll"), ("o", "overlay"), ("Space", "pause"), ("+/-", "speed"), ("p", "controls"), ("?", "help"), ("q", "world")]
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
fn map_options(sim: &Sim, app: &AppState, stack: &OverlayStack, origin: (usize, usize), time: &crate::sim::Time, region_sel: usize) -> MapOptions {
    let active = !stack.is_empty();
    let night = !active && time.is_night() && app.params.ui.day_night_tint == DayNightTint::Map;
    let winter = !active && time.season() == Season::Winter;
    MapOptions {
        stack: *stack,
        night,
        winter,
        cursor: app.look_cursor,
        follow: app.follow,
        origin,
        creatures: true,
        selected_region: stack.regions.then_some(region_sel),
        species_color: if stack.base == Base::Species { sim.roster().color(stack.species) } else { theme::TEXT },
        // Disease first when both want a creature's colour (S14 item 17).
        creature_tint: match stack.disease {
            Disease::On(shown) => Some(disease_tints(sim, shown)),
            Disease::Off if stack.base == Base::Parasites => Some(parasite_tints(sim)),
            Disease::Off => None,
        },
    }
}

/// The viewport origin (follow-centre or clamped scroll) and the map title,
/// cut with `…` so the scroll hint survives in the top border (S14 item 19).
#[allow(clippy::too_many_arguments)]
fn map_origin_title(app: &AppState, sim: &Sim, max: (usize, usize), stack: &OverlayStack, world_name: &str, map_w: u16, hint: &str) -> ((usize, usize), String) {
    let origin = (app.viewport_origin.0.min(max.0), app.viewport_origin.1.min(max.1));
    let title = if let Some(id) = app.follow {
        let name = sim.creatures.get(id).map_or("?", |c| c.name_str(sim.roster()));
        format!("{world_name} · following {name}")
    } else if app.look_cursor.is_some() {
        format!("{world_name} · look")
    } else if stack.is_empty() {
        world_name.to_string()
    } else {
        format!("{world_name} · overlay: {}", stack.title(sim.roster(), &sim.disease))
    };
    (origin, cut_title(&title, map_w, hint))
}

/// Cut `title` so ` title ` and ` hint ` both fit between the corners. The
/// cut mark is the CP437 `·` of `common::clip`, not `…`, which CP437 lacks.
fn cut_title(title: &str, map_w: u16, hint: &str) -> String {
    let room = usize::from(map_w).saturating_sub(2 + 2 + 2 + hint.chars().count());
    crate::ui::screens::common::clip(title, room)
}

/// The follow-centred origin, if following, and the scroll hint text.
fn map_hint(world: &World, app: &AppState, max: (usize, usize), inner_w: usize, inner_h: usize) -> (Option<(usize, usize)>, String) {
    // Follow mode centres the viewport on the followed creature.
    let follow_origin = app.follow.and_then(|id| app.sim.as_ref()?.creatures.get(id)).map(|c| (c.x.saturating_sub(inner_w.div_euclid(2)).min(max.0), c.y.saturating_sub(inner_h.div_euclid(2)).min(max.1)));
    let origin = follow_origin.unwrap_or_else(|| (app.viewport_origin.0.min(max.0), app.viewport_origin.1.min(max.1)));
    let hint = if inner_w < world.width() {
        format!("x {}-{} of {}   ← → scroll", origin.0, origin.0 + inner_w - 1, world.width())
    } else {
        format!("{}x{} cells", world.width(), world.height())
    };
    (follow_origin, hint)
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
        rows.push(Box::new(Text::new(" [s] species · [t] traits & fates").style(theme::dim_text())));
        rows.push(Box::new(Spacer::rows(1)));
        rows.push(Box::new(Divider::new("Overlay")));
        rows.push(Box::new(Text::new(" press o to open the switcher").style(theme::dim_text())));
        rows.push(Box::new(Text::new(" Tab flips between base and marks").style(theme::dim_text())));
        rows.push(Box::new(Spacer::rows(1)));
        rows.push(Box::new(Divider::new("Legend")));
        rows.push(Box::new(Legend::map().species(sim.roster())));
        VStack::from_boxes(&rows).render(buf, inner);
    }
}

mod base;
mod input;
mod stack_sidebar;
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
