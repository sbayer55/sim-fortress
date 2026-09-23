//! The rename modal: typing, keeping, clearing, cancelling, and that it
//! swallows the global keys while it is up.

use super::*;
use crate::sim::{Kind, Params};
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::Terminal;

fn press(m: &mut RenameModal, app: &mut AppState, code: KeyCode) -> Action {
    m.handle_key(KeyEvent::new(code, KeyModifiers::NONE), app)
}

fn type_text(m: &mut RenameModal, app: &mut AppState, text: &str) {
    for c in text.chars() {
        assert!(matches!(press(m, app, KeyCode::Char(c)), Action::None), "{c:?} is typed, not passed on");
    }
}

/// A fresh world and one living predator founder in it.
fn app_and_predator() -> (AppState, CreatureId) {
    let mut app = AppState::new(Params::default());
    let sim = Sim::new(7, Params::default());
    let id = sim.creatures.living().find(|c| sim.roster().kind(c.species) == Kind::Predator).map_or(CreatureId(0), |c| c.id);
    app.sim = Some(sim);
    (app, id)
}

fn sim(app: &AppState) -> &Sim {
    app.sim.as_ref().expect("a world")
}

fn name_of(app: &AppState, id: CreatureId) -> String {
    sim(app).creatures.get(id).map(|c| c.name_str(sim(app).roster()).to_string()).unwrap_or_default()
}

#[test]
fn typing_then_enter_names_the_animal_and_space_and_p_do_not_escape() {
    let (mut app, id) = app_and_predator();
    let mut m = RenameModal::new(RenameTarget::Animal(id), sim(&app));
    assert_eq!(m.text, "", "an unnamed animal starts with an empty field");
    // `type_text` checks each key comes back `Action::None`, so the global
    // table (space pauses, `p` pins) never sees it.
    type_text(&mut m, &mut app, "Old pGrey");
    assert!(matches!(press(&mut m, &mut app, KeyCode::Enter), Action::Pop));
    assert_eq!(name_of(&app, id), "Old pGrey");
}

#[test]
fn esc_leaves_the_name_alone() {
    let (mut app, id) = app_and_predator();
    let before = name_of(&app, id);
    let mut m = RenameModal::new(RenameTarget::Animal(id), sim(&app));
    type_text(&mut m, &mut app, "Fang");
    assert!(matches!(press(&mut m, &mut app, KeyCode::Esc), Action::Pop));
    assert_eq!(name_of(&app, id), before);
}

#[test]
fn a_named_animal_prefills_and_an_emptied_field_restores_the_pool_name() {
    let (mut app, id) = app_and_predator();
    let pool = name_of(&app, id);
    assert!(app.sim.as_mut().is_some_and(|s| s.rename_creature(id, "Fang")));
    let mut m = RenameModal::new(RenameTarget::Animal(id), sim(&app));
    assert_eq!(m.text, "Fang");
    press(&mut m, &mut app, KeyCode::Delete);
    assert_eq!(m.text, "");
    press(&mut m, &mut app, KeyCode::Enter);
    assert_eq!(name_of(&app, id), pool);
}

#[test]
fn only_ascii_is_typed_and_the_field_stops_at_the_limit() {
    let (mut app, id) = app_and_predator();
    let mut m = RenameModal::new(RenameTarget::Animal(id), sim(&app));
    type_text(&mut m, &mut app, "\u{e9}Abcdefghijklmnop");
    assert_eq!(m.text, "Abcdefghijkl", "é dropped, twelve kept");
    press(&mut m, &mut app, KeyCode::Backspace);
    assert_eq!(m.text, "Abcdefghijk");
}

#[test]
fn a_dynasty_is_named_and_the_modal_draws_its_field() {
    let (mut app, id) = app_and_predator();
    let mut m = RenameModal::new(RenameTarget::Dynasty(id), sim(&app));
    type_text(&mut m, &mut app, "Grey Court");
    let mut terminal = Terminal::new(TestBackend::new(155, 45)).unwrap_or_else(|e| panic!("{e}"));
    terminal.draw(|f| m.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap_or_else(|e| panic!("{e}"));
    let buf = terminal.backend().buffer();
    let screen: Vec<String> = (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect();
    assert!(screen.iter().any(|r| r.contains("Name this dynasty")));
    assert!(screen.iter().any(|r| r.contains("Grey Court_")));
    assert!(screen.iter().any(|r| r.contains("Empty restores") && r.contains(" line")));
    press(&mut m, &mut app, KeyCode::Enter);
    assert_eq!(sim(&app).lineage.dynasties().get(id).and_then(|d| d.name.as_deref()), Some("Grey Court"));
}
