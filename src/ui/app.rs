//! Live application shell: shared state, screen stack and the tick loop.

use std::io;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::{DefaultTerminal, Frame};

use crate::sim::{Params, Sim};
use crate::theme;

use super::screens::s09_worldgen::WorldGen;
use super::screens::s10_controls::Controls;
use super::screens::s11_help::Help;
use super::screens::{self, Action, Stack, TickAccumulator};

pub struct AppState {
    pub sim: Option<Sim>,
    pub paused: bool,
    pub speed_idx: usize,
    pub params: Params,
    pub speed_before_alert: Option<usize>,
}

impl AppState {
    pub fn new(params: Params) -> Self {
        AppState { sim: None, paused: false, speed_idx: 0, params, speed_before_alert: None }
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

    pub fn step_ticks(&mut self, n: u64) {
        if let Some(sim) = self.sim.as_mut() {
            for _ in 0..n {
                let _report = sim.step();
            }
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
                self.state.step_ticks(1);
                Action::None
            }
            KeyCode::Char('p') => Action::Push(Box::new(Controls::new())),
            KeyCode::Char('?') => Action::Push(Box::new(Help::new())),
            // C1: `q`/`w` return to world generation (title flow arrives in C6).
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

        if event::poll(Duration::from_millis(16))? {
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
            app.state.step_ticks(ticks_ran);
        }

        // Redraw on key, or on a tick (throttled to ~30 fps).
        if force_draw || (ticks_ran > 0 && last_draw.elapsed() >= min_frame) {
            terminal.draw(|f| app.draw(f))?;
            last_draw = Instant::now();
        }
    }
}
