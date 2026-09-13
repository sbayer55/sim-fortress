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
    let ui = UiParams {
        autosave_days: 7,
        day_night_tint: false,
        log_births: true,
        pause_on_follow_death: false,
        ..UiParams::default()
    };
    save_ui_to(&path, &ui).unwrap();
    assert_eq!(load_ui_from(&path).unwrap(), ui);
}

/// Regenerate the text renders of the live data screens, the way the C6 snapshot
/// set was produced: draw the screen over a deterministic `Sim` on a
/// `ratatui::backend::TestBackend` at the fixed 155x45 frame and write the buffer.
///
/// `cargo test --lib -- --ignored regenerate_screen_renders`
#[test]
#[ignore = "writes docs/screens/renders/*.txt; run explicitly to refresh the snapshots"]
fn regenerate_screen_renders() {
    use crate::ui::screens::s03_inspector::Inspector;
    use crate::ui::screens::s04_species::{SpeciesBrowser, SpeciesDetail};
    use crate::ui::screens::Screen;
    use crate::sim::{SpeciesId, Kind};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    // One deterministic year of a default world: populated, mid-drift.
    let mut sim = Sim::new(7, Params::default());
    for _ in 0..8640 {
        sim.step();
    }
    // S03a: the oldest living prey (the render is titled with its species).
    let prey = sim
        .creatures
        .living()
        .filter(|c| c.species.kind() == Kind::Prey)
        .min_by_key(|c| (c.born_day, c.id))
        .map(|c| (c.id, c.species))
        .expect("the default world keeps prey alive for a year");

    // S04b: a predator with living members if there is one, else the most numerous.
    let detail_species = SpeciesId::ALL
        .iter()
        .copied()
        .filter(|s| sim.species[s.index()].count > 0)
        .max_by_key(|s| (s.kind() == Kind::Predator, sim.species[s.index()].count))
        .unwrap_or(SpeciesId::Vole);

    let mut app = AppState::new(Params::default());
    app.sim = Some(sim);

    // The snapshot convention from docs/screens/README.md: the id/title on row 0,
    // the screen in a 155x44 frame below it (as the prototype binary did).
    let snap = |app: &AppState, screen: &dyn Screen, title: &str| -> String {
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                f.buffer_mut().set_stringn(0, 0, format!(" {title:<154}"), 155, ratatui::style::Style::default());
                screen.render(app, f, Rect::new(0, 1, 155, 44));
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..45 {
            for x in 0..155 {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    };

    let s03_title = format!("S03a  Creature Inspector - prey ({})", prey.1.name());
    let s04b_title = format!("S04b  Species Browser - species detail ({})", detail_species.name());
    let files = [
        ("docs/screens/renders/S03a.txt", s03_title.clone(), snap(&app, &Inspector::new(prey.0), &s03_title)),
        (
            "docs/screens/renders/S04a.txt",
            "S04a  Species Browser - species table".to_string(),
            snap(&app, &SpeciesBrowser::new(), "S04a  Species Browser - species table"),
        ),
        ("docs/screens/renders/S04b.txt", s04b_title.clone(), snap(&app, &SpeciesDetail::new(detail_species), &s04b_title)),
    ];
    for (path, title, text) in files {
        std::fs::write(path, text).unwrap_or_else(|e| panic!("write {path} ({title}): {e}"));
    }
}
