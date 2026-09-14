//! Tests for the S04 species browser.

use super::{SpeciesBrowser, SpeciesDetail};
use ratatui::layout::Rect;
use crate::sim::Sim;
use crate::ui::app::AppState;
use crate::ui::screens::Screen;
use crate::glyphs;
use crate::sim::disease::{Infection, Outbreak, PathogenId, Stage};
use crate::sim::Params;
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
fn s04b_disease_section() {
    let mut app = AppState::new(Params::default());
    let mut sim = Sim::new(7, Params::default());
    let id = sim.creatures.living_ids()[0];
    let species = sim.creatures.get(id).unwrap().species;
    sim.creatures.get_mut(id).unwrap().infection =
        Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 0, ends_day: 9, severity: 0.8, source: None, outbreak: 0 });
    let mut species_cases = [0u32; 6];
    species_cases[species.index()] = 12;
    sim.disease.outbreaks.push(Outbreak {
        pathogen: PathogenId(0),
        started_day: 0,
        ended_day: None,
        origin_region: 0,
        index_case: id,
        cases: 12,
        deaths: 3,
        recovered: 2,
        peak_active: 5,
        peak_day: 0,
        species_cases,
        species_deaths: [0; 6],
        epidemic: false,
        resist_at_start: [0.3; 6],
        resist_at_end: [0.0; 6],
        active: 5,
        cases_today: 1,
    });
    app.sim = Some(sim);
    let text = screen_text(&app, &SpeciesDetail::new(species));
    assert!(text.contains("Disease"), "{text}");
    assert!(text.contains(&format!("{} Y1 D001", glyphs::DISEASE)), "outbreak line missing: {text}");
    assert!(text.contains("12 cases, 0 dead, resist .30"), "{text}");
    // S04a: the Sick column header and the summary's disease lines render too.
    let text = screen_text(&app, &SpeciesBrowser::new());
    assert!(text.contains("Sick"), "{text}");
    assert!(text.contains("susceptible to:"), "{text}");
    assert!(text.contains("worms: mean load"), "{text}");
}
