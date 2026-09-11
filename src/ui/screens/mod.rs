//! Screen stack, key routing and the tick accumulator for the live app.

use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;

use super::app::AppState;
use crate::widgets::util;

pub mod s01_map;
pub mod s05_charts;
pub mod s06_ecology;
pub mod s07_log;
pub mod s09_worldgen;
pub mod s10_controls;
pub mod s11_help;

pub enum Action {
    None,
    Unhandled,
    Push(Box<dyn Screen>),
    Pop,
    Replace(Box<dyn Screen>),
    Quit,
}

pub trait Screen {
    /// `false` for modals (they dim what is drawn beneath them).
    fn opaque(&self) -> bool;
    /// The top screen sees every key first. Return `Unhandled` for keys it does
    /// not consume; only then does the stack apply the global key table.
    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action;
    fn render(&self, app: &AppState, f: &mut Frame, area: Rect);
}

/// Ordered screen stack; `stack` lives outside `AppState` so a screen is never
/// borrowed twice when it receives `&mut AppState`.
#[derive(Default)]
pub struct Stack {
    pub screens: Vec<Box<dyn Screen>>,
}

impl Stack {
    pub fn new() -> Self {
        Stack { screens: Vec::new() }
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
        self.screens.last_mut().map(|b| b.as_mut())
    }
}

/// Draw every screen from the lowest `opaque` one upward; before drawing a
/// non-opaque screen, dim everything drawn so far by 55 % (modal backdrop).
pub fn render_stack(stack: &Stack, app: &AppState, f: &mut Frame, area: Rect) {
    let start = stack.screens.iter().rposition(|s| s.opaque()).unwrap_or(0);
    for i in start..stack.screens.len() {
        let screen = &stack.screens[i];
        if !screen.opaque() {
            util::dim_area(f.buffer_mut(), area, 0.55);
        }
        screen.render(app, f, area);
    }
}

/// Fractional tick accumulator: keeps the sub-tick remainder between frames.
#[derive(Default)]
pub struct TickAccumulator {
    frac: f64,
}

impl TickAccumulator {
    pub fn new() -> Self {
        TickAccumulator { frac: 0.0 }
    }

    /// Feed `elapsed_secs` at `ticks_per_sec` and return the whole ticks due.
    pub fn add(&mut self, elapsed_secs: f64, ticks_per_sec: f64) -> u64 {
        let total = self.frac + elapsed_secs * ticks_per_sec;
        let whole = total.floor() as u64;
        self.frac = total - whole as f64;
        whole
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;
    use crate::ui::app::AppState;
    use crate::ui::screens::s09_worldgen::WorldGen;
    use crate::sim::Params;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::style::{Color, Style};
    use ratatui::Terminal;

    fn state() -> AppState {
        AppState::new(Params::default())
    }

    struct Dummy {
        opaque: bool,
        mark: Option<(u16, u16, Color)>,
    }

    impl Screen for Dummy {
        fn opaque(&self) -> bool {
            self.opaque
        }
        fn handle_key(&mut self, _key: KeyEvent, _app: &mut AppState) -> Action {
            Action::Unhandled
        }
        fn render(&self, _app: &AppState, f: &mut Frame, area: Rect) {
            let _ = area;
            if let Some((x, y, c)) = self.mark {
                if let Some(cell) = f.buffer_mut().cell_mut((x, y)) {
                    cell.set_char('#');
                    cell.set_style(Style::default().fg(c).bg(theme::PANEL_BG));
                }
            }
        }
    }

    #[test]
    fn push_pop_replace() {
        let mut s = Stack::new();
        assert!(s.is_empty());
        s.push(Box::new(Dummy { opaque: true, mark: None }));
        s.push(Box::new(Dummy { opaque: true, mark: None }));
        assert_eq!(s.len(), 2);
        s.pop();
        assert_eq!(s.len(), 1);
        s.replace(Box::new(Dummy { opaque: false, mark: None }));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn modal_renders_below() {
        let app = state();
        let mut stack = Stack::new();
        stack.push(Box::new(Dummy { opaque: true, mark: Some((1, 0, theme::TEXT)) }));
        stack.push(Box::new(Dummy { opaque: false, mark: Some((0, 0, theme::ACCENT)) }));

        let backend = TestBackend::new(4, 2);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_stack(&stack, &app, f, Rect::new(0, 0, 4, 2)))
            .unwrap();
        let buf = terminal.backend().buffer();

        // Modal is drawn on top (undimmed).
        assert_eq!(buf[(0, 0)].fg, theme::ACCENT);
        // The opaque base beneath the modal was dimmed toward the background.
        assert_eq!(buf[(1, 0)].fg, theme::dim(theme::TEXT, 0.55));
    }

    #[test]
    fn tick_accumulator_10s_x1_is_20() {
        let mut acc = TickAccumulator::new();
        assert_eq!(acc.add(10.0, 2.0), 20);
        assert_eq!(acc.frac, 0.0);
        // Fractional carry is preserved.
        assert_eq!(acc.add(0.25, 2.0), 0);
        assert_eq!(acc.add(0.25, 2.0), 1);
    }

    #[test]
    fn s09_text_field_consumes_printable_keys() {
        let mut app = state();
        let mut s = WorldGen::new();
        // Shift+Tab from the default Size focus to Seed (a text field).
        s.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE), &mut app);
        let a = s.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE), &mut app);
        assert!(matches!(a, Action::None));
    }

    #[test]
    fn s09_size_field_arrows_adjust_width_and_height() {
        let mut app = state();
        let mut s = WorldGen::new();
        let (w0, h0) = (app.params.world.width, app.params.world.height);
        // Default focus is Size: Left/Right change width, Up/Down change height.
        s.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &mut app);
        s.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &mut app);
        let p = s.form_params();
        assert_eq!(p.world.height, (h0 + 5).min(1000));
        assert_eq!(p.world.width, (w0 + 10).min(1000));
        s.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &mut app);
        s.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &mut app);
        assert_eq!(s.form_params().world.height, (h0 + 5).min(1000).saturating_sub(10).max(30));
    }

    #[test]
    fn s09_max_size_preview_renders() {
        let mut app = state();
        let mut s = WorldGen::new();
        for _ in 0..100 {
            s.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &mut app);
        }
        for _ in 0..200 {
            s.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &mut app);
        }
        let p = s.form_params();
        assert_eq!((p.world.width, p.world.height), (1000, 1000));
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| s.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap();
    }

    #[test]
    fn s09_q_quits_when_no_text_focus() {
        let mut app = state();
        let mut s = WorldGen::new();
        // Default focus is Size (not a text field), so 'q' quits.
        let a = s.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE), &mut app);
        assert!(matches!(a, Action::Quit));
    }

    #[test]
    fn s07_chip_sets() {
        use crate::sim::EventKind;
        use crate::ui::screens::s07_log::ChipFilter;

        let mut f = ChipFilter::new();
        assert!(f.all);
        // Key 2 selects births only.
        f.toggle(2);
        assert!(!f.all);
        assert!(f.matches(EventKind::Birth));
        assert!(!f.matches(EventKind::DeathPredation));
        // Key 3 adds deaths.
        f.toggle(3);
        assert!(f.matches(EventKind::Birth));
        assert!(f.matches(EventKind::DeathPredation));
        assert!(!f.matches(EventKind::Drought));
        // Toggling both off reverts to all.
        f.toggle(2);
        f.toggle(3);
        assert!(f.all);
        // Cycle: all → deaths+extinctions → migrations+droughts → all.
        f.cycle();
        assert!(f.matches(EventKind::DeathStarved));
        assert!(f.matches(EventKind::Extinction));
        assert!(!f.matches(EventKind::Birth));
        f.cycle();
        assert!(f.matches(EventKind::Migration));
        assert!(f.matches(EventKind::DroughtEased));
        f.cycle();
        assert!(f.all);
    }

    #[test]
    fn s06_status_rule() {
        use crate::sim::params::ScarcityThresholds;
        use crate::ui::screens::s06_ecology::region_status;

        let th = ScarcityThresholds::default();
        assert_eq!(region_status(0.30, 0, true, &th), "Scarce");
        assert_eq!(region_status(0.38, 0, true, &th), "Strained");
        assert_eq!(region_status(0.50, 0, true, &th), "Stable");
        assert_eq!(region_status(0.50, 10, true, &th), "Plenty");
        assert_eq!(region_status(0.50, 0, false, &th), "Plenty");
    }
}
