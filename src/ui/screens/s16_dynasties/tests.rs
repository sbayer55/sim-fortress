//! Tests for the S16 Top Dynasties screen.

use super::*;
use crate::glyphs;
use crate::sim::Params;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::Terminal;

fn key(s: &mut DynastiesScreen, app: &mut AppState, code: KeyCode) -> Action {
    s.handle_key(KeyEvent::new(code, KeyModifiers::NONE), app)
}

/// A world stepped `days` days, so the lines have members and kills.
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
fn draw(app: &AppState, s: &DynastiesScreen) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(155, 45)).unwrap();
    terminal.draw(|f| s.render(app, f, Rect::new(0, 1, 155, 44))).unwrap();
    let buf = terminal.backend().buffer();
    (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect()
}

fn selected_root(s: &DynastiesScreen, app: &AppState) -> Option<CreatureId> {
    let sim = app.sim.as_ref().unwrap();
    let ranked = s.ranked(sim);
    let ranked = ranked.as_ref().unwrap();
    let rows = s.rows(ranked);
    s.sel_index(ranked, &rows).and_then(|i| ranked.dynasties.get(i)).map(|d| d.root)
}

#[test]
fn regions_cycle_through_the_all_stop_and_the_cursor_wraps_by_identity() {
    let mut app = app_after(3);
    let mut s = DynastiesScreen::new();
    for _ in 0..8 {
        key(&mut s, &mut app, KeyCode::Right);
    }
    assert_eq!(s.region, ALL);
    key(&mut s, &mut app, KeyCode::Right);
    assert_eq!(s.region, 0);
    key(&mut s, &mut app, KeyCode::Left);
    assert_eq!(s.region, ALL, "the All stop sits after the eighth region");
    let first = selected_root(&s, &app).expect("a line is selected");
    key(&mut s, &mut app, KeyCode::Down);
    let second = selected_root(&s, &app).expect("a line is selected");
    assert_ne!(first, second);
    assert_eq!(s.sel_dynasty, Some(second), "the selection is stored as a root id");
    key(&mut s, &mut app, KeyCode::Up);
    assert_eq!(selected_root(&s, &app), Some(first));
    key(&mut s, &mut app, KeyCode::Up);
    let sim = app.sim.as_ref().unwrap();
    let n = s.rows(s.ranked(sim).as_ref().unwrap()).len();
    assert!(n >= 2, "the valley lists several lines");
    assert_ne!(selected_root(&s, &app), Some(first), "the cursor wraps to the last row");
}

#[test]
fn tab_moves_to_the_members_and_species_cycles_back_to_all() {
    let mut app = app_after(3);
    let mut s = DynastiesScreen::new();
    key(&mut s, &mut app, KeyCode::Left);
    assert!(matches!(key(&mut s, &mut app, KeyCode::Tab), Action::None));
    assert_eq!(s.focus, Focus::Members);
    draw(&app, &s);
    key(&mut s, &mut app, KeyCode::Down);
    assert!(s.sel_member.is_some(), "a member is selected by id");
    key(&mut s, &mut app, KeyCode::Tab);
    assert_eq!(s.focus, Focus::Dynasties);
    let predators = app.sim.as_ref().unwrap().roster().predator_ids().count();
    for _ in 0..predators {
        key(&mut s, &mut app, KeyCode::Char('s'));
        assert!(s.species.is_some());
    }
    key(&mut s, &mut app, KeyCode::Char('s'));
    assert_eq!(s.species, None);
    assert!(matches!(key(&mut s, &mut app, KeyCode::Esc), Action::Pop));
    assert!(matches!(key(&mut s, &mut app, KeyCode::Char('x')), Action::Unhandled));
}

#[test]
fn pins_hold_four_toggle_off_and_jump_back() {
    let mut app = app_after(3);
    let mut s = DynastiesScreen::new();
    key(&mut s, &mut app, KeyCode::Left);
    let first = selected_root(&s, &app).unwrap();
    key(&mut s, &mut app, KeyCode::Char('p'));
    assert_eq!(app.pins, vec![Pin::Dynasty(first)]);
    key(&mut s, &mut app, KeyCode::Char('p'));
    assert!(app.pins.is_empty(), "pinning again unpins");
    key(&mut s, &mut app, KeyCode::Char('p'));
    key(&mut s, &mut app, KeyCode::Tab);
    key(&mut s, &mut app, KeyCode::Char('p'));
    assert_eq!(app.pins.len(), 2);
    assert!(matches!(app.pins.get(1), Some(Pin::Member(_))));
    key(&mut s, &mut app, KeyCode::Tab);
    for _ in 0..3 {
        key(&mut s, &mut app, KeyCode::Down);
        key(&mut s, &mut app, KeyCode::Char('p'));
    }
    assert_eq!(app.pins.len(), MAX_PINS, "a fifth pin is ignored");
    // Leave, then jump back to the first pin: its region and line come back.
    key(&mut s, &mut app, KeyCode::Right);
    key(&mut s, &mut app, KeyCode::Down);
    key(&mut s, &mut app, KeyCode::Char('1'));
    assert_eq!(selected_root(&s, &app), Some(first));
    assert_eq!(s.focus, Focus::Dynasties);
    key(&mut s, &mut app, KeyCode::Char('2'));
    assert_eq!(s.focus, Focus::Members);
    assert!(matches!(key(&mut s, &mut app, KeyCode::Char('9')), Action::Unhandled));
}

#[test]
fn f_follows_the_subject_and_l_opens_its_lineage() {
    let mut app = app_after(3);
    let mut s = DynastiesScreen::new();
    key(&mut s, &mut app, KeyCode::Left);
    assert!(matches!(key(&mut s, &mut app, KeyCode::Char('f')), Action::Pop));
    let followed = app.follow.expect("the carrier is followed");
    assert!(app.sim.as_ref().unwrap().creatures.get(followed).is_some_and(|c| c.alive));
    assert!(matches!(key(&mut s, &mut app, KeyCode::Char('l')), Action::Push(_)));
}

#[test]
fn renders_every_panel_in_cp437_with_the_borders_in_place() {
    let app = app_after(3);
    let mut s = DynastiesScreen::new();
    s.region = ALL;
    let rows = draw(&app, &s);
    for (y, row) in rows.iter().enumerate() {
        for ch in row.chars() {
            assert!(glyphs::is_cp437(ch), "row {y}: {ch:?} is not CP437 in {row}");
        }
    }
    assert!(rows[1].starts_with("╔ Watch "));
    assert!(rows[4].starts_with("╔ Top dynasties by region "));
    assert!(rows[5].contains("◄ All regions ►"));
    assert!(rows.iter().any(|r| r.contains("Race since Y1")));
    let shown: usize = rows
        .iter()
        .find_map(|r| r.split("the top ").nth(1).and_then(|t| t.split(' ').next()).and_then(|n| n.parse().ok()))
        .expect("the members divider names how many rows show");
    assert!(shown <= 4, "the 44-row harness shows at most four member rows, not {shown}");
    assert!(rows[44].contains("[←→] region") && rows[44].contains("[Esc] back"));
    for (y, row) in rows.iter().enumerate().take(43).skip(5) {
        let r: Vec<char> = row.chars().collect();
        for x in [0, 111, 112, 154] {
            assert_eq!(r.get(x), Some(&'║'), "row {y} col {x}: {row}");
        }
    }
    for x in [0, 154] {
        assert_eq!(rows[2].chars().nth(x), Some('║'), "watch row col {x}");
    }
}

#[test]
fn a_fresh_world_and_an_empty_filter_draw_without_a_selection() {
    let app = app_after(0);
    let mut s = DynastiesScreen::new();
    let rows = draw(&app, &s);
    assert!(rows.iter().any(|r| r.contains("Top member")));
    // A region with no line of the filtered species shows the empty message.
    let empty_region = {
        let sim = app.sim.as_ref().unwrap();
        let ranked = s.ranked(sim);
        let ranked = ranked.as_ref().unwrap();
        (0..ALL).find(|&r| !ranked.dynasties.iter().any(|d| d.region == Some(crate::cast!(r => u8))))
    };
    if let Some(r) = empty_region {
        s.region = r;
        let rows = draw(&app, &s);
        assert!(rows.iter().any(|row| row.contains("no dynasties match the filter")));
        assert!(rows.iter().any(|row| row.contains("nothing selected")));
    }
}

#[test]
fn portraits_are_fifteen_cells_of_cp437_and_fall_back_by_kind() {
    for p in portraits::all() {
        for row in p {
            assert_eq!(row.chars().count(), usize::from(portraits::W));
            assert!(row.chars().all(glyphs::is_cp437));
        }
    }
    let app = app_after(0);
    let roster = app.sim.as_ref().unwrap().roster();
    let fox = roster.ids().find(|&id| roster.adult_glyph(id) == 'F').unwrap();
    let vole = roster.ids().find(|&id| roster.adult_glyph(id) == 'V').unwrap();
    assert!(std::ptr::eq(portraits::for_species(roster, fox), portraits::all()[0]));
    assert!(std::ptr::eq(portraits::for_species(roster, vole), portraits::all()[4]));
}

#[test]
fn helpers_rank_format_and_shorten() {
    assert_eq!(rank::rank_desc(&[9.0, 5.0, 5.0, 1.0], 5.0), (2, 4));
    assert_eq!(rank::rank_desc(&[], 5.0), (1, 0));
    assert_eq!(rank::top_pct(1, 33), 4);
    assert_eq!(rank::top_pct(33, 33), 100);
    assert_eq!(rank::top_pct(1, 0), 100);
    assert_eq!(region_short("Northern Taiga"), "N. Taiga");
    assert_eq!(region_short("Far Eastern Plains"), "F. E. Plains");
    assert_eq!(region_short("Fens"), "Fens");
    assert_eq!(region_short("All regions"), "Valley");
    assert_eq!(signed(0.07), "+.07");
    assert_eq!(signed(-0.05), "-.05");
    assert_eq!(age_str(401, 360), "1y 41d");
    assert_eq!(Stat::Age.fmt(730.0, 360), "2y");
    assert_eq!(Stat::Kills.fmt(12.4, 360), "12");
}

#[test]
fn n_opens_the_rename_modal_and_a_named_line_shows_its_name() {
    let mut app = app_after(3);
    let mut s = DynastiesScreen::new();
    draw(&app, &s);
    assert!(matches!(key(&mut s, &mut app, KeyCode::Char('n')), Action::Push(_)), "n names the focused line");
    let root = selected_root(&s, &app).expect("a line is selected");
    assert!(app.sim.as_mut().is_some_and(|sim| sim.rename_dynasty(root, "Grey Court")));
    let rows = draw(&app, &s);
    assert!(rows.iter().any(|r| r.contains("Grey Court")), "the rename rebuilds the cached ranking the same day");
    key(&mut s, &mut app, KeyCode::Tab);
    assert!(matches!(key(&mut s, &mut app, KeyCode::Char('n')), Action::Push(_)), "n names the focused member");
}

#[test]
fn founder_line_keeps_a_multi_word_name() {
    assert_eq!(founder_line("Ash w#003"), "Ash line");
    assert_eq!(founder_line("Old Grey w#003"), "Old Grey line");
    assert_eq!(name_and_tag("Old Grey w#003"), ("Old Grey", "w#003"));
    let app = app_after(1);
    let mut d = crate::sim::lineage::dynasties::rank(app.sim.as_ref().unwrap()).remove(0);
    d.name = Some("The Ash Court".into());
    assert_eq!(the_line(&d), "The Ash Court", "no doubled article");
    d.name = Some("Ash Court".into());
    assert_eq!(the_line(&d), "the Ash Court");
}
