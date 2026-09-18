//! Screen stack, key routing and the tick accumulator for the live app.

use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;

use super::app::AppState;
use crate::widgets::util;

pub mod common;
pub mod confirm;
pub mod load_world;
pub mod s00_title;
pub mod s01_map;
pub mod s03_inspector;
pub mod s04_species;
pub mod s05_charts;
pub mod s06_ecology;
pub mod s07_log;
pub mod s08_lineage;
pub mod s09_worldgen;
pub mod s10_controls;
pub mod s11_help;
pub mod s12_alert;
pub mod s13_zoom;
pub mod s14_switcher;
pub mod s15_traits;

#[derive(Debug)]
pub enum Action {
    None,
    Unhandled,
    Push(Box<dyn Screen>),
    Pop,
    Replace(Box<dyn Screen>),
    Quit,
    /// Replace the whole stack with the S00 title screen (C6 FR3).
    GoTitle,
    /// Replace the whole stack with `[Title, WorldMap]` for the named world.
    EnterWorld { name: String },
}

pub trait Screen: std::fmt::Debug {
    /// `false` for modals (the stack draws what is beneath them first).
    fn opaque(&self) -> bool;
    /// Whether the stack dims everything drawn so far before this non-opaque
    /// screen draws. S14 says no: the map beneath it is a live preview.
    fn dims_backdrop(&self) -> bool {
        true
    }
    /// The top screen sees every key first. Return `Unhandled` for keys it does
    /// not consume; only then does the stack apply the global key table.
    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action;
    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect);
}

/// Ordered screen stack; `stack` lives outside `AppState` so a screen is never
/// borrowed twice when it receives `&mut AppState`.
#[derive(Default)]
#[derive(Debug)]
pub struct Stack {
    pub screens: Vec<Box<dyn Screen>>,
}

impl Stack {
    pub fn new() -> Self {
        Self { screens: Vec::new() }
    }

    pub fn push(&mut self, s: Box<dyn Screen>) {
        self.screens.push(s);
    }

    pub fn pop(&mut self) -> Option<Box<dyn Screen>> {
        self.screens.pop()
    }

    pub fn replace(&mut self, s: Box<dyn Screen>) -> Option<Box<dyn Screen>> {
        let old = self.screens.pop();
        self.screens.push(s);
        old
    }

    pub fn len(&self) -> usize {
        self.screens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.screens.is_empty()
    }

    pub fn top_mut(&mut self) -> Option<&mut (dyn Screen + 'static)> {
        self.screens.last_mut().map(AsMut::as_mut)
    }
}

/// Draw every screen from the lowest `opaque` one upward; before drawing a
/// non-opaque screen that `dims_backdrop`, dim everything drawn so far by
/// 55 % (modal backdrop).
pub fn render_stack(stack: &Stack, app: &AppState, f: &mut Frame<'_>, area: Rect) {
    let start = stack.screens.iter().rposition(|s| s.opaque()).unwrap_or(0);
    for i in start..stack.screens.len() {
        let screen = &stack.screens[i];
        if !screen.opaque() && screen.dims_backdrop() {
            util::dim_area(f.buffer_mut(), area, 0.55);
        }
        screen.render(app, f, area);
    }
}

/// Fractional tick accumulator: keeps the sub-tick remainder between frames.
#[derive(Default)]
#[derive(Debug)]
pub struct TickAccumulator {
    frac: f64,
}

impl TickAccumulator {
    pub const fn new() -> Self {
        Self { frac: 0.0 }
    }

    /// Feed `elapsed_secs` at `ticks_per_sec` and return the whole ticks due.
    pub fn add(&mut self, elapsed_secs: f64, ticks_per_sec: f64) -> u64 {
        let total = self.frac + elapsed_secs * ticks_per_sec;
        let whole = crate::cast!(total.floor() => u64);
        self.frac = total - crate::cast!(whole => f64);
        whole
    }
}

#[cfg(test)]
mod tests;
