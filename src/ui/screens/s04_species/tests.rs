//! Tests for the S04 species browser.

use super::{Pane, SpeciesBrowser, SpeciesDetail};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
    screen_text_sized(app, screen, 155, 45)
}

fn screen_text_sized(app: &AppState, screen: &dyn Screen, w: u16, h: u16) -> String {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| screen.render(app, f, Rect::new(0, 0, w, h))).unwrap();
    let buf = terminal.backend().buffer();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn app_with_sim() -> AppState {
    let mut app = AppState::new(Params::default());
    let mut sim = Sim::new(7, Params::default());
    for _ in 0..(sim.params.time.ticks_per_day * 3) {
        sim.step();
    }
    app.sim = Some(sim);
    app
}

/// The line of the render that holds the `Selected:` title.
fn summary_title_line(text: &str) -> &str {
    text.lines().find(|l| l.contains("Selected:")).expect("summary panel title")
}

#[test]
fn s04a_focus_starts_on_the_table_and_tab_moves_it() {
    let mut app = app_with_sim();
    let mut s = SpeciesBrowser::new();
    assert_eq!(s.focus, Pane::Table);
    // Default focus: the table has the double focus border colour, and ↑↓ select.
    let before = s.sel;
    s.handle_key(key(KeyCode::Down), &mut app);
    assert_eq!(s.sel, before + 1, "Down selects while the table is focused");

    s.handle_key(key(KeyCode::Tab), &mut app);
    assert_eq!(s.focus, Pane::Summary);
    let sel = s.sel;
    s.handle_key(key(KeyCode::Down), &mut app);
    assert_eq!(s.sel, sel, "Down scrolls, not selects, while the summary is focused");

    s.handle_key(key(KeyCode::BackTab), &mut app);
    assert_eq!(s.focus, Pane::Table);
    s.handle_key(key(KeyCode::Right), &mut app);
    assert_eq!(s.focus, Pane::Summary);
    s.handle_key(key(KeyCode::Left), &mut app);
    assert_eq!(s.focus, Pane::Table);
}

#[test]
fn s04a_focus_border_follows_the_focused_panel() {
    use ratatui::style::Color;
    let app = app_with_sim();
    let mut s = SpeciesBrowser::new();
    let focus_fg = crate::theme::border_focus().fg.unwrap_or(Color::Reset);
    let plain_fg = crate::theme::border().fg.unwrap_or(Color::Reset);
    assert_ne!(focus_fg, plain_fg, "the theme must tell the borders apart");

    let border_fg = |s: &SpeciesBrowser, y: u16| {
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| s.render(&app, f, Rect::new(0, 0, 155, 45))).unwrap();
        terminal.backend().buffer()[(0, y)].fg
    };
    // Row 0 is the table's top border, row 13 the summary's.
    assert_eq!(border_fg(&s, 0), focus_fg, "table focused by default");
    assert_eq!(border_fg(&s, 13), plain_fg, "summary unfocused by default");
    s.focus = Pane::Summary;
    assert_eq!(border_fg(&s, 0), plain_fg);
    assert_eq!(border_fg(&s, 13), focus_fg);
}

#[test]
fn s04a_summary_scrolls_when_the_terminal_is_short() {
    let mut app = app_with_sim();
    let mut s = SpeciesBrowser::new();
    s.focus = Pane::Summary;
    // A 30-row terminal leaves the summary about 14 inner rows: the body overflows.
    let text = screen_text_sized(&app, &s, 155, 30);
    assert!(text.contains(&format!("{}", glyphs::DOWN)), "overflow indicator: {text}");
    assert!(text.contains("Base genome vs current mean"), "{text}");
    s.handle_key(key(KeyCode::End), &mut app);
    let text = screen_text_sized(&app, &s, 155, 30);
    assert!(!text.contains("Base genome vs current mean"), "scrolled past the top: {text}");
    assert!(text.contains("Habitat"), "{text}");
    // Selecting another species resets the scroll.
    s.focus = Pane::Table;
    s.handle_key(key(KeyCode::Down), &mut app);
    let text = screen_text_sized(&app, &s, 155, 30);
    assert!(text.contains("Base genome vs current mean"), "{text}");
}

#[test]
fn s04a_population_plot_fits_the_panel_width() {
    let mut app = app_with_sim();
    for w in [155u16, 120, 100, 80] {
        let mut s = SpeciesBrowser::new();
        s.focus = Pane::Summary;
        let mut text = screen_text_sized(&app, &s, w, 45);
        assert!(summary_title_line(&text).starts_with('╔'), "{w}: {text}");
        // Below 105 inner columns the history stacks under the identity column
        // and is reached by scrolling.
        if !text.contains("Population, last 240 days") {
            s.handle_key(key(KeyCode::End), &mut app);
            text = screen_text_sized(&app, &s, w, 45);
        }
        assert!(text.contains("Population, last 240 days"), "{w}: {text}");
        // Every row inside the summary panel ends with the right border: nothing
        // (in particular the plot) spills past it.
        for (y, line) in text.lines().enumerate().skip(13).take(29) {
            let last = line.chars().last().unwrap();
            assert!(last == '║' || last == '╗' || last == '╝' || last == '╣', "row {y} at width {w} overflows: {line:?}");
        }
        assert!(text.contains("today"), "{w}: axis label: {text}");
    }
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

