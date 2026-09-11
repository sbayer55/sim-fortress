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
    use crate::ui::screens::s01_map::WorldMap;
    use crate::ui::screens::s09_worldgen::WorldGen;
    use crate::sim::{Params, Sim, SpeciesId};
    use crate::widgets::map::Overlay;
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
    fn s04_sort_cycle() {
        use crate::ui::screens::s04_species::{sorted_indices, SortCol, SpeciesBrowser};
        let mut app = state();
        app.sim = Some(Sim::new(1, Params::default()));
        let mut s = SpeciesBrowser::new();
        assert_eq!(s.sort, SortCol::Count);
        let expect = [SortCol::Births, SortCol::Deaths, SortCol::Generation, SortCol::Name, SortCol::Count];
        for want in expect {
            s.handle_key(key(KeyCode::Char('s')), &mut app);
            assert_eq!(s.sort, want);
        }
        // Count order: voles (240) before hares (180) before deer (90); absent species last in species order.
        let sim = app.sim.as_ref().unwrap();
        assert_eq!(sorted_indices(sim, SortCol::Count), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(sorted_indices(sim, SortCol::Name), vec![2, 3, 1, 5, 0, 4]);
        // Ties (all zero births) keep species order.
        assert_eq!(sorted_indices(sim, SortCol::Births), vec![0, 1, 2, 3, 4, 5]);
        // Enter opens the detail screen.
        assert!(matches!(s.handle_key(key(KeyCode::Enter), &mut app), Action::Push(_)));
    }

    #[test]
    fn s07_birth_ticker_gate() {
        use crate::sim::{Event, EventKind};
        let (mut app, map) = map_with_sim();
        let sim = app.sim.as_mut().unwrap();
        let mk = |kind: EventKind, text: &str| Event {
            year: 1,
            day: 1,
            hour: 6,
            kind,
            species: None,
            subject: None,
            text: text.to_string(),
            pos: None,
            detail: String::new(),
        };
        sim.events.push(mk(EventKind::Note, "a note"));
        sim.events.push(mk(EventKind::Birth, "a birth"));
        let render = |app: &AppState| -> String {
            let backend = TestBackend::new(155, 45);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal.draw(|f| map.render(app, f, Rect::new(0, 0, 155, 45))).unwrap();
            let buf = terminal.backend().buffer();
            // The ticker sits directly under the map (row `height − MAP_CHROME_ROWS`).
            let ticker_row = 45 - crate::ui::viewport::MAP_CHROME_ROWS;
            (0..155).map(|x| buf[(x, ticker_row)].symbol().to_string()).collect::<String>()
        };
        app.params.ui.log_births = false;
        let off = render(&app);
        assert!(off.contains("a note"), "ticker should fall back to the last non-birth event: {off:?}");
        assert!(!off.contains("a birth"));
        app.params.ui.log_births = true;
        let on = render(&app);
        assert!(on.contains("a birth"), "ticker should show births when log_births is on: {on:?}");
        // Births are always in the log itself.
        assert!(app.sim.as_ref().unwrap().events.iter().any(|e| e.kind == EventKind::Birth));
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
    fn s09_space_opens_typed_entry_on_numeric_fields() {
        let mut app = state();
        let mut s = WorldGen::new();
        let k = |c| KeyEvent::new(c, KeyModifiers::NONE);
        // Size field: Space, type "250x120", Enter.
        s.handle_key(k(KeyCode::Char(' ')), &mut app);
        for c in "250x120".chars() {
            s.handle_key(k(KeyCode::Char(c)), &mut app);
        }
        // Esc cancels an open entry rather than leaving the screen.
        let a = s.handle_key(k(KeyCode::Esc), &mut app);
        assert!(matches!(a, Action::None));
        let p = s.form_params();
        assert_eq!((p.world.width, p.world.height), (Params::default().world.width, Params::default().world.height));
        // Try again and apply with Enter.
        s.handle_key(k(KeyCode::Char(' ')), &mut app);
        for c in "250x120".chars() {
            s.handle_key(k(KeyCode::Char(c)), &mut app);
        }
        let a = s.handle_key(k(KeyCode::Enter), &mut app);
        assert!(matches!(a, Action::None), "Enter applies the typed value instead of generating");
        let p = s.form_params();
        assert_eq!((p.world.width, p.world.height), (250, 120));

        // Water %: typed values are clamped to the field range.
        s.handle_key(k(KeyCode::Tab), &mut app);
        s.handle_key(k(KeyCode::Char(' ')), &mut app);
        for c in "99".chars() {
            s.handle_key(k(KeyCode::Char(c)), &mut app);
        }
        s.handle_key(k(KeyCode::Enter), &mut app);
        assert_eq!(s.form_params().world.water_pct, 60);

        // Tab applies and moves on; Backspace edits the buffer.
        s.handle_key(k(KeyCode::Char(' ')), &mut app);
        s.handle_key(k(KeyCode::Char('1')), &mut app);
        s.handle_key(k(KeyCode::Char('9')), &mut app);
        s.handle_key(k(KeyCode::Backspace), &mut app);
        s.handle_key(k(KeyCode::Char('2')), &mut app);
        s.handle_key(k(KeyCode::Tab), &mut app);
        let p = s.form_params();
        assert_eq!(p.world.water_pct, 12);
        // Now on Forest %: a float field further down accepts decimals.
        for _ in 0..10 {
            s.handle_key(k(KeyCode::Tab), &mut app); // -> field 14, Mutation rate
        }
        s.handle_key(k(KeyCode::Char(' ')), &mut app);
        for c in "0.15".chars() {
            s.handle_key(k(KeyCode::Char(c)), &mut app);
        }
        s.handle_key(k(KeyCode::Enter), &mut app);
        assert!((s.form_params().genetics.mutation_rate - 0.15).abs() < 1e-6);

        // The open entry renders with a caret.
        s.handle_key(k(KeyCode::Char(' ')), &mut app);
        s.handle_key(k(KeyCode::Char('7')), &mut app);
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| s.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap();
        let all: String = (0..45).map(|y| (0..66).map(|x| terminal.backend().buffer()[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect();
        assert!(all.contains("7_"), "caret shown while typing:\n{all}");
    }

    #[test]
    fn s09_q_returns_to_title_when_no_text_focus() {
        let mut app = state();
        let mut s = WorldGen::new();
        // Default focus is Size (not a text field), so 'q' returns to the title.
        let a = s.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE), &mut app);
        assert!(matches!(a, Action::Pop));
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

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn map_with_sim() -> (AppState, WorldMap) {
        let mut app = state();
        app.sim = Some(Sim::new(1, Params::default()));
        (app, WorldMap::new("Test".into()))
    }

    #[test]
    fn s01_key_5_and_o_cycle_reach_region_overlay() {
        let (mut app, mut s) = map_with_sim();
        s.handle_key(key(KeyCode::Char('5')), &mut app);
        assert_eq!(s.overlay, Overlay::Region);
        s.handle_key(key(KeyCode::Char('3')), &mut app);
        // The cycle runs Moisture → Sense → Region → Species → None.
        s.handle_key(key(KeyCode::Char('o')), &mut app);
        assert!(matches!(s.overlay, Overlay::Sense(_)), "Moisture → Sense, got {:?}", s.overlay);
        s.handle_key(key(KeyCode::Char('o')), &mut app);
        assert_eq!(s.overlay, Overlay::Region);
        s.handle_key(key(KeyCode::Char('o')), &mut app);
        assert!(matches!(s.overlay, Overlay::Species(_)), "Region → Species, got {:?}", s.overlay);
        s.handle_key(key(KeyCode::Char('o')), &mut app);
        assert_eq!(s.overlay, Overlay::None);
    }

    #[test]
    fn s01_key_6_opens_species_overlay_and_tab_cycles_species() {
        let (mut app, mut s) = map_with_sim();
        s.handle_key(key(KeyCode::Char('6')), &mut app);
        let Overlay::Species(first) = s.overlay else { panic!("6 opens the species overlay, got {:?}", s.overlay) };
        assert!(app.sim.as_ref().unwrap().creatures.living().any(|c| c.species == first), "defaults to a living species");
        // Tab walks every species in ALL order, wrapping; the sidebar stays put.
        let n = SpeciesId::ALL.len();
        for i in 1..=n {
            s.handle_key(key(KeyCode::Tab), &mut app);
            assert_eq!(s.overlay, Overlay::Species(SpeciesId::ALL[(first.index() + i) % n]));
        }
        assert!(!s.wide, "Tab cycles species instead of toggling the sidebar");
        s.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE), &mut app);
        assert_eq!(s.overlay, Overlay::Species(SpeciesId::ALL[(first.index() + n - 1) % n]));
        // Arrows still scroll under the species overlay.
        app.viewport_size.set((110, 40));
        s.handle_key(key(KeyCode::Right), &mut app);
        assert_eq!(app.viewport_origin.0, 5);
        // Esc clears; reopening restores the last species shown.
        s.handle_key(key(KeyCode::Esc), &mut app);
        assert_eq!(s.overlay, Overlay::None);
        s.handle_key(key(KeyCode::Char('6')), &mut app);
        assert_eq!(s.overlay, Overlay::Species(SpeciesId::ALL[(first.index() + n - 1) % n]));
    }

    #[test]
    fn s01_species_overlay_renders_155x45() {
        let (mut app, mut s) = map_with_sim();
        s.handle_key(key(KeyCode::Char('6')), &mut app);
        s.overlay = Overlay::Species(SpeciesId::Vole);
        s.world_name = "The Valley of Sunfall".into();
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| s.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap();
        let buf = terminal.backend().buffer();
        let row = |y: u16| -> String { (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect() };
        assert!(row(0).contains("overlay: voles"), "{}", row(0));
        let all: String = (0..45).map(row).collect::<Vec<_>>().join("\n");
        assert!(all.contains("Vole density"));
        assert!(all.contains("Lynx"));
        assert!(all.contains("next species"), "status bar names Tab");
        assert!(all.contains("6 species"), "selector lists the sixth overlay");
    }

    #[test]
    fn s01_region_updown_wraps_and_enter_centres() {
        let (mut app, mut s) = map_with_sim();
        app.viewport_size.set((110, 40));
        s.handle_key(key(KeyCode::Char('5')), &mut app);
        let n = app.sim.as_ref().unwrap().world.regions.len();
        s.handle_key(key(KeyCode::Up), &mut app);
        assert_eq!(s.region_sel, n - 1);
        s.handle_key(key(KeyCode::Down), &mut app);
        assert_eq!(s.region_sel, 0);
        // Up/Down select rather than scroll under the region overlay.
        assert_eq!(app.viewport_origin, (0, 0));
        s.region_sel = n - 1;
        s.handle_key(key(KeyCode::Enter), &mut app);
        let r = app.sim.as_ref().unwrap().world.regions[n - 1].clone();
        let (cx, cy) = ((r.1 + r.3) / 2, (r.2 + r.4) / 2);
        let (mx, my) = app.viewport_max();
        assert_eq!(app.viewport_origin, (cx.saturating_sub(55).min(mx), cy.saturating_sub(20).min(my)));
    }

    #[test]
    fn s01_region_overlay_renders_155x45() {
        let (mut app, mut s) = map_with_sim();
        s.handle_key(key(KeyCode::Char('5')), &mut app);
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| s.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap();
        let buf = terminal.backend().buffer();
        let row = |y: u16| -> String { (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect() };
        assert!(row(0).contains("overlay: regions"), "{}", row(0));
        let side: String = (0..45).map(row).collect::<Vec<_>>().join("\n");
        assert!(side.contains("Fenlands"));
        assert!(side.contains("Enter"));
    }
}
