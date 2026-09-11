//! Live application shell: shared state, screen stack and the tick loop.

use std::cell::Cell;
use std::io;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::{DefaultTerminal, Frame};

use crate::sim::creatures::CreatureId;
use crate::sim::{Alert, Params, Sim};
use crate::theme;

use super::screens::s04_species::SpeciesBrowser;
use super::screens::s05_charts::Charts;
use super::screens::s06_ecology::Ecology;
use super::screens::s07_log::EventLog;
use super::screens::s08_lineage::LineageScreen;
use super::screens::s09_worldgen::WorldGen;
use super::screens::s10_controls::Controls;
use super::screens::s11_help::Help;
use super::screens::s12_alert::AlertModal;
use super::screens::{self, Action, Stack, TickAccumulator};

pub struct AppState {
    pub sim: Option<Sim>,
    pub paused: bool,
    pub speed_idx: usize,
    pub params: Params,
    pub speed_before_alert: Option<usize>,
    /// Map viewport origin (top-left world cell), shared so data screens can
    /// centre the map on a region/event.
    pub viewport_origin: (usize, usize),
    /// Inner size of the map viewport as of the last draw (cols, rows). Written
    /// by the map screen's `render` so key handlers can clamp scrolling.
    pub viewport_size: Cell<(usize, usize)>,
    /// Look-mode cursor cell (S01c); `Some` means the map is in look mode.
    pub look_cursor: Option<(usize, usize)>,
    /// Followed creature id (S01e); `Some` means the map is in follow mode.
    pub follow: Option<CreatureId>,
    /// Tick at which the followed creature died (for the 3-hour grace period).
    pub follow_death_tick: Option<u64>,
    /// Extinction alerts waiting to be shown (C5 FR8).
    pub alert_queue: Vec<Alert>,
    /// The alert currently displayed as the top S12 modal.
    pub alert_shown: Option<Alert>,
}

impl AppState {
    pub fn new(params: Params) -> Self {
        AppState {
            sim: None,
            paused: false,
            speed_idx: 0,
            params,
            speed_before_alert: None,
            viewport_origin: (0, 0),
            viewport_size: Cell::new((0, 0)),
            look_cursor: None,
            follow: None,
            follow_death_tick: None,
            alert_queue: Vec::new(),
            alert_shown: None,
        }
    }

    /// Largest valid viewport origin for the current world and last-drawn viewport.
    pub fn viewport_max(&self) -> (usize, usize) {
        let (vw, vh) = self.viewport_size.get();
        match &self.sim {
            Some(sim) => crate::ui::viewport::max_origin(sim.world.width(), sim.world.height(), vw, vh),
            None => (0, 0),
        }
    }

    /// Scroll the map viewport by `(dx, dy)` cells, clamped to the world so the
    /// origin never overshoots and every key press moves the view.
    pub fn scroll_viewport(&mut self, dx: isize, dy: isize) {
        let (mx, my) = self.viewport_max();
        let (x, y) = self.viewport_origin;
        let x = x.min(mx).saturating_add_signed(dx).min(mx);
        let y = y.min(my).saturating_add_signed(dy).min(my);
        self.viewport_origin = (x, y);
    }

    /// Centre the map viewport on world cell `(cx, cy)`, clamped to the world
    /// using the viewport size measured at the last draw.
    pub fn centre_viewport_on(&mut self, cx: usize, cy: usize) {
        let (vw, vh) = self.viewport_size.get();
        let (mx, my) = self.viewport_max();
        self.viewport_origin = (cx.saturating_sub(vw / 2).min(mx), cy.saturating_sub(vh / 2).min(my));
    }

    pub fn speed(&self) -> u32 {
        self.params.ui.speeds[self.speed_idx]
    }

    pub fn ticks_per_sec(&self) -> f64 {
        self.params.ui.base_ticks_per_second as f64 * self.speed() as f64
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn speed_up(&mut self) {
        if self.speed_idx + 1 < self.params.ui.speeds.len() {
            self.speed_idx += 1;
        }
    }

    pub fn speed_down(&mut self) {
        self.speed_idx = self.speed_idx.saturating_sub(1);
    }

    pub fn set_speed(&mut self, idx: usize) {
        self.speed_idx = idx.min(self.params.ui.speeds.len().saturating_sub(1));
    }

    /// Step `n` ticks and return any alerts raised (C5 FR8).
    pub fn step_ticks(&mut self, n: u64) -> Vec<Alert> {
        let mut out = Vec::new();
        if let Some(sim) = self.sim.as_mut() {
            for _ in 0..n {
                let report = sim.step();
                out.extend(report.alerts);
            }
        }
        out
    }

    /// Queue extinction alerts: record the pre-alert speed and pause (when the
    /// option is on); when off, the events are already logged/tickered only.
    pub fn enqueue_alerts(&mut self, alerts: Vec<Alert>) {
        if alerts.is_empty() {
            return;
        }
        if self.params.ui.auto_pause_on_extinction {
            if self.speed_before_alert.is_none() {
                self.speed_before_alert = Some(self.speed_idx);
            }
            self.paused = true;
            self.alert_queue.extend(alerts);
        }
    }

    /// Pop the next pending alert into `alert_shown`, if one is free.
    pub fn take_next_alert(&mut self) -> Option<Alert> {
        if self.alert_shown.is_some() || self.alert_queue.is_empty() {
            return None;
        }
        let a = self.alert_queue.remove(0);
        self.alert_shown = Some(a.clone());
        Some(a)
    }

    /// The current S12 modal was dismissed. Restore speed only when the whole
    /// queue is drained (Continue on the last alert).
    pub fn dismiss_alert(&mut self, restore_speed: bool) {
        self.alert_shown = None;
        if restore_speed && self.alert_queue.is_empty() {
            if let Some(s) = self.speed_before_alert.take() {
                self.speed_idx = s;
                self.paused = false;
            }
        }
    }

    /// Follow-mode death handling (FR11): pause on death when configured, else
    /// end follow after 3 simulated hours.
    pub fn handle_follow(&mut self) {
        let Some(id) = self.follow else { return };
        let Some(sim) = &self.sim else { return };
        if sim.creatures.get(id).is_none_or(|c| c.alive) {
            self.follow_death_tick = None;
            return;
        }
        let tick = sim.time.tick;
        if self.follow_death_tick.is_none() {
            self.follow_death_tick = Some(tick);
        }
        if self.params.ui.pause_on_follow_death {
            self.paused = true;
        } else if tick >= self.follow_death_tick.unwrap_or(0) + 3 {
            self.follow = None;
            self.follow_death_tick = None;
        }
    }
}

pub struct App {
    pub state: AppState,
    pub stack: Stack,
}

impl App {
    pub fn new(params: Params) -> Self {
        App { state: AppState::new(params), stack: Stack::new() }
    }

    /// Apply the global key table to an unhandled key.
    fn handle_global(&mut self, code: KeyCode) -> Action {
        match code {
            KeyCode::Esc => Action::Pop,
            KeyCode::Char(' ') => {
                self.state.toggle_pause();
                Action::None
            }
            KeyCode::Char('+') => {
                self.state.speed_up();
                Action::None
            }
            KeyCode::Char('-') => {
                self.state.speed_down();
                Action::None
            }
            KeyCode::Char('.') => {
                let alerts = self.state.step_ticks(1);
                self.state.enqueue_alerts(alerts);
                Action::None
            }
            KeyCode::Char('p') => Action::Push(Box::new(Controls::new())),
            KeyCode::Char('?') => Action::Push(Box::new(Help::new())),
            KeyCode::Char('e') => Action::Push(Box::new(EventLog::new())),
            KeyCode::Char('y') => Action::Push(Box::new(Ecology::new())),
            KeyCode::Char('g') => Action::Push(Box::new(Charts::new())),
            KeyCode::Char('s') => Action::Push(Box::new(SpeciesBrowser::new())),
            KeyCode::Char('l') => {
                // Lineage of the followed creature, else the oldest living one.
                let focus = self.state.follow.or_else(|| self.state.sim.as_ref().and_then(|s| s.oldest_living()));
                match focus {
                    Some(id) => Action::Push(Box::new(LineageScreen::new(id))),
                    None => Action::None,
                }
            }
            // `q`/`w` return to world generation (title flow arrives in C6).
            KeyCode::Char('q') | KeyCode::Char('w') => Action::Push(Box::new(WorldGen::new())),
            _ => Action::None,
        }
    }

    /// Route a key to the top screen, then to the global table if unhandled.
    /// Returns `true` when the app should quit.
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let action = match self.stack.top_mut() {
            Some(top) => top.handle_key(key, &mut self.state),
            None => Action::None,
        };
        let action = match action {
            Action::Unhandled => self.handle_global(key.code),
            other => other,
        };
        match action {
            Action::None | Action::Unhandled => false,
            Action::Push(s) => {
                self.stack.push(s);
                false
            }
            Action::Pop => {
                self.stack.pop();
                false
            }
            Action::Replace(s) => {
                self.stack.replace(s);
                false
            }
            Action::Quit => true,
        }
    }

    pub fn draw(&self, f: &mut Frame) {
        let area = f.area();
        f.render_widget(Paragraph::new("").style(Style::default().bg(theme::BG)), area);
        screens::render_stack(&self.stack, &self.state, f, area);
    }
}

pub fn run(terminal: &mut DefaultTerminal, params: Params) -> io::Result<()> {
    let mut app = App::new(params.clone());
    app.stack.push(Box::new(WorldGen::from_params(params)));

    let mut acc = TickAccumulator::new();
    let mut last_frame = Instant::now();
    let mut last_draw = Instant::now();
    let min_frame = Duration::from_millis(33);

    loop {
        let mut force_draw = false;

        // Wait briefly for input, then drain everything queued (key repeat can
        // deliver several presses per frame) before stepping and drawing.
        let mut wait = Duration::from_millis(16);
        while event::poll(wait)? {
            wait = Duration::ZERO;
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    force_draw = true;
                    if app.handle_key(k) {
                        return Ok(());
                    }
                }
                Event::Resize(_, _) => force_draw = true,
                _ => {}
            }
        }

        let now = Instant::now();
        let elapsed = last_frame.elapsed().as_secs_f64();
        last_frame = now;

        let mut ticks_ran = 0u64;
        if !app.state.paused {
            let tps = app.state.ticks_per_sec();
            ticks_ran = acc.add(elapsed, tps).min(200);
            let alerts = app.state.step_ticks(ticks_ran);
            app.state.enqueue_alerts(alerts);
        }
        app.state.handle_follow();

        // Raise the next queued extinction alert, if one is pending and none is shown.
        if let Some(alert) = app.state.take_next_alert() {
            app.stack.push(Box::new(AlertModal::new(&alert)));
            force_draw = true;
        }

        // Redraw on key, or on a tick (throttled to ~30 fps).
        if force_draw || (ticks_ran > 0 && last_draw.elapsed() >= min_frame) {
            terminal.draw(|f| app.draw(f))?;
            last_draw = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centre_viewport_on_clamps() {
        let mut app = AppState::new(Params::default());
        app.sim = Some(Sim::new(1, Params::default()));
        app.viewport_size.set((110, 40));
        let (w, h) = (app.sim.as_ref().unwrap().world.width(), app.sim.as_ref().unwrap().world.height());
        app.centre_viewport_on(0, 0);
        assert_eq!(app.viewport_origin, (0, 0));
        app.centre_viewport_on(w, h);
        assert_eq!(app.viewport_origin, (w - 110, h - 40));
        app.centre_viewport_on(60, 25);
        assert_eq!(app.viewport_origin, (5.min(w - 110), 5.min(h - 40)));
    }

    #[test]
    fn scroll_never_overshoots_and_up_moves_immediately() {
        let mut app = AppState::new(Params::default());
        app.sim = Some(Sim::new(1, Params::default()));
        // 150x40 world drawn in a 110x30 viewport → max origin (40, 10).
        app.viewport_size.set((110, 30));
        assert_eq!(app.viewport_max(), (40, 10));

        // Hammer Down well past the bottom edge: origin must stop at the max.
        for _ in 0..20 {
            app.scroll_viewport(0, 5);
        }
        assert_eq!(app.viewport_origin, (0, 10));
        // One Up tap moves the view at once (no hidden overshoot to burn off).
        app.scroll_viewport(0, -5);
        assert_eq!(app.viewport_origin, (0, 5));

        for _ in 0..20 {
            app.scroll_viewport(5, 0);
        }
        assert_eq!(app.viewport_origin, (40, 5));
        app.scroll_viewport(-5, 0);
        assert_eq!(app.viewport_origin, (35, 5));

        // Never goes below zero either.
        app.scroll_viewport(-100, -100);
        assert_eq!(app.viewport_origin, (0, 0));
    }

    #[test]
    fn scroll_recovers_from_stale_origin_beyond_max() {
        let mut app = AppState::new(Params::default());
        app.sim = Some(Sim::new(1, Params::default()));
        app.viewport_size.set((110, 30));
        // An origin set by another screen past the max is pulled back first.
        app.viewport_origin = (500, 500);
        app.scroll_viewport(0, -5);
        assert_eq!(app.viewport_origin, (40, 5));
    }
}
