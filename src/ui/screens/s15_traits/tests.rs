//! Tests for the S15 Traits & Fates screen.

use super::*;
use crate::glyphs;
use crate::sim::Params;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::Terminal;

fn key(s: &mut TraitsScreen, app: &mut AppState, code: KeyCode) -> Action {
    s.handle_key(KeyEvent::new(code, KeyModifiers::NONE), app)
}

/// A world stepped `days` days, so there are lives and deaths to show.
fn app_after(days: u64) -> AppState {
    let mut app = AppState::new(Params::default());
    let mut sim = Sim::new(7, Params::default());
    for _ in 0..days * 24 {
        sim.step();
    }
    app.sim = Some(sim);
    app
}

/// The screen drawn into the data-screen area of a 155×45 frame (rows 1–44).
fn draw(app: &AppState, s: &TraitsScreen) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(155, 45)).unwrap();
    terminal.draw(|f| s.render(app, f, Rect::new(0, 1, 155, 44))).unwrap();
    let buf = terminal.backend().buffer();
    (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect()
}

#[test]
fn arrows_move_the_cursor_and_wrap() {
    let mut app = AppState::new(Params::default());
    app.sim = Some(Sim::new(7, Params::default()));
    let mut s = TraitsScreen::new();
    assert!(matches!(key(&mut s, &mut app, KeyCode::Up), Action::None));
    assert_eq!(s.trait_ix, N_TRAITS - 1);
    key(&mut s, &mut app, KeyCode::Down);
    assert_eq!(s.trait_ix, 0);
    key(&mut s, &mut app, KeyCode::Left);
    assert_eq!(s.outcome, N_OUTCOMES - 1);
    key(&mut s, &mut app, KeyCode::Right);
    key(&mut s, &mut app, KeyCode::Right);
    assert_eq!(s.outcome, 1);
}

#[test]
fn digits_pick_species_in_the_roster_and_c_cycles_the_days() {
    let mut app = AppState::new(Params::default());
    app.sim = Some(Sim::new(7, Params::default()));
    let n = app.sim.as_ref().map_or(0, |s| s.roster().len());
    let mut s = TraitsScreen::new();
    key(&mut s, &mut app, KeyCode::Char('3'));
    assert_eq!(s.species, 2);
    if n < 9 {
        key(&mut s, &mut app, KeyCode::Char('9'));
        assert_eq!(s.species, 2, "a digit past the roster does nothing");
    }
    key(&mut s, &mut app, KeyCode::Char('c'));
    assert_eq!(s.crowding, Crowding::Crowded);
    key(&mut s, &mut app, KeyCode::Char('c'));
    key(&mut s, &mut app, KeyCode::Char('c'));
    assert_eq!(s.crowding, Crowding::All);
    assert!(matches!(key(&mut s, &mut app, KeyCode::Esc), Action::Pop));
    assert!(matches!(key(&mut s, &mut app, KeyCode::Char('x')), Action::Unhandled));
}

#[test]
fn renders_the_matrix_sidebar_and_status_in_cp437() {
    let app = app_after(120);
    let mut s = TraitsScreen::new();
    s.outcome = 6;
    let rows = draw(&app, &s);
    let all: String = rows.concat();
    for c in all.chars() {
        assert!(glyphs::is_cp437(c), "{c:?} is not CP437");
    }
    let row = |y: usize| rows.get(y).cloned().unwrap_or_default();
    assert!(row(1).contains("Traits and outcomes: Voles"), "{}", row(1));
    assert!(row(1).contains("Selected"));
    assert!(row(4).contains("Speed") || row(5).contains("Speed"), "{}", row(5));
    assert!(row(4).contains("Preyed"), "{}", row(4));
    assert!(rows.iter().any(|r| r.contains("Speed across ages")));
    assert!(rows.iter().any(|r| r.contains("By trait third")));
    assert!(rows.iter().any(|r| r.contains("Strongest links")));
    assert!(row(44).contains("[c] crowding"), "{}", row(44));
    assert_borders(&rows);
}

/// Both panels keep their borders on every body row.
fn assert_borders(rows: &[String]) {
    for (y, row) in rows.iter().enumerate().take(43).skip(2) {
        let r: Vec<char> = row.chars().collect();
        for x in [0, 111, 112, 154] {
            assert_eq!(r.get(x), Some(&'║'), "row {y} col {x}: {row}");
        }
    }
}

/// Every species and every filter draws without breaking a border, including
/// the ones with too few lives to show a cell.
#[test]
fn every_species_and_filter_draws() {
    let mut app = app_after(60);
    let n = app.sim.as_ref().map_or(0, |s| s.roster().len());
    let mut s = TraitsScreen::new();
    for sp in 0..n.min(9) {
        let d = char::from_digit(crate::cast!(sp + 1 => u32), 10).unwrap_or('1');
        key(&mut s, &mut app, KeyCode::Char(d));
        for _ in 0..3 {
            key(&mut s, &mut app, KeyCode::Char('c'));
            let rows = draw(&app, &s);
            let r: Vec<char> = rows.get(20).map(|r| r.chars().collect()).unwrap_or_default();
            assert_eq!(r.get(111), Some(&'║'), "species {sp}");
            assert_eq!(r.get(154), Some(&'║'), "species {sp}");
        }
    }
}

#[test]
fn a_fresh_world_with_no_deaths_draws() {
    let mut app = AppState::new(Params::default());
    app.sim = Some(Sim::new(7, Params::default()));
    let rows = draw(&app, &TraitsScreen::new());
    assert!(rows.iter().any(|r| r.contains("n/a")));
    assert!(rows.iter().any(|r| r.contains("no deaths in the window")));
}

#[test]
fn signed_and_frac_formats() {
    assert_eq!(signed(0.42), "+.42");
    assert_eq!(signed(-0.05), "-.05");
    assert_eq!(signed(1.0), "+1.0");
    assert_eq!(frac(0.555), ".56");
    assert_eq!(days(85.0), "85d");
    assert_eq!(days(1095.0), "3.0y");
}

/// Lines of the sidebar (screen columns 113–154).
fn sidebar(rows: &[String]) -> Vec<String> {
    rows.iter().map(|r| r.chars().skip(113).take(41).collect::<String>().trim().to_string()).collect()
}

#[test]
fn too_few_lives_wins_over_flat_and_flat_is_worded() {
    let mut app = AppState::new(Params::default());
    app.sim = Some(Sim::new(7, Params::default()));
    // Day 1: no deaths, so the lifespan cell has n = 0 (and is trivially flat).
    let s = TraitsScreen::new();
    let side = sidebar(&draw(&app, &s));
    assert!(side.iter().any(|l| l == "too few lives to say (under 30)"), "{side:#?}");
    assert!(side.iter().any(|l| l == "from 0 deaths"), "{side:#?}");
    assert!(side.iter().any(|l| l.contains("Too few vole lives")), "{side:#?}");
    // Young per year: every adult founder has 0 young on day 1, a flat column.
    let mut s = TraitsScreen::new();
    key(&mut s, &mut app, KeyCode::Right);
    let side = sidebar(&draw(&app, &s));
    assert!(side.iter().any(|l| l == "none: all lives had 0.0 young per year"), "{side:#?}");
}

/// The old-age column takes no side: its cells keep the panel ground and the
/// sidebar calls it neutral rather than helps or hurts.
#[test]
fn old_age_cells_are_neutral() {
    let app = app_after(120);
    let mut s = TraitsScreen::for_species(2);
    s.outcome = 5;
    let mut terminal = Terminal::new(TestBackend::new(155, 45)).unwrap();
    terminal.draw(|f| s.render(&app, f, Rect::new(0, 1, 155, 44))).unwrap();
    let buf = terminal.backend().buffer();
    // Inner x 1 + X0 31 + 5 × 8 = column 72; trait rows start at screen row 5.
    for t in 0..12u16 {
        let bg = buf[(74, 5 + 2 * t)].bg;
        assert_eq!(bg, theme::PANEL_BG, "trait {t}: the old-age cell is coloured");
    }
    let rows: Vec<String> = (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect();
    let side = sidebar(&rows);
    assert!(!side.iter().any(|l| l.contains("the trait helps") || l.contains("the trait hurts")), "{side:#?}");
}

#[test]
fn a_whole_band_of_one_cause_reads_100() {
    let app = app_after(300);
    let rows = draw(&app, &TraitsScreen::new());
    let died = rows.iter().find(|r| r.contains("died of ")).cloned().unwrap_or_default();
    assert!(!died.contains("99 "), "a 100% band must not be capped at 99: {died}");
}
