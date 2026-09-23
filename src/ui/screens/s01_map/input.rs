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
use crate::widgets::map::{Base, Disease, Layer, OverlayStack};
use crate::widgets::panel;
use crate::theme;
use super::WorldMap;
use super::look::move_look_cursor;

impl WorldMap {
    /// Draw the gutter, or the sidebar for the current mode and stack: the
    /// S01 status sidebar with nothing on, a layer's own S02 sidebar with one
    /// layer on, the compact form with more (S14 item 20).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_map_sidebar(&self, f: &mut Frame<'_>, area: Rect, map_w: u16, map_rows: u16, side_w: u16, app: &AppState, sim: &Sim, world: &World, time: &crate::sim::Time, stack: &OverlayStack) {
        if side_w == GUTTER_W {
            let gutter = Rect::new(area.x + map_w, area.y, side_w, map_rows);
            let inner = panel::draw(f, gutter, "", panel::Kind::Outer);
            for (i, ch) in "«SIDEBAR»".chars().enumerate() {
                let y = inner.y + 1 + crate::cast!(i => u16);
                if y < inner.bottom() {
                    f.buffer_mut().set_stringn(inner.x, y, ch.to_string(), 1, theme::key());
                }
            }
            return;
        }
        let side = Rect::new(area.x + map_w, area.y, side_w, map_rows);
        if let Some(id) = app.follow {
            Self::follow_sidebar(f, side, app, sim, id);
        } else if app.look_cursor.is_some() {
            Self::look_sidebar(f, side, app, sim, world);
        } else {
            let layers: Vec<Layer> = stack.layers().collect();
            match layers.as_slice() {
                [] => Self::sidebar(f, side, app, sim, world, time),
                [one] => self.layer_sidebar(f, side, app, sim, world, stack, *one),
                _ => Self::compact_sidebar(f, side, sim, stack),
            }
        }
    }

    /// One layer's own S02 sidebar.
    #[allow(clippy::too_many_arguments)]
    fn layer_sidebar(&self, f: &mut Frame<'_>, side: Rect, app: &AppState, sim: &Sim, world: &World, stack: &OverlayStack, layer: Layer) {
        match layer {
            Layer::Base(Base::None) => {}
            Layer::Base(Base::Vegetation | Base::Pressure | Base::Moisture) => Self::overlay_sidebar(f, side, world, stack),
            Layer::Base(Base::Species) => Self::species_sidebar(f, side, sim, stack),
            Layer::Base(Base::Parasites) => Self::parasite_sidebar(f, side, sim, stack),
            Layer::Base(Base::Scent) => Self::scent_sidebar(f, side, sim, stack),
            Layer::Base(Base::Succession) => Self::succession_sidebar(f, side, sim, stack),
            Layer::Sense => {
                if let Some(id) = stack.sense_subject {
                    Self::sense_sidebar(f, side, app, sim, id, stack);
                }
            }
            Layer::Regions => self.region_sidebar(f, side, app, sim, stack),
            Layer::Health => Self::health_sidebar(f, side, sim, stack),
            Layer::Disease => {
                if let Disease::On(shown) = stack.disease {
                    Self::disease_sidebar(f, side, sim, shown, stack);
                }
            }
        }
    }

    /// `Tab` / `BackTab`: the sub-pick of the first layer that has one, else
    /// (forwards only) the wide view.
    fn cycle_sub_pick(&mut self, app: &mut AppState, backwards: bool, wide: bool) {
        if backwards {
            if !Self::cycle_species(app, true) {
                Self::cycle_pathogen(app, true);
            }
        } else if !Self::cycle_sense(app) && !Self::cycle_species(app, false) && !Self::cycle_pathogen(app, false) && wide {
            self.wide = !self.wide;
        }
    }

    // ---- plain mode (S01a/b/d + overlays) ----
    pub(super) fn handle_plain_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Char('k') => {
                let (vw, vh) = app.viewport_size.get();
                app.enter_look((app.viewport_origin.0 + vw.div_euclid(2), app.viewport_origin.1 + vh.div_euclid(2)));
                Action::None
            }
            KeyCode::Tab => {
                self.cycle_sub_pick(app, false, true);
                Action::None
            }
            KeyCode::BackTab => {
                self.cycle_sub_pick(app, true, false);
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
                if app.overlay.regions {
                    let n = Self::region_count(app);
                    self.region_sel = (self.region_sel + n.saturating_sub(1)) % n.max(1);
                } else {
                    app.scroll_viewport(0, -5);
                }
                Action::None
            }
            KeyCode::Down => {
                if app.overlay.regions {
                    let n = Self::region_count(app);
                    self.region_sel = (self.region_sel + 1) % n.max(1);
                } else {
                    app.scroll_viewport(0, 5);
                }
                Action::None
            }
            KeyCode::Enter if app.overlay.regions => {
                if let Some(sim) = &app.sim {
                    if self.region_sel < sim.world.regions.len() {
                        let (cx, cy) = sim.world.region_centre(self.region_sel);
                        app.centre_viewport_on(cx, cy);
                    }
                }
                Action::None
            }
            // S02d: the status bar offers [i] inspect / [f] follow for the sense subject.
            KeyCode::Char('i') if app.overlay.sense => match app.overlay.sense_subject {
                Some(id) => Action::Push(Box::new(Inspector::new(id))),
                None => Action::None,
            },
            KeyCode::Char('f') if app.overlay.sense => {
                if let Some(id) = app.overlay.sense_subject {
                    app.follow = Some(id);
                    app.follow_death_tick = None;
                }
                Action::None
            }
            KeyCode::Esc => {
                app.overlay.clear();
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
                self.cycle_sub_pick(app, false, false);
                Action::None
            }
            KeyCode::BackTab => {
                self.cycle_sub_pick(app, true, false);
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
                self.cycle_sub_pick(app, false, true);
                Action::None
            }
            KeyCode::BackTab => {
                self.cycle_sub_pick(app, true, false);
                Action::None
            }
            _ => Action::Unhandled,
        }
    }
}
