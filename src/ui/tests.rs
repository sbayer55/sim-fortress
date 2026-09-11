//! C6 UI tests: title navigation, load-list ordering, confirm-modal keys and
//! options persistence.

use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::sim::params::UiParams;
use crate::sim::{save, Params, Sim};
use crate::ui::app::{AppState, ConfirmRequest, ConfirmYes};
use crate::ui::config::{load_ui_from, save_ui_to};
use crate::ui::screens::confirm::ConfirmModal;
use crate::ui::screens::s00_title::Title;
use crate::ui::screens::{Action, Screen};

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn tmpdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("simf-ui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn title_menu_navigation() {
    let dir = tmpdir("title");
    let mut app = AppState::new(Params::default());
    app.saves_dir = dir;
    let mut title = Title::new();
    assert_eq!(title.selection, 0);

    // With no saves, Down skips "Load World" (index 1).
    title.handle_key(key(KeyCode::Down), &mut app);
    assert_eq!(title.selection, 2, "Load World is skipped when there are no saves");
    title.handle_key(key(KeyCode::Down), &mut app);
    assert_eq!(title.selection, 3);
    title.handle_key(key(KeyCode::Up), &mut app);
    assert_eq!(title.selection, 2);
    title.handle_key(key(KeyCode::Up), &mut app);
    assert_eq!(title.selection, 0);

    // Enter on New World opens the world generation form.
    assert!(matches!(title.handle_key(key(KeyCode::Enter), &mut app), Action::Push(_)));

    // Enter on Options opens the controls modal.
    title.selection = 2;
    assert!(matches!(title.handle_key(key(KeyCode::Enter), &mut app), Action::Push(_)));

    // Quit with no world exits directly (no confirm).
    title.selection = 3;
    assert!(matches!(title.handle_key(key(KeyCode::Enter), &mut app), Action::Quit));
}

#[test]
fn load_list_sorted() {
    let dir = tmpdir("loadlist");
    let mut a = Sim::new(1, Params::default());
    for _ in 0..100 {
        a.step();
    }
    save::save(&a, "Alpha World", &dir).unwrap();
    let mut b = Sim::new(2, Params::default());
    for _ in 0..300 {
        b.step();
    }
    save::save(&b, "Beta World", &dir).unwrap();

    let saves = save::list_saves(&dir);
    assert_eq!(saves.len(), 2);
    assert_eq!(saves[0].header.world_name, "Beta World", "newest save first");
    assert_eq!(saves[1].header.world_name, "Alpha World");
}

#[test]
fn confirm_modal_keys() {
    let mut app = AppState::new(Params::default());
    let mut m = ConfirmModal::new();
    assert_eq!(m.focus, 0);

    // Esc = No: pops and clears the request.
    app.confirm = Some(ConfirmRequest { question: "really?".into(), yes: ConfirmYes::QuitApp });
    assert!(matches!(m.handle_key(key(KeyCode::Esc), &mut app), Action::Pop));
    assert!(app.confirm.is_none());

    // Yes (focus 0) on QuitApp quits.
    app.confirm = Some(ConfirmRequest { question: "really?".into(), yes: ConfirmYes::QuitApp });
    m.focus = 0;
    assert!(matches!(m.handle_key(key(KeyCode::Enter), &mut app), Action::Quit));

    // No (focus 1) pops without acting.
    app.confirm = Some(ConfirmRequest { question: "really?".into(), yes: ConfirmYes::QuitApp });
    m.focus = 1;
    assert!(matches!(m.handle_key(key(KeyCode::Enter), &mut app), Action::Pop));
    assert!(app.confirm.is_none());

    // Left/Right move the focus.
    app.confirm = Some(ConfirmRequest { question: "really?".into(), yes: ConfirmYes::QuitApp });
    m.focus = 0;
    m.handle_key(key(KeyCode::Right), &mut app);
    assert_eq!(m.focus, 1);
    m.handle_key(key(KeyCode::Left), &mut app);
    assert_eq!(m.focus, 0);
}

#[test]
fn options_persist() {
    let dir = tmpdir("options");
    let path = dir.join("ui.toml");
    let mut ui = UiParams::default();
    ui.autosave_days = 7;
    ui.day_night_tint = false;
    ui.log_births = true;
    ui.pause_on_follow_death = false;
    save_ui_to(&path, &ui).unwrap();
    assert_eq!(load_ui_from(&path).unwrap(), ui);
}
