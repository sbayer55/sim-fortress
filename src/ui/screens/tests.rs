//! Tests for the screen stack and the screens that have no test module of their own.

#![allow(clippy::float_cmp)]


use super::*;
use crate::theme;
use crate::ui::app::AppState;
use crate::ui::screens::s01_map::WorldMap;
use crate::ui::screens::s09_worldgen::WorldGen;
use crate::sim::{Params, Sim, SpeciesId};
use crate::ui::style::SpeciesStyle;
use crate::widgets::map::{Base, OverlayStack};
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::{Color, Style};
use ratatui::Terminal;

fn state() -> AppState {
    AppState::new(Params::default())
}

#[derive(Debug)]
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
    fn render(&self, _app: &AppState, f: &mut Frame<'_>, area: Rect) {
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
    // Shift+Tab from the default Map width focus to Seed (a text field).
    s.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE), &mut app);
    let a = s.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE), &mut app);
    assert!(matches!(a, Action::None));
}

#[test]
fn s09_width_and_height_fields_adjust_with_arrows() {
    let mut app = state();
    let mut s = WorldGen::new();
    let (w0, h0) = (app.params.world.width, app.params.world.height);
    // Default focus is Map width: Left/Right change the width.
    s.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &mut app);
    assert_eq!(s.form_params().world.width, (w0 + 10).min(1000));
    // Up/Down are no longer a second axis for the size.
    s.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &mut app);
    assert_eq!(s.form_params().world.height, h0);
    // Tab to Map height: Left/Right change the height.
    s.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &mut app);
    s.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &mut app);
    s.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &mut app);
    assert_eq!(s.form_params().world.height, (h0 + 10).min(1000));
    s.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), &mut app);
    assert_eq!(s.form_params().world.height, (h0 + 5).min(1000));
}

#[test]
fn s09_renders_width_and_height_as_separate_fields() {
    let app = state();
    let s = WorldGen::new();
    let backend = TestBackend::new(155, 45);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| s.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap();
    let form: String = (0..45).map(|y| (0..66).map(|x| terminal.backend().buffer()[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect();
    assert!(form.contains("Map width"), "width is its own field:\n{form}");
    assert!(form.contains("Map height"), "height is its own field:\n{form}");
    assert!(!form.contains("150 x 40"), "the combined size field is gone:\n{form}");
}

#[test]
fn s09_max_size_preview_renders() {
    let mut app = state();
    let mut s = WorldGen::new();
    // Map width is focused first; max it out.
    for _ in 0..100 {
        s.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &mut app);
    }
    s.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &mut app);
    // Map height.
    for _ in 0..200 {
        s.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &mut app);
    }
    let p = s.form_params();
    assert_eq!((p.world.width, p.world.height), (1000, 1000));
    let backend = TestBackend::new(155, 45);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| s.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap();
}

#[test]
fn s09_typed_size_entry_applies_and_cancels() {
    let mut app = state();
    let mut s = WorldGen::new();
    let k = |c| KeyEvent::new(c, KeyModifiers::NONE);
    // Map width: Space, type "250", Esc cancels rather than leaving.
    s.handle_key(k(KeyCode::Char(' ')), &mut app);
    for c in "250".chars() {
        s.handle_key(k(KeyCode::Char(c)), &mut app);
    }
    let a = s.handle_key(k(KeyCode::Esc), &mut app);
    assert!(matches!(a, Action::None));
    assert_eq!(s.form_params().world.width, Params::default().world.width);
    // Try again and apply with Enter.
    s.handle_key(k(KeyCode::Char(' ')), &mut app);
    for c in "250".chars() {
        s.handle_key(k(KeyCode::Char(c)), &mut app);
    }
    let a = s.handle_key(k(KeyCode::Enter), &mut app);
    assert!(matches!(a, Action::None), "Enter applies the typed value instead of generating");
    assert_eq!(s.form_params().world.width, 250);
    // Map height is its own numeric field with the same typed entry.
    s.handle_key(k(KeyCode::Tab), &mut app);
    s.handle_key(k(KeyCode::Char(' ')), &mut app);
    for c in "120".chars() {
        s.handle_key(k(KeyCode::Char(c)), &mut app);
    }
    let a = s.handle_key(k(KeyCode::Enter), &mut app);
    assert!(matches!(a, Action::None));
    let p = s.form_params();
    assert_eq!((p.world.width, p.world.height), (250, 120));
}

#[test]
fn s09_typed_entry_clamps_and_edits() {
    let mut app = state();
    let mut s = WorldGen::new();
    let k = |c| KeyEvent::new(c, KeyModifiers::NONE);
    // Water %: typed values are clamped to the field range.
    s.handle_key(k(KeyCode::Tab), &mut app); // -> Map height
    s.handle_key(k(KeyCode::Tab), &mut app); // -> Water %
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
    for _ in 0..11 {
        s.handle_key(k(KeyCode::Tab), &mut app); // -> field 16, Mutation rate
    }
    s.handle_key(k(KeyCode::Char(' ')), &mut app);
    for c in "0.15".chars() {
        s.handle_key(k(KeyCode::Char(c)), &mut app);
    }
    s.handle_key(k(KeyCode::Enter), &mut app);
    assert!((s.form_params().genetics.mutation_rate - 0.15).abs() < 1e-6);
}

#[test]
fn s09_typed_entry_renders_caret() {
    let mut app = state();
    let mut s = WorldGen::new();
    let k = |c| KeyEvent::new(c, KeyModifiers::NONE);
    s.handle_key(k(KeyCode::Char(' ')), &mut app);
    s.handle_key(k(KeyCode::Char('7')), &mut app);
    let backend = TestBackend::new(155, 45);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| s.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap();
    let all: String = (0..45).map(|y| (0..66).map(|x| terminal.backend().buffer()[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect();
    assert!(all.contains("7_"), "caret shown while typing:\n{all}");
}

#[test]
fn s09_lists_one_row_per_roster_species() {
    // A seventh species from a params overlay: the form grows a row, the
    // Tab order grows with it, and Generate carries the roster through.
    let mut p = Params::default();
    p.apply_overlay("[[species]]\nname = \"boar\"\nplural = \"Boars\"\nglyph = \"b\"\ninitial_count = 40\n").unwrap();
    let mut app = AppState::new(p.clone());
    let mut s = WorldGen::from_params(&p);
    let backend = TestBackend::new(155, 45);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| s.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap();
    let form: String = (0..45).map(|y| (0..66).map(|x| terminal.backend().buffer()[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect();
    assert!(form.contains("Boars"), "the seventh species has a row:\n{form}");
    assert!(form.contains("Lynxes"), "the built-in species keep theirs:\n{form}");
    // Tab all the way round: 10 world fields, 7 species rows, 12 tail fields.
    let k = |c| KeyEvent::new(c, KeyModifiers::NONE);
    for _ in 0..(10 + 7 + 12) {
        s.handle_key(k(KeyCode::Tab), &mut app);
    }
    let generated = s.form_params();
    assert_eq!(generated.species.len(), 7);
    assert_eq!(generated.species.0[6].name, "boar");
    assert_eq!(generated.species.0[6].initial_count, 40);
}

#[test]
fn s09_q_returns_to_title_when_no_text_focus() {
    let mut app = state();
    let mut s = WorldGen::new();
    // Default focus is Map width (not a text field), so 'q' returns to the title.
    let a = s.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE), &mut app);
    assert!(matches!(a, Action::Pop));
}

#[test]
fn s07_chip_toggles() {
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
    // Key 9 selects the wary chip (C5 FR5b).
    f.toggle(9);
    assert!(!f.all);
    assert!(f.matches(EventKind::Wary));
    assert!(!f.matches(EventKind::Birth));
}

#[test]
fn s07_chip_cycle() {
    use crate::sim::EventKind;
    use crate::ui::screens::s07_log::ChipFilter;

    let mut f = ChipFilter::new();
    // Cycle: all → deaths+extinctions → migrations+droughts → all.
    f.cycle();
    assert!(f.matches(EventKind::DeathStarved));
    assert!(f.matches(EventKind::Extinction));
    assert!(!f.matches(EventKind::Birth));
    f.cycle();
    assert!(f.matches(EventKind::Migration));
    assert!(f.matches(EventKind::DroughtEased));
    assert!(!f.matches(EventKind::Outbreak));
    f.cycle();
    assert!(f.matches(EventKind::Outbreak));
    assert!(f.matches(EventKind::Recovery));
    assert!(!f.matches(EventKind::DeathDisease), "disease deaths stay under the deaths chip");
    f.cycle();
    assert!(f.all);
}

#[test]
fn s07_chip_disease_deaths() {
    use crate::sim::EventKind;
    use crate::ui::screens::s07_log::ChipFilter;

    let mut f = ChipFilter::new();
    // Key 8 is the disease chip; disease deaths live under deaths (key 3).
    f.toggle(8);
    assert_eq!(f.kinds, [false, false, false, false, false, false, true, false]);
    assert!(f.matches(EventKind::Epidemic));
    assert!(!f.matches(EventKind::DeathDisease));
    f.toggle(3);
    assert!(f.matches(EventKind::DeathDisease));
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
fn s01_esc_clears_the_stack_and_keeps_the_sub_picks() {
    let (mut app, mut s) = map_with_sim();
    app.overlay = OverlayStack { base: Base::Species, species: SpeciesId(2), regions: true, ..OverlayStack::PLAIN };
    s.handle_key(key(KeyCode::Esc), &mut app);
    assert!(app.overlay.is_empty());
    assert_eq!(app.overlay.species, SpeciesId(2), "the species survives Esc");
    // Esc on an empty stack is still the map's key, not the global Pop.
    assert!(matches!(s.handle_key(key(KeyCode::Esc), &mut app), Action::None));
}

#[test]
fn s01_look_mode_pauses_and_esc_restores_previous_speed() {
    let (mut app, mut s) = map_with_sim();
    app.set_speed(2);
    assert!(!app.paused);
    s.handle_key(key(KeyCode::Char('k')), &mut app);
    assert!(app.look_cursor.is_some());
    assert!(app.paused, "entering look mode pauses the sim");
    // Cursor moves keep the sim paused and the remembered speed intact.
    s.handle_key(key(KeyCode::Right), &mut app);
    assert!(app.paused);
    s.handle_key(key(KeyCode::Esc), &mut app);
    assert!(app.look_cursor.is_none());
    assert!(!app.paused, "leaving look mode resumes");
    assert_eq!(app.speed_idx, 2, "and restores the previous speed");
}

#[test]
fn s01_look_mode_restores_paused_when_entered_paused() {
    let (mut app, mut s) = map_with_sim();
    app.paused = true;
    s.handle_key(key(KeyCode::Char('k')), &mut app);
    assert!(app.paused);
    s.handle_key(key(KeyCode::Esc), &mut app);
    assert!(app.paused, "was paused before look mode, stays paused after");
}

#[test]
fn s01_look_mode_selection_restores_speed() {
    let (mut app, mut s) = map_with_sim();
    app.set_speed(1);
    s.handle_key(key(KeyCode::Char('k')), &mut app);
    assert!(app.paused);
    let pos = app.sim.as_ref().unwrap().creatures.living().next().map(|c| (c.x, c.y)).expect("a living creature");
    app.look_cursor = Some(pos);
    // Following the creature under the cursor is a selection: look ends.
    s.handle_key(key(KeyCode::Char('f')), &mut app);
    assert!(app.follow.is_some());
    assert!(app.look_cursor.is_none());
    assert!(!app.paused);
    assert_eq!(app.speed_idx, 1);
    // Inspecting is a selection too.
    app.follow = None;
    s.handle_key(key(KeyCode::Char('k')), &mut app);
    app.look_cursor = Some(pos);
    let action = s.handle_key(key(KeyCode::Enter), &mut app);
    assert!(matches!(action, Action::Push(_)));
    assert!(app.look_cursor.is_none());
    assert!(!app.paused);
    assert_eq!(app.speed_idx, 1);
}

#[test]
fn s01_health_mark_keeps_the_map_keys_in_every_mode() {
    let (mut app, mut s) = map_with_sim();
    app.overlay.health = true;
    // Arrows scroll, Tab still toggles the sidebar (no sub-pick to cycle).
    app.viewport_size.set((110, 40));
    s.handle_key(key(KeyCode::Right), &mut app);
    assert_eq!(app.viewport_origin.0, 5);
    s.handle_key(key(KeyCode::Tab), &mut app);
    assert!(s.wide);
    s.handle_key(key(KeyCode::Tab), &mut app);
    s.handle_key(key(KeyCode::Esc), &mut app);
    assert!(app.overlay.is_empty());
    // Look mode keeps the mark; Esc leaves the mode first, as before.
    app.overlay.health = true;
    s.handle_key(key(KeyCode::Char('k')), &mut app);
    assert!(app.look_cursor.is_some() && app.overlay.health, "look mode stays on under the mark");
    s.handle_key(key(KeyCode::Esc), &mut app);
    assert!(app.look_cursor.is_none() && app.overlay.health);
    // Follow mode too.
    app.follow = app.sim.as_ref().unwrap().creatures.living_ids().first().copied();
    s.handle_key(key(KeyCode::Tab), &mut app);
    assert!(s.wide && app.overlay.health);
}

#[test]
fn s01_health_overlay_renders_155x45() {
    let (mut app, mut s) = map_with_sim();
    app.overlay.health = true;
    s.world_name = "The Valley of Sunfall".into();
    let backend = TestBackend::new(155, 45);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| s.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap();
    let buf = terminal.backend().buffer();
    let row = |y: u16| -> String { (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect() };
    assert!(row(0).contains("overlay: health"), "{}", row(0));
    let all: String = (0..45).map(row).collect::<Vec<_>>().join("\n");
    assert!(all.contains("Weakest vital"));
    assert!(all.contains("Lynx"));
    // Every living creature is drawn in a condition colour, never its species colour.
    let sim = app.sim.as_ref().unwrap();
    let mut seen = 0;
    for c in sim.creatures.living().filter(|c| c.alive && c.x < 110 && c.y < 40) {
        let cell = &buf[(1 + crate::cast!(c.x => u16), 1 + crate::cast!(c.y => u16))];
        if cell.symbol() != sim.roster().glyph(c.species).to_string() && cell.symbol() != sim.roster().adult_glyph(c.species).to_string() {
            continue; // another creature or resource drew over it
        }
        assert!([theme::GOOD, theme::WARN, theme::BAD].contains(&cell.fg), "{:?} at {},{}", cell.fg, c.x, c.y);
        seen += 1;
    }
    assert!(seen > 10, "saw {seen} creatures");
    // The sidebar tallies add up to the living population.
    let n = sim.creatures.living().filter(|c| c.alive).count();
    assert!(all.contains(&format!("all     {n:>4}")), "{all}");
}

#[test]
fn s01_species_base_tab_cycles_species() {
    let (mut app, mut s) = map_with_sim();
    let first = WorldMap::default_species(&app);
    assert!(app.sim.as_ref().unwrap().creatures.living().any(|c| c.species == first), "defaults to a living species");
    app.overlay.base = Base::Species;
    app.overlay.species = first;
    // Tab walks every species in ALL order, wrapping; the sidebar stays put.
    let n = app.params.species.len();
    for i in 1..=n {
        s.handle_key(key(KeyCode::Tab), &mut app);
        assert_eq!(app.overlay.species, SpeciesId::from_index((first.index() + i) % n));
    }
    assert!(!s.wide, "Tab cycles species instead of toggling the sidebar");
    s.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE), &mut app);
    assert_eq!(app.overlay.species, SpeciesId::from_index((first.index() + n - 1) % n));
    // Arrows still scroll under the species base.
    app.viewport_size.set((110, 40));
    s.handle_key(key(KeyCode::Right), &mut app);
    assert_eq!(app.viewport_origin.0, 5);
    // Esc clears; the last species shown is remembered for the next time.
    s.handle_key(key(KeyCode::Esc), &mut app);
    assert!(app.overlay.is_empty());
    assert_eq!(app.overlay.species, SpeciesId::from_index((first.index() + n - 1) % n));
}

#[test]
fn s11_help_keys_column_fits() {
    use crate::ui::screens::s11_help::Help;
    let (app, _) = map_with_sim();
    let h = Help::new();
    let backend = TestBackend::new(155, 45);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| h.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap();
    let buf = terminal.backend().buffer();
    let all: String = (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect();
    assert!(all.contains("world generation"), "last key row is inside the modal");
    assert!(all.contains("Shift+Tab"));
}

#[test]
fn s11_help_scrolls_when_the_modal_is_short() {
    use crate::ui::screens::s11_help::Help;
    let (mut app, _) = map_with_sim();
    let mut h = Help::new();
    let text = |h: &Help, app: &AppState| -> String {
        let backend = TestBackend::new(155, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| h.render(app, f, Rect::new(0, 0, 155, 30))).unwrap();
        let buf = terminal.backend().buffer();
        (0..29).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    };
    let top = text(&h, &app);
    assert!(top.contains("Navigation"), "{top}");
    assert!(!top.contains("world generation"), "last key row must be hidden: {top}");
    assert!(top.contains(&format!("{}", crate::glyphs::DOWN)), "no ↓ indicator: {top}");
    h.handle_key(key(KeyCode::End), &mut app);
    let bottom = text(&h, &app);
    assert!(bottom.contains("world generation"), "{bottom}");
    assert!(bottom.contains(&format!("{}", crate::glyphs::UP)), "no ↑ indicator: {bottom}");
    assert!(!bottom.contains("Navigation"), "{bottom}");
    h.handle_key(key(KeyCode::Home), &mut app);
    assert!(text(&h, &app).contains("Navigation"));
}

#[test]
fn s01_species_overlay_renders_155x45() {
    let (mut app, mut s) = map_with_sim();
    app.overlay.base = Base::Species;
    app.overlay.species = SpeciesId(0);
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
}

#[test]
fn s01_region_updown_wraps_and_enter_centres() {
    let (mut app, mut s) = map_with_sim();
    app.viewport_size.set((110, 40));
    app.overlay.regions = true;
    let n = app.sim.as_ref().unwrap().world.regions.len();
    s.handle_key(key(KeyCode::Up), &mut app);
    assert_eq!(s.region_sel, n - 1);
    s.handle_key(key(KeyCode::Down), &mut app);
    assert_eq!(s.region_sel, 0);
    // Up/Down select rather than scroll under the region overlay.
    assert_eq!(app.viewport_origin, (0, 0));
    s.region_sel = n - 1;
    s.handle_key(key(KeyCode::Enter), &mut app);
    let (cx, cy) = app.sim.as_ref().unwrap().world.region_centre(n - 1);
    let (mx, my) = app.viewport_max();
    assert_eq!(app.viewport_origin, (cx.saturating_sub(55).min(mx), cy.saturating_sub(20).min(my)));
}

#[test]
fn s01_region_overlay_renders_155x45() {
    let (mut app, s) = map_with_sim();
    app.overlay.regions = true;
    let backend = TestBackend::new(155, 45);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| s.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap();
    let buf = terminal.backend().buffer();
    let row = |y: u16| -> String { (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect() };
    assert!(row(0).contains("overlay: regions"), "{}", row(0));
    let side: String = (0..45).map(row).collect::<Vec<_>>().join("\n");
    for r in &app.sim.as_ref().unwrap().world.regions {
        assert!(side.contains(&r.0), "region {:?} missing from the sidebar", r.0);
    }
    assert!(side.contains("Enter"));
}
