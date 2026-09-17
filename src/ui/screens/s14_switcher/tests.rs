//! Tests for the S14 overlay switcher.

use super::rows::{rows, Tab};
use super::{Focus, OverlaySwitcher};
use crate::sim::{Kind, Params, Sim, SpeciesId};
use crate::ui::app::AppState;
use crate::ui::screens::s01_map::WorldMap;
use crate::ui::screens::{render_stack, Action, Screen, Stack};
use crate::widgets::map::{Base, Disease, Layer, OverlayStack};
use crate::{glyphs, theme};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn fresh() -> AppState {
    let mut app = AppState::new(Params::default());
    app.sim = Some(Sim::new(7, Params::default()));
    app
}

/// The map and the switcher over it, as the live stack draws them.
fn draw(app: &AppState, switcher: OverlaySwitcher) -> Buffer {
    let stack = Stack { screens: vec![Box::new(WorldMap::new("Test".into())), Box::new(switcher)] };
    let mut terminal = Terminal::new(TestBackend::new(155, 45)).unwrap();
    terminal.draw(|f| render_stack(&stack, app, f, f.area())).unwrap();
    terminal.backend().buffer().clone()
}

fn row_text(buf: &Buffer, y: u16, x0: u16, x1: u16) -> String {
    (x0..x1).map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' ')).collect()
}

#[test]
fn opens_on_base_tab_first_row() {
    let mut app = fresh();
    let s = OverlaySwitcher::open(&mut app);
    assert_eq!((s.tab, s.cur, s.focus), (Tab::Base, 0, Focus::Rows));
    // Opening refreshes the off sub-picks by the default rules.
    assert!(app.sim.as_ref().unwrap().creatures.living().any(|c| c.species == app.overlay.species));
    let subject = app.overlay.sense_subject.expect("a predator lives");
    assert_eq!(app.sim.as_ref().unwrap().roster().kind(app.sim.as_ref().unwrap().creatures.get(subject).unwrap().species), Kind::Predator);
    let base: Vec<Layer> = rows(Tab::Base, &app).iter().map(|r| r.layer).collect();
    assert_eq!(base, Base::ALL.map(Layer::Base).to_vec());
    assert_eq!(rows(Tab::Marks, &app).iter().map(|r| r.layer).collect::<Vec<_>>(), Layer::MARKS.to_vec());
}

#[test]
fn tab_flips_tabs() {
    let mut app = fresh();
    let mut s = OverlaySwitcher::open(&mut app);
    s.handle_key(key(KeyCode::Down), &mut app);
    assert!(matches!(s.handle_key(key(KeyCode::Tab), &mut app), Action::None));
    assert_eq!((s.tab, s.cur), (Tab::Marks, 0));
    s.handle_key(key(KeyCode::BackTab), &mut app);
    assert_eq!(s.tab, Tab::Base);
    // Up wraps to the last row, Down back to the first.
    s.handle_key(key(KeyCode::Up), &mut app);
    assert_eq!(s.cur, 5);
    s.handle_key(key(KeyCode::Down), &mut app);
    assert_eq!(s.cur, 0);
}

#[test]
fn space_selects_base() {
    let mut app = fresh();
    let mut s = OverlaySwitcher::open(&mut app);
    for _ in 0..3 {
        s.handle_key(key(KeyCode::Down), &mut app);
    }
    s.handle_key(key(KeyCode::Char(' ')), &mut app);
    assert_eq!(app.overlay.base, Base::Moisture);
    s.handle_key(key(KeyCode::Down), &mut app);
    s.handle_key(key(KeyCode::Char(' ')), &mut app);
    assert_eq!(app.overlay.base, Base::Species, "one base at a time");
    s.handle_key(key(KeyCode::Up), &mut app);
    s.handle_key(key(KeyCode::Up), &mut app);
    s.handle_key(key(KeyCode::Up), &mut app);
    s.handle_key(key(KeyCode::Up), &mut app);
    s.handle_key(key(KeyCode::Char(' ')), &mut app);
    assert_eq!(app.overlay.base, Base::None);
}

#[test]
fn space_toggles_mark() {
    let mut app = fresh();
    let mut s = OverlaySwitcher::open(&mut app);
    s.handle_key(key(KeyCode::Tab), &mut app);
    s.handle_key(key(KeyCode::Down), &mut app);
    s.handle_key(key(KeyCode::Char(' ')), &mut app);
    assert!(app.overlay.regions);
    s.handle_key(key(KeyCode::Down), &mut app);
    s.handle_key(key(KeyCode::Char(' ')), &mut app);
    assert!(app.overlay.regions && app.overlay.health, "marks accumulate");
    s.handle_key(key(KeyCode::Down), &mut app);
    s.handle_key(key(KeyCode::Char(' ')), &mut app);
    assert_eq!(app.overlay.disease, Disease::On(None));
    s.handle_key(key(KeyCode::Char(' ')), &mut app);
    assert_eq!(app.overlay.disease, Disease::Off);
    s.handle_key(key(KeyCode::Up), &mut app);
    s.handle_key(key(KeyCode::Up), &mut app);
    s.handle_key(key(KeyCode::Char(' ')), &mut app);
    assert!(!app.overlay.regions);
    // Sense on the first row: on with the remembered subject.
    s.handle_key(key(KeyCode::Up), &mut app);
    s.handle_key(key(KeyCode::Char(' ')), &mut app);
    assert!(app.overlay.sense && app.overlay.sense_subject.is_some());
}

#[test]
fn right_enters_list_enter_picks_and_enables() {
    let mut app = fresh();
    let mut s = OverlaySwitcher::open(&mut app);
    for _ in 0..4 {
        s.handle_key(key(KeyCode::Down), &mut app);
    }
    let remembered = app.overlay.species;
    s.handle_key(key(KeyCode::Right), &mut app);
    assert_eq!(s.focus, Focus::List);
    assert_eq!(s.list_cur, remembered.index(), "the cursor starts on the remembered entry");
    s.handle_key(key(KeyCode::Down), &mut app);
    assert!(matches!(s.handle_key(key(KeyCode::Enter), &mut app), Action::None), "Enter in the list does not close");
    assert_eq!(app.overlay.species, SpeciesId::from_index((remembered.index() + 1) % app.params.species.len()));
    assert_eq!(app.overlay.base, Base::Species, "picking turns the layer on");
    assert_eq!(s.focus, Focus::Rows);
    // The pathogen list: All, then each live slot; Space picks like Enter.
    s.handle_key(key(KeyCode::Tab), &mut app);
    for _ in 0..3 {
        s.handle_key(key(KeyCode::Down), &mut app);
    }
    s.handle_key(key(KeyCode::Right), &mut app);
    assert_eq!(s.list_cur, 0, "All pathogens is remembered at first");
    s.handle_key(key(KeyCode::Down), &mut app);
    s.handle_key(key(KeyCode::Char(' ')), &mut app);
    assert_eq!(app.overlay.disease, Disease::On(Some(crate::sim::PathogenId(0))));
    // Right on a row without a sub-pick does nothing.
    s.handle_key(key(KeyCode::Up), &mut app);
    s.handle_key(key(KeyCode::Right), &mut app);
    assert_eq!(s.focus, Focus::Rows);
}

#[test]
fn left_returns_without_change() {
    let mut app = fresh();
    let mut s = OverlaySwitcher::open(&mut app);
    for _ in 0..4 {
        s.handle_key(key(KeyCode::Down), &mut app);
    }
    let before = app.overlay;
    s.handle_key(key(KeyCode::Right), &mut app);
    s.handle_key(key(KeyCode::Down), &mut app);
    s.handle_key(key(KeyCode::Down), &mut app);
    s.handle_key(key(KeyCode::Left), &mut app);
    assert_eq!(s.focus, Focus::Rows);
    assert_eq!(app.overlay, before);
    // Tab from the list flips tabs and returns focus to the rows.
    s.handle_key(key(KeyCode::Right), &mut app);
    s.handle_key(key(KeyCode::Tab), &mut app);
    assert_eq!((s.tab, s.focus, s.cur), (Tab::Marks, Focus::Rows, 0));
}

#[test]
fn backspace_clears_stack_and_stays_open() {
    let mut app = fresh();
    app.overlay = OverlayStack { base: Base::Species, species: SpeciesId(2), regions: true, health: true, ..OverlayStack::PLAIN };
    app.overlay.show_disease(Some(crate::sim::PathogenId(1)));
    let mut s = OverlaySwitcher::open(&mut app);
    assert!(matches!(s.handle_key(key(KeyCode::Backspace), &mut app), Action::None));
    assert!(app.overlay.is_empty());
    assert_eq!((app.overlay.species, app.overlay.pathogen), (SpeciesId(2), Some(crate::sim::PathogenId(1))), "sub-picks survive");
}

#[test]
fn enter_closes_and_keeps_stack() {
    let mut app = fresh();
    let mut s = OverlaySwitcher::open(&mut app);
    s.handle_key(key(KeyCode::Down), &mut app);
    s.handle_key(key(KeyCode::Char(' ')), &mut app);
    assert!(matches!(s.handle_key(key(KeyCode::Enter), &mut app), Action::Pop));
    assert_eq!(app.overlay.base, Base::Vegetation);
    assert!(matches!(OverlaySwitcher::new().handle_key(key(KeyCode::Esc), &mut app), Action::Pop));
    assert!(matches!(OverlaySwitcher::new().handle_key(key(KeyCode::Char('o')), &mut app), Action::Pop));
    assert_eq!(app.overlay.base, Base::Vegetation);
}

#[test]
fn digits_are_unhandled() {
    let mut app = fresh();
    let mut s = OverlaySwitcher::open(&mut app);
    for c in '0'..='9' {
        assert!(matches!(s.handle_key(key(KeyCode::Char(c)), &mut app), Action::Unhandled), "{c}");
    }
    s.handle_key(key(KeyCode::Right), &mut app);
    assert!(matches!(s.handle_key(key(KeyCode::Char('?')), &mut app), Action::Unhandled), "global keys pass through");
    assert_eq!(app.overlay, OverlayStack { species: app.overlay.species, sense_subject: app.overlay.sense_subject, ..OverlayStack::PLAIN });
}

#[test]
fn sense_row_disabled_without_predators() {
    let mut app = fresh();
    {
        let sim = app.sim.as_mut().unwrap();
        let predators: Vec<_> = sim.creatures.living().filter(|c| sim.roster().kind(c.species) == Kind::Predator).map(|c| c.id).collect();
        for id in predators {
            sim.creatures.get_mut(id).unwrap().alive = false;
        }
    }
    let mut s = OverlaySwitcher::open(&mut app);
    assert_eq!(app.overlay.sense_subject, None);
    let marks = rows(Tab::Marks, &app);
    assert!(marks[0].disabled);
    assert_eq!(marks[0].value.as_deref(), Some("no predators"));
    s.handle_key(key(KeyCode::Tab), &mut app);
    s.handle_key(key(KeyCode::Char(' ')), &mut app);
    assert!(!app.overlay.sense, "Space does nothing on the disabled row");
    s.handle_key(key(KeyCode::Right), &mut app);
    assert_eq!(s.focus, Focus::Rows, "→ does nothing on the disabled row");
    let buf = draw(&app, s);
    let side: String = (11..26).map(|y| row_text(&buf, y, 15, 49) + "\n").collect();
    assert!(side.contains("no predators"), "{side}");
}

#[test]
fn dead_sense_subject_turns_mark_off() {
    let mut app = fresh();
    let mut s = OverlaySwitcher::open(&mut app);
    s.handle_key(key(KeyCode::Tab), &mut app);
    s.handle_key(key(KeyCode::Char(' ')), &mut app);
    let id = app.overlay.sense_subject.expect("on");
    app.sim.as_mut().unwrap().creatures.get_mut(id).unwrap().alive = false;
    s.handle_key(key(KeyCode::Down), &mut app);
    assert!(!app.overlay.sense);
    assert_eq!(app.overlay.sense_subject, None);
    // Turning it on again picks a living predator.
    s.handle_key(key(KeyCode::Up), &mut app);
    s.handle_key(key(KeyCode::Char(' ')), &mut app);
    assert!(app.overlay.sense);
    assert_ne!(app.overlay.sense_subject, Some(id));
}

/// Moisture as the base, the cursor on Species and its list focused (S14c).
fn s14c() -> (AppState, OverlaySwitcher) {
    let mut app = fresh();
    app.overlay.base = Base::Moisture;
    let mut s = OverlaySwitcher::open(&mut app);
    for _ in 0..4 {
        s.handle_key(key(KeyCode::Down), &mut app);
    }
    s.handle_key(key(KeyCode::Right), &mut app);
    (app, s)
}

#[test]
fn render_matches_geometry() {
    let (app, s) = s14c();
    let buf = draw(&app, s);
    let at = |x: u16, y: u16| buf[(x, y)].symbol().to_string();
    assert_eq!((at(14, 10), at(97, 10), at(14, 30), at(97, 30)), ("╔".into(), "╗".into(), "╚".into(), "╝".into()));
    let top = row_text(&buf, 10, 14, 98);
    assert!(top.contains("Overlay"), "{top}");
    assert!(top.contains("o or Esc closes"), "{top}");
    assert_eq!(at(49, 10), glyphs::T_DOWN.to_string());
    assert!((11..26).all(|y| at(49, y) == glyphs::V_LINE.to_string()), "the divider spans rows 11-25");
    assert_eq!(at(49, 26), glyphs::T_UP.to_string());
    assert!(row_text(&buf, 27, 14, 98).contains("Showing"));
    assert!(row_text(&buf, 28, 14, 98).contains("moisture"), "{}", row_text(&buf, 28, 14, 98));
    assert!(row_text(&buf, 29, 14, 98).contains("Backspace clears everything"));
    assert!(row_text(&buf, 44, 0, 155).contains("[Bksp] clear all"));
}

#[test]
fn render_columns_of_the_base_tab() {
    let (app, s) = s14c();
    let buf = draw(&app, s);
    let left = |y: u16| row_text(&buf, y, 15, 49);
    let strip = left(11);
    assert!(strip.contains("Base heatmap") && strip.contains("Marks"), "{strip}");
    assert!(strip.contains("Tab"), "{strip}");
    assert!(left(12).contains("pick one"), "{}", left(12));
    assert!(left(13).contains("( ) None"), "{}", left(13));
    assert!(left(16).contains("(•) Moisture"), "{}", left(16));
    assert!(left(17).contains("( ) Species"), "{}", left(17));
    assert!(left(17).contains(glyphs::CUE), "{}", left(17));
    assert_eq!(buf[(49 + 2, 11)].fg, theme::ACCENT, "the list header is accent");
    assert!(row_text(&buf, 11, 51, 97).contains("species · which species"));
    assert!(row_text(&buf, 12, 51, 97).contains("alive"), "{}", row_text(&buf, 12, 51, 97));
    assert_eq!(buf[(51, 12)].symbol(), glyphs::PLAY.to_string(), "the list cursor marker");
}

#[test]
fn render_marks_tab_description_column() {
    // The Marks tab, with Regions on and the cursor on Regions: a description column.
    let mut app = fresh();
    app.overlay.regions = true;
    let mut s = OverlaySwitcher::open(&mut app);
    s.handle_key(key(KeyCode::Tab), &mut app);
    s.handle_key(key(KeyCode::Down), &mut app);
    let buf = draw(&app, s);
    assert!(row_text(&buf, 12, 15, 49).contains("any number"));
    assert!(row_text(&buf, 14, 15, 49).contains("[x] Regions"), "{}", row_text(&buf, 14, 15, 49));
    assert!(row_text(&buf, 11, 51, 97).contains("regions  · mark"));
    assert!(row_text(&buf, 12, 51, 97).contains("named regions, tinted with labels"));
    assert!(row_text(&buf, 23, 51, 97).contains("no sub-pick for this layer"), "{}", row_text(&buf, 23, 51, 97));
    assert!(row_text(&buf, 24, 51, 97).contains("Space toggles it"));
}

#[test]
fn backdrop_is_not_dimmed() {
    let app = fresh();
    let plain = {
        let map = WorldMap::new("Test".into());
        let mut terminal = Terminal::new(TestBackend::new(155, 45)).unwrap();
        terminal.draw(|f| map.render(&app, f, f.area())).unwrap();
        terminal.backend().buffer().clone()
    };
    let over = draw(&app, OverlaySwitcher::new());
    // A map cell outside the box and a sidebar cell keep their colours.
    for (x, y) in [(1u16, 1u16), (5, 40), (120, 5)] {
        assert_eq!(over[(x, y)].fg, plain[(x, y)].fg, "fg at {x},{y}");
        assert_eq!(over[(x, y)].bg, plain[(x, y)].bg, "bg at {x},{y}");
    }
    assert_ne!(over[(14, 10)].symbol(), plain[(14, 10)].symbol(), "the box is drawn");
    assert!(row_text(&over, 28, 14, 98).contains("nothing, plain terrain"));
}
