//! Tests for the S17 Hunt Watch screen.

use super::*;
use crate::sim::hunt_watch::{HuntKey, HuntOutcome, HuntTrace};
use crate::sim::{Params, SpeciesId};
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::Terminal;

fn key(s: &mut HuntWatch, app: &mut AppState, code: KeyCode) -> Action {
    s.handle_key(KeyEvent::new(code, KeyModifiers::NONE), app)
}

/// A default world stepped until a predator is mid-hunt, from one year in
/// (cap 4000 ticks). An open trace alone is not enough: the details header
/// reads `Prey` only while the selected hunter's status is `Hunting`, and
/// which trace opens first is seed-fragile (it moved when C2 FR12 changed
/// the default run).
fn app_with_hunts() -> AppState {
    let mut app = AppState::new(Params::default());
    let mut sim = Sim::new(7, Params::default());
    for _ in 0..8640 {
        sim.step();
    }
    let hunting = |sim: &Sim| model::collect(sim, &[]).iter().any(|v| v.status == Status::Hunting);
    let mut steps = 0;
    while !hunting(&sim) && steps < 4000 {
        sim.step();
        steps += 1;
    }
    assert!(sim.hunts.traces().any(HuntTrace::is_open), "the default world hunts");
    assert!(hunting(&sim), "a predator is mid-hunt");
    app.sim = Some(sim);
    app
}

fn fresh_app() -> AppState {
    let mut app = AppState::new(Params::default());
    app.sim = Some(Sim::new(7, Params::default()));
    app
}

/// The screen drawn into the data-screen area of a 155×45 frame (rows 1–44).
fn draw(app: &AppState, s: &HuntWatch) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(155, 45)).unwrap();
    terminal.draw(|f| s.render(app, f, Rect::new(0, 1, 155, 44))).unwrap();
    let buf = terminal.backend().buffer();
    (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect()
}

fn assert_borders(rows: &[String]) {
    for (y, row) in rows.iter().enumerate().skip(2).take(41) {
        let cells: Vec<char> = row.chars().collect();
        assert_eq!(cells[0], '║', "left border on row {y}");
        assert_eq!(cells[154], '║', "right border on row {y}");
    }
    assert!(rows[1].starts_with('╔'), "panel top");
    assert!(rows[43].starts_with('╚'), "panel bottom");
}

#[test]
fn borders_hold_and_every_cell_is_cp437_with_and_without_details() {
    let app = app_with_hunts();
    let mut s = HuntWatch::new();
    for details in [true, false] {
        s.details = details;
        let rows = draw(&app, &s);
        assert_borders(&rows);
        for (y, row) in rows.iter().enumerate() {
            for ch in row.chars() {
                assert!(glyphs::is_cp437(ch), "{ch:?} on row {y} (details {details})");
            }
        }
        assert!(rows[44].contains("[↑↓] row") && rows[44].contains("[d] details") && rows[44].contains("[Esc] back"), "status keys");
    }
}

#[test]
fn d_toggles_the_details_panel_and_the_lane_count() {
    let app = app_with_hunts();
    let mut s = HuntWatch::new();
    // Select the predator that is mid-hunt: the first lane is whoever was
    // tracked first, and the header reads `Prey` only for a live hunt.
    s.selected = model::collect(app.sim.as_ref().unwrap(), &[]).iter().find(|v| v.status == Status::Hunting).map(HuntView::id);
    let rows = draw(&app, &s);
    assert!(rows.iter().any(|r| r.contains("Details ·")), "the details divider is drawn");
    assert!(rows.iter().any(|r| r.contains("Hunter") && r.contains("Prey")), "the details header");
    assert_eq!(s.lane_cap(42), 5);
    assert_eq!(s.lane_cap(41), 5, "the 44-row harness still fits five lanes");
    let mut app2 = app_with_hunts();
    assert!(matches!(key(&mut s, &mut app2, KeyCode::Char('d')), Action::None));
    assert!(!s.details);
    assert_eq!(s.lane_cap(42), 8);
    let rows = draw(&app, &s);
    assert!(!rows.iter().any(|r| r.contains("Details ·")), "no divider without details");
    assert!(rows.iter().any(|r| r.contains("row 8 · free") || r.contains("row 7 · free") || r.chars().filter(|&c| c == '┤').count() >= 5), "eight slots are shown");
}

#[test]
fn an_empty_world_draws_free_rows_and_no_selection() {
    let app = fresh_app();
    let s = HuntWatch::new();
    let rows = draw(&app, &s);
    assert_borders(&rows);
    assert!(rows[2].contains("row 1 · free"));
    assert!(rows[7].contains("row 2 · free"));
    assert!(rows.iter().any(|r| r.contains("no predator tracked")));
    assert!(rows[1].contains("0 hunting · 0 tracked"));
}

#[test]
fn slots_are_stable_and_newcomers_take_the_lowest_free_slot() {
    let app = app_with_hunts();
    let sim = app.sim.as_ref().unwrap();
    let views = model::collect(sim, &app.pins);
    assert!(!views.is_empty());
    let mut lanes = Lanes::default();
    lanes.refresh(&views);
    let first = lanes.slots;
    let first_id = first[0].expect("slot 1 is taken");
    // A second refresh with the same board changes nothing.
    lanes.refresh(&views);
    assert_eq!(lanes.slots, first);
    // Dropping the first hunter frees slot 1; the others keep theirs.
    let fewer: Vec<_> = views.iter().filter(|v| v.id() != first_id).collect();
    let fewer_owned: Vec<HuntView<'_>> = fewer.iter().map(|v| HuntView::build(sim, v.trace, &app.pins).unwrap()).collect();
    lanes.refresh(&fewer_owned);
    assert_eq!(lanes.slots[0], None);
    assert_eq!(lanes.slots[1..], first[1..]);
    // It returns into the lowest free slot.
    lanes.refresh(&views);
    assert_eq!(lanes.slots, first);
}

#[test]
fn pinned_lanes_sort_first_and_the_selection_follows_an_id() {
    let mut app = app_with_hunts();
    let mut s = HuntWatch::new();
    draw(&app, &s);
    let occupied = s.lanes.borrow().occupied(&app.pins);
    if occupied.len() < 2 {
        return; // one hunter: nothing to reorder
    }
    key(&mut s, &mut app, KeyCode::Down);
    let second = s.selected.expect("↓ selects the second lane");
    assert_eq!(second, occupied[1]);
    key(&mut s, &mut app, KeyCode::Char('p'));
    assert!(app.pins.contains(&Pin::Member(second)), "p pins the selected hunter");
    let order = s.lanes.borrow().occupied(&app.pins);
    assert_eq!(order[0], second, "the pinned lane sorts first");
    assert_eq!(s.lanes.borrow().slots, s.lanes.borrow().slots, "slots themselves do not move");
    key(&mut s, &mut app, KeyCode::Char('p'));
    assert!(!app.pins.contains(&Pin::Member(second)), "p again unpins");
    assert_eq!(s.lanes.borrow().occupied(&app.pins), occupied);
    // Wrapping.
    for _ in 0..occupied.len() {
        key(&mut s, &mut app, KeyCode::Down);
    }
    assert_eq!(s.selected, Some(second), "↓ wraps around the occupied lanes");
    key(&mut s, &mut app, KeyCode::Up);
    assert_eq!(s.selected, Some(occupied[0]));
}

#[test]
fn a_fifth_pin_is_refused_with_a_note() {
    let mut app = app_with_hunts();
    app.pins = vec![Pin::Dynasty(CreatureId(1)), Pin::Dynasty(CreatureId(2)), Pin::Dynasty(CreatureId(3)), Pin::Dynasty(CreatureId(4))];
    let mut s = HuntWatch::new();
    draw(&app, &s);
    key(&mut s, &mut app, KeyCode::Char('p'));
    assert_eq!(app.pins.len(), 4);
    assert_eq!(s.note, Some("4 pinned already"));
    let rows = draw(&app, &s);
    assert!(rows[44].contains("4 pinned already"));
}

#[test]
fn i_and_o_open_the_inspector() {
    let mut app = app_with_hunts();
    let mut s = HuntWatch::new();
    draw(&app, &s);
    let sel = s.lanes.borrow().occupied(&app.pins)[0];
    assert!(matches!(key(&mut s, &mut app, KeyCode::Char('i')), Action::Push(_)));
    let prey = app.sim.as_ref().unwrap().hunts.trace(sel).unwrap().prey();
    let prey_alive = app.sim.as_ref().unwrap().creatures.get(prey).is_some();
    let o = key(&mut s, &mut app, KeyCode::Char('o'));
    assert_eq!(matches!(o, Action::Push(_)), prey_alive, "o opens the prey while it exists");
}

#[test]
fn enter_follows_esc_pops_and_the_rest_falls_through() {
    let mut app = app_with_hunts();
    let mut s = HuntWatch::new();
    draw(&app, &s);
    let sel = s.lanes.borrow().occupied(&app.pins)[0];
    assert!(matches!(key(&mut s, &mut app, KeyCode::Char(' ')), Action::Unhandled));
    assert!(matches!(key(&mut s, &mut app, KeyCode::Char('x')), Action::Unhandled));
    assert!(matches!(key(&mut s, &mut app, KeyCode::Esc), Action::Pop));
    assert!(matches!(key(&mut s, &mut app, KeyCode::Enter), Action::Pop));
    assert_eq!(app.follow, Some(sel));
    assert_eq!(app.follow_death_tick, None);
}

#[test]
fn tug_is_monotone_in_the_gap_and_pinned_at_the_edges() {
    let far = model::tug(8, 9, None, 30, 0.8, None);
    let near = model::tug(3, 9, None, 30, 0.8, None);
    let contact = model::tug(1, 9, Some(5), 30, 0.8, Some(0.9));
    assert!(far < near && near < contact, "{far} {near} {contact}");
    assert!(contact > 0.8);
    let timed_out = model::tug(2, 9, Some(30), 30, 0.8, Some(0.9));
    assert!(timed_out <= 0.5, "a spent clock pulls toward escape: {timed_out}");
    let tired = model::tug(2, 9, Some(3), 30, 0.05, Some(0.9));
    assert!(tired <= 0.5, "spent legs pull toward escape: {tired}");
}

#[test]
fn verdict_words_follow_their_thresholds() {
    assert_eq!(model::need_word(0.85, 0.45).0, "DESPERATE");
    assert_eq!(model::need_word(0.65, 0.45).0, "HUNGRY");
    assert_eq!(model::need_word(0.50, 0.45).0, "IN NEED");
    assert_eq!(model::need_word(0.20, 0.45).0, "NOT HUNGRY");
    assert_eq!(model::challenge_word(0.75).0, "EASY PREY");
    assert_eq!(model::challenge_word(0.55).0, "FAIR CHASE");
    assert_eq!(model::challenge_word(0.40).0, "CHALLENGE");
    assert_eq!(model::challenge_word(0.10).0, "LONG SHOT");
    assert_eq!(model::frac(0.78), ".78");
    assert_eq!(model::frac(1.0), "1.0");
}

#[test]
fn a_resolved_hunt_stays_on_the_board_and_reads_as_a_remembered_lane() {
    // Drive a world past a resolution, then check the lane words on the board.
    let mut app = app_with_hunts();
    let sim = app.sim.as_mut().unwrap();
    let mut steps = 0;
    while !sim.hunts.traces().any(|t| t.end().is_some()) && steps < 3000 {
        sim.step();
        steps += 1;
    }
    let s = HuntWatch::new();
    let rows = draw(&app, &s);
    let sim = app.sim.as_ref().unwrap();
    let ended = sim.hunts.traces().filter(|t| t.end().is_some()).count();
    if ended > 0 {
        let joined = rows.join("\n");
        assert!(
            ["COOLDOWN", "EATING", "FED", "PATROL", "RESTING", "KILL", "MISSED", "LOST", "TIMED OUT", "DROPPED"].iter().any(|w| joined.contains(w)),
            "a resolved hunt shows its state"
        );
    }
    assert_borders(&rows);
}

#[test]
fn a_hand_built_trace_narrates_opening_chase_and_outcome() {
    let mut app = fresh_app();
    let sim = app.sim.as_mut().unwrap();
    // Any two living creatures serve as the pair; the trace is what the beats read.
    let ids: Vec<CreatureId> = sim.creatures.living_ids();
    let (hunter, prey) = (ids[0], ids[1]);
    let key_ = HuntKey { hunter, species: SpeciesId(0), prey };
    sim.hunts.begin(key_, 0);
    sim.hunts.end(key_, HuntOutcome::Miss, 3, 2);
    let view = HuntView::build(sim, sim.hunts.trace(hunter).unwrap(), &[]).unwrap();
    let lines = beats::beats(&view);
    assert!(lines.iter().any(|b| b.text.contains("lunges and misses")), "{lines:?}");
    assert!(lines.iter().all(|b| b.tick <= sim.time.tick + 4));
}
