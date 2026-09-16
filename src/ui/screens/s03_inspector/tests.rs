//! Tests for the S03 creature inspector.

use super::Inspector;
use ratatui::layout::Rect;
use crate::sim::disease::{PathogenId, Stage};
use crate::ui::app::AppState;
use crate::ui::screens::Screen;
use crate::glyphs;
use crate::sim::disease::Infection;
use crate::sim::{Params, Sim};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn screen_text(app: &AppState, screen: &dyn Screen) -> String {
    let backend = TestBackend::new(155, 45);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| screen.render(app, f, Rect::new(0, 0, 155, 45))).unwrap();
    let buf = terminal.backend().buffer();
    (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
}

#[test]
fn s03_sickness_rows() {
    let mut app = AppState::new(Params::default());
    let mut sim = Sim::new(7, Params::default());
    let id = sim.creatures.living_ids()[0];
    sim.creatures.get_mut(id).unwrap().infection =
        Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 0, ends_day: 9, severity: 0.8, source: None, outbreak: 0 });
    sim.creatures.get_mut(id).unwrap().immune_until[1] = u32::MAX;
    app.sim = Some(sim);
    let text = screen_text(&app, &Inspector::new(id));
    assert!(text.contains(&format!("{} ", glyphs::DISEASE)), "sickness row missing: {text}");
    assert!(text.contains("infectious day 1/~9"), "{text}");
    assert!(text.contains("sick — resting more, no mating"), "{text}");
    assert!(text.contains("for life"), "{text}");
    assert!(text.contains("contagion risk"), "{text}");
    assert!(text.contains("resistance cost"), "{text}");
    assert!(text.contains("fell ill with"), "{text}");
}

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::sim::creatures::Mutation;
use crate::sim::TRAIT_NAMES;
use crate::theme;

fn press(screen: &mut Inspector, app: &mut AppState, code: KeyCode) {
    screen.handle_key(KeyEvent::new(code, KeyModifiers::NONE), app);
}

/// Row index of the first line containing `needle`, if any.
fn row_of(text: &str, needle: &str) -> Option<usize> {
    text.lines().position(|l| l.contains(needle))
}

/// The middle (Genome) column of every body row (not the status bar).
fn genome_column(text: &str) -> String {
    text.lines().take(44).map(|l| l.chars().skip(52).take(52).collect::<String>() + "\n").collect()
}

fn world() -> (AppState, crate::sim::creatures::CreatureId) {
    let mut app = AppState::new(Params::default());
    let sim = Sim::new(7, Params::default());
    let id = sim.creatures.living_ids()[0];
    app.sim = Some(sim);
    (app, id)
}

#[test]
fn s03_timeline_follows_behaviour() {
    let (app, id) = world();
    let text = screen_text(&app, &Inspector::new(id));
    let vitals = row_of(&text, " Vitals ").unwrap();
    let behaviour = row_of(&text, " Behaviour ").unwrap();
    let timeline = row_of(&text, " Timeline ").unwrap();
    assert!(vitals < behaviour && behaviour < timeline, "{text}");
}

#[test]
fn s03_genome_scrolls_to_forecast() {
    let (mut app, id) = world();
    let mutations: Vec<Mutation> = (0..6).map(|t| Mutation { trait_idx: t, delta: 0.05, generation: 1 }).collect();
    app.sim.as_mut().unwrap().creatures.get_mut(id).unwrap().mutations = mutations;
    let mut screen = Inspector::new(id);
    let last = TRAIT_NAMES[TRAIT_NAMES.len() - 1];

    let text = screen_text(&app, &screen);
    let col = genome_column(&text);
    // The trait table shows every name once; the clipped forecast would show it twice.
    assert_eq!(col.matches(last).count(), 1, "{col}");
    assert!(col.contains(&format!("{}", glyphs::DOWN)), "no ↓ indicator: {col}");
    assert!(!col.contains(&format!("{}", glyphs::UP)), "unexpected ↑ indicator: {col}");

    press(&mut screen, &mut app, KeyCode::Right);
    press(&mut screen, &mut app, KeyCode::End);
    let text = screen_text(&app, &screen);
    let col = genome_column(&text);
    assert_eq!(col.matches(last).count(), 2, "{col}");
    assert!(col.contains(&format!("{}", glyphs::UP)), "no ↑ indicator: {col}");
    assert!(!col.contains(&format!("{}", glyphs::DOWN)), "unexpected ↓ indicator: {col}");

    press(&mut screen, &mut app, KeyCode::Home);
    let text = screen_text(&app, &screen);
    assert_eq!(genome_column(&text).matches(last).count(), 1);
}

#[test]
fn s03_focus_border_moves_with_arrows() {
    let (mut app, id) = world();
    let mut screen = Inspector::new(id);
    let corner = |app: &AppState, screen: &Inspector, x: u16| {
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| screen.render(app, f, Rect::new(0, 0, 155, 45))).unwrap();
        terminal.backend().buffer()[(x, 0)].fg
    };
    let focus = theme::border_focus().fg.unwrap();
    assert_eq!(corner(&app, &screen, 0), focus);
    assert_ne!(corner(&app, &screen, 52), focus);
    press(&mut screen, &mut app, KeyCode::Right);
    assert_ne!(corner(&app, &screen, 0), focus);
    assert_eq!(corner(&app, &screen, 52), focus);
    press(&mut screen, &mut app, KeyCode::Left);
    press(&mut screen, &mut app, KeyCode::Left);
    assert_eq!(corner(&app, &screen, 104), focus);
}

#[test]
fn s03_scroll_clamps_to_content() {
    let (mut app, id) = world();
    let mut screen = Inspector::new(id);
    let before = screen_text(&app, &screen);
    press(&mut screen, &mut app, KeyCode::Up);
    for _ in 0..200 {
        press(&mut screen, &mut app, KeyCode::Down);
    }
    let after = screen_text(&app, &screen);
    let top = |t: &str| t.lines().nth(1).unwrap().chars().take(52).collect::<String>();
    // Identity fits in its panel, so scrolling past the end leaves it unchanged.
    assert_eq!(top(&before), top(&after), "{after}");
}
