//! Key handling for the map, and the sidebar dispatch table.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::Frame;
use crate::sim::{Sim, World};
use crate::ui::app::AppState;
use crate::ui::screens::s03_inspector::Inspector;
use crate::ui::screens::s13_zoom::Zoom;
use crate::ui::screens::Action;
use crate::ui::viewport::GUTTER_W;
use crate::widgets::map::Overlay;
use crate::widgets::panel;
use crate::theme;
use super::WorldMap;
use super::look::move_look_cursor;

impl WorldMap {
    // ---- plain mode (S01a/b/d + overlays) ----
    /// Draw the gutter or the overlay-specific sidebar for `overlay`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_map_sidebar(&self, f: &mut Frame<'_>, area: Rect, map_w: u16, map_rows: u16, side_w: u16, app: &AppState, sim: &Sim, world: &World, time: &crate::sim::Time, overlay: Overlay, overlay_active: bool) {
        if side_w == GUTTER_W {
            let gutter = Rect::new(area.x + map_w, area.y, side_w, map_rows);
            let inner = panel::draw(f, gutter, "", panel::Kind::Outer);
            for (i, ch) in "«SIDEBAR»".chars().enumerate() {
                let y = inner.y + 1 + crate::cast!(i => u16);
                if y < inner.bottom() {
                    f.buffer_mut().set_stringn(inner.x, y, ch.to_string(), 1, theme::key());
                }
            }
        } else {
            let side = Rect::new(area.x + map_w, area.y, side_w, map_rows);
            if let Some(id) = app.follow {
                Self::follow_sidebar(f, side, app, sim, id);
            } else if app.look_cursor.is_some() {
                Self::look_sidebar(f, side, app, sim, world);
            } else if let Overlay::Sense(id) = overlay {
                self.sense_sidebar(f, side, app, sim, id);
            } else if overlay == Overlay::Region {
                self.region_sidebar(f, side, app, sim);
            } else if let Overlay::Species(sp) = overlay {
                self.species_sidebar(f, side, sim, sp);
            } else if overlay == Overlay::Health {
                self.health_sidebar(f, side, sim);
            } else if let Overlay::Disease(shown) = overlay {
                self.disease_sidebar(f, side, sim, shown);
            } else if overlay == Overlay::Parasites {
                self.parasite_sidebar(f, side, sim);
            } else if overlay_active {
                self.overlay_sidebar(f, side, app, world);
            } else {
                Self::sidebar(f, side, app, sim, world, time);
            }
        }
    }

    /// `o` cycles through every overlay.
    fn cycle_overlay(&mut self, app: &AppState) {
        self.overlay = match self.overlay {
            Overlay::None => Overlay::Vegetation,
            Overlay::Vegetation => Overlay::Pressure,
            Overlay::Pressure => Overlay::Moisture,
            Overlay::Moisture => match Self::default_sense(app) {
                Some(id) => Overlay::Sense(id),
                None => Overlay::Region,
            },
            Overlay::Sense(_) => Overlay::Region,
            Overlay::Region => Overlay::Species(self.default_species(app)),
            Overlay::Species(_) => Overlay::Health,
            Overlay::Health => Overlay::Disease(None),
            Overlay::Disease(_) => Overlay::Parasites,
            Overlay::Parasites => Overlay::None,
        };
        match self.overlay {
            Overlay::Sense(id) => self.sense_id = Some(id),
            Overlay::Species(sp) => self.species_sel = sp,
            _ => {}
        }
    }

    /// The `1`-`9` overlay shortcuts.
    fn select_overlay(&mut self, c: char, app: &AppState) {
        match c {
            '1' => self.overlay = Overlay::Vegetation,
            '2' => self.overlay = Overlay::Pressure,
            '3' => self.overlay = Overlay::Moisture,
            '4' => {
                if let Some(id) = Self::default_sense(app) {
                    self.overlay = Overlay::Sense(id);
                    self.sense_id = Some(id);
                }
            }
            '5' => self.overlay = Overlay::Region,
            '6' => self.open_species(app),
            '7' => self.overlay = Overlay::Health,
            '8' => self.overlay = Overlay::Disease(None),
            '9' => self.overlay = Overlay::Parasites,
            _ => {}
        }
    }

    pub(super) fn handle_plain_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Char('k') => {
                let (vw, vh) = app.viewport_size.get();
                app.enter_look((app.viewport_origin.0 + vw.div_euclid(2), app.viewport_origin.1 + vh.div_euclid(2)));
                Action::None
            }
            KeyCode::Tab => {
                if !self.cycle_sense(app) && !self.cycle_species(app, false) && !self.cycle_pathogen(app, false) {
                    self.wide = !self.wide;
                }
                Action::None
            }
            KeyCode::BackTab => {
                if !self.cycle_species(app, true) {
                    self.cycle_pathogen(app, true);
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
                    let n = Self::region_count(app);
                    self.region_sel = (self.region_sel + n.saturating_sub(1)) % n.max(1);
                } else {
                    app.scroll_viewport(0, -5);
                }
                Action::None
            }
            KeyCode::Down => {
                if self.overlay == Overlay::Region {
                    let n = Self::region_count(app);
                    self.region_sel = (self.region_sel + 1) % n.max(1);
                } else {
                    app.scroll_viewport(0, 5);
                }
                Action::None
            }
            KeyCode::Enter if self.overlay == Overlay::Region => {
                if let Some(sim) = &app.sim {
                    if let Some(r) = sim.world.regions.get(self.region_sel) {
                        let (cx, cy) = ((r.1 + r.3).div_euclid(2), (r.2 + r.4).div_euclid(2));
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
                self.cycle_overlay(app);
                Action::None
            }
            KeyCode::Char(c @ '1'..='9') => {
                self.select_overlay(c, app);
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
    pub(super) fn handle_look_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        let Some(sim) = &app.sim else { return Action::None };
        let step = if key.modifiers.contains(KeyModifiers::SHIFT) { 10 } else { 1 };
        match key.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                move_look_cursor(&mut app.look_cursor, key.code, step, (sim.world.width(), sim.world.height()));
                Action::None
            }
            KeyCode::Enter => {
                if let Some((x, y)) = app.look_cursor {
                    if let Some(id) = Self::creature_at_cursor(sim, x, y) {
                        app.leave_look();
                        return Action::Push(Box::new(Inspector::new(id)));
                    }
                }
                Action::None
            }
            KeyCode::Char('f') => {
                if let Some((x, y)) = app.look_cursor {
                    if let Some(id) = Self::creature_at_cursor(sim, x, y) {
                        app.follow = Some(id);
                        app.leave_look();
                    }
                }
                Action::None
            }
            KeyCode::Char('z') => Action::Push(Box::new(Zoom::new())),
            KeyCode::Tab => {
                if !self.cycle_sense(app) && !self.cycle_species(app, false) {
                    self.cycle_pathogen(app, false);
                }
                Action::None
            }
            KeyCode::BackTab => {
                if !self.cycle_species(app, true) {
                    self.cycle_pathogen(app, true);
                }
                Action::None
            }
            KeyCode::Char(c @ ('4' | '6' | '7' | '8' | '9')) => {
                self.select_overlay(c, app);
                Action::None
            }
            KeyCode::Esc => {
                app.leave_look();
                Action::None
            }
            _ => Action::Unhandled,
        }
    }

    // ---- follow mode (S01e) ----
    pub(super) fn handle_follow_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
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
                if !self.cycle_sense(app) && !self.cycle_species(app, false) && !self.cycle_pathogen(app, false) {
                    self.wide = !self.wide;
                }
                Action::None
            }
            KeyCode::BackTab => {
                if !self.cycle_species(app, true) {
                    self.cycle_pathogen(app, true);
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
            KeyCode::Char('6') => {
                self.open_species(app);
                Action::None
            }
            KeyCode::Char('7') => {
                self.overlay = Overlay::Health;
                Action::None
            }
            KeyCode::Char('8') => {
                self.overlay = Overlay::Disease(None);
                Action::None
            }
            KeyCode::Char('9') => {
                self.overlay = Overlay::Parasites;
                Action::None
            }
            _ => Action::Unhandled,
        }
    }
}
