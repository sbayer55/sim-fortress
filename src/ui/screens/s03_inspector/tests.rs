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
