//! Tests for the S05 charts screen.

use super::*;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use crate::sim::{Outbreak, PathogenId};
use crate::sim::Sim;
use crate::ui::app::AppState;
use crate::ui::screens::Screen;
use crate::glyphs;
use crate::sim::{CreatureId, Params};
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::Terminal;

fn fake_outbreak(sim: &mut Sim) {
    sim.disease.first_index = 0;
    sim.disease.outbreaks.push(Outbreak {
        pathogen: PathogenId(0),
        started_day: 0,
        ended_day: Some(12),
        origin_region: 1,
        index_case: CreatureId(0),
        cases: 14,
        deaths: 5,
        recovered: 9,
        peak_active: 7,
        peak_day: 6,
        species_cases: [14, 0, 0, 0, 0, 0],
        species_deaths: [5, 0, 0, 0, 0, 0],
        epidemic: true,
        resist_at_start: [0.30; 6],
        resist_at_end: [0.33; 6],
        active: 0,
        cases_today: 0,
    });
}

fn screen_text(app: &AppState, screen: &Charts) -> String {
    let backend = TestBackend::new(155, 45);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| screen.render(app, f, Rect::new(0, 0, 155, 45))).unwrap();
    let buf = terminal.backend().buffer();
    (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
}

#[test]
fn s05d_variant_cycling() {
    let mut app = AppState::new(Params::default());
    let mut sim = Sim::new(7, Params::default());
    fake_outbreak(&mut sim);
    app.sim = Some(sim);

    let mut s = Charts::new();
    // g cycles a → b → c → d → a.
    for _ in 0..3 {
        s.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE), &mut app);
    }
    assert!(s.variant == Variant::Infections);
    s.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE), &mut app);
    assert!(s.variant == Variant::Time);
    // 4 picks it directly; 1–3 keep their meaning.
    s.handle_key(KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE), &mut app);
    assert!(s.variant == Variant::Infections);
    s.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE), &mut app);
    assert!(s.variant == Variant::Phase);
    assert!(screen_text(&app, &s).contains("chart 2/4"));
}

#[test]
fn s05d_infections_render() {
    let mut app = AppState::new(Params::default());
    let mut sim = Sim::new(7, Params::default());
    fake_outbreak(&mut sim);
    let name = sim.disease.name(PathogenId(0)).to_string();
    assert!(!name.is_empty() && name != "?", "default roster should have a pathogen in slot 0");
    app.sim = Some(sim);

    let mut s = Charts::new();
    for _ in 0..3 {
        s.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE), &mut app);
    }
    assert!(s.variant == Variant::Infections);
    let text = screen_text(&app, &s);
    assert!(text.contains("Infections — cases and resistance"), "{text}");
    assert!(text.contains("Mean Resistance (host species)"), "{text}");
    assert!(text.contains("Outbreaks"), "{text}");
    assert!(text.contains(&format!("{} {}", glyphs::DISEASE, name)), "{text}");
    assert!(text.contains("cases 14"), "{text}");
    assert!(text.contains("dead 5"), "{text}");
    assert!(text.contains("resist +.03"), "{text}");
    assert!(text.contains("chart 4/4  infections"), "{text}");
}
