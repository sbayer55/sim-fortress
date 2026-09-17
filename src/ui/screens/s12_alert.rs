//! S12: the live alert modals — S12a extinction (C5 FR8) and S12b epidemic
//! (C7 FR9). Drawn non-opaque so the map beneath is dimmed; owns its own
//! status bar repaint.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::Frame;

use crate::sim::{Alert, Cause, CreatureId, EventKind, ExtinctionRecord, PathogenId, Sim, SpeciesId};
use crate::ui::app::AppState;
use crate::ui::screens::common::sp;
use crate::ui::screens::s08_lineage::LineageScreen;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{EventKindStyle, SeasonStyle, SpeciesStyle};
use crate::widgets::{Component, Modal, Rows, Spacer, StatusBar, Text, VStack};
use crate::{glyphs, theme};

const BUTTONS: [&str; 3] = ["[ Continue ]", "[ View lineage ]", "[ Pause ]"];
const HINT: &str = "Enter select   ←→ move   Esc continue";
/// S12b button set: *Show outbreak* replaces *View lineage*.
const EPIDEMIC_BUTTONS: [&str; 3] = ["[ Continue ]", "[ Show outbreak ]", "[ Pause ]"];

#[derive(Debug)]
pub struct AlertModal {
    pub species: SpeciesId,
    pub last: CreatureId,
    pub focus: usize,
    /// C7 FR9: `Some((pathogen, outbreak))` for the epidemic variant (S12b).
    pub epidemic: Option<(PathogenId, u16)>,
}

impl AlertModal {
    pub const fn new(alert: &Alert) -> Self {
        match alert {
            Alert::Extinction { species, last, .. } => Self { species: *species, last: *last, focus: 0, epidemic: None },
            Alert::Epidemic { pathogen, outbreak, .. } => Self { species: SpeciesId(0), last: CreatureId(0), focus: 0, epidemic: Some((*pathogen, *outbreak)) },
        }
    }
}

impl Screen for AlertModal {
    fn opaque(&self) -> bool {
        false // modal: dims whatever is drawn beneath
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        if self.epidemic.is_some() {
            return self.handle_key_epidemic(key, app);
        }
        match key.code {
            KeyCode::Left => {
                self.focus = (self.focus + BUTTONS.len() - 1) % BUTTONS.len();
                Action::None
            }
            KeyCode::Right => {
                self.focus = (self.focus + 1) % BUTTONS.len();
                Action::None
            }
            KeyCode::Enter => self.activate(app),
            KeyCode::Char('l') => {
                app.dismiss_alert(false);
                Action::Replace(Box::new(LineageScreen::new(self.last)))
            }
            KeyCode::Char(' ') => {
                app.dismiss_alert(false);
                Action::Pop
            }
            KeyCode::Esc => {
                app.dismiss_alert(true);
                Action::Pop
            }
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else { return };
        if let Some((pathogen, outbreak)) = self.epidemic {
            self.render_epidemic(sim, f, area, pathogen, outbreak);
            return;
        }
        let s = &sim.species[self.species.index()];
        let record: Option<&ExtinctionRecord> = sim.last_extinct[self.species.index()].as_ref();
        let magenta_bold = Style::default().fg(theme::MAGENTA).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD);
        let last_label = record.map_or_else(|| "#000".to_string(), |r| format!("{} {}", r.name, r.tag));
        let (cause, pos, age) = match record {
            Some(r) => (r.cause, r.pos, r.age),
            None => (Cause::Age, (0, 0), 0),
        };
        let kind = cause_kind(cause);
        let years = match s.first_birth_day {
            Some(fb) => record.map_or(0, |r| r.day.saturating_sub(fb).div_euclid(360)),
            None => sim.time.year().saturating_sub(1),
        };
        let rows: Rows<'static> = vec![
            Box::new(Spacer::rows(1)),
            Box::new(Text::new(format!("The {} are extinct", sim.roster().plural(self.species))).style(magenta_bold).center()),
            Box::new(Spacer::rows(1)),
            Box::new(Text::spans(vec![
                sp(format!("  Year {}, Day {} of {} ", sim.time.year(), sim.time.day_of_season(), sim.time.season().name()), theme::text()),
                sp(sim.time.season().glyph().to_string(), Style::default().fg(sim.time.season().color()).bg(theme::PANEL_BG)),
                sp("   last individual: ", theme::dim_text()),
                sp(last_label, Style::default().fg(sim.roster().color(self.species)).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            ])),
            Box::new(Text::spans(vec![
                sp("  ", theme::text()),
                sp(format!("{} ", glyphs::DEATH), Style::default().fg(kind.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                sp(cause.label(), Style::default().fg(kind.color()).bg(theme::PANEL_BG)),
                sp(
                    match record {
                        Some(r) => format!(" in {} at ({}, {}), age {} days", r.region, pos.0, pos.1, age),
                        None => format!(" in the wild at ({}, {})", pos.0, pos.1),
                    },
                    theme::text(),
                ),
            ])),
            Box::new(Spacer::rows(1)),
            Box::new(Text::spans(vec![
                sp("  peak population ", theme::dim_text()),
                sp(format!("{}", s.peak), theme::text()),
                sp("   generations survived ", theme::dim_text()),
                sp(format!("{}", s.generation), theme::text()),
                sp("   years ", theme::dim_text()),
                sp(format!("{years}"), theme::text()),
            ])),
        ];
        let modal = Modal::new(64, 12)
            .banner(format!("{} EXTINCTION {}", glyphs::EXTINCTION, glyphs::EXTINCTION), theme::MAGENTA)
            .buttons(&BUTTONS, Some(self.focus))
            .hint(HINT);
        let buf = f.buffer_mut();
        let body = modal.render(buf, area);
        VStack::from_boxes(&rows).render(buf, body);

        let keys: &[(&str, &str)] = &[("Enter", "select"), ("←→", "move"), ("l", "lineage"), ("Space", "pause"), ("Esc", "continue")];
        draw_alert_status(f, area, keys, &format!("{} paused on extinction", glyphs::PAUSE_STR));
    }
}

impl AlertModal {
    fn activate(&self, app: &mut AppState) -> Action {
        match self.focus {
            0 => {
                app.dismiss_alert(true);
                Action::Pop
            }
            1 => {
                app.dismiss_alert(false);
                Action::Replace(Box::new(LineageScreen::new(self.last)))
            }
            _ => {
                app.dismiss_alert(false);
                Action::Pop
            }
        }
    }
}

// ---- S12b epidemic variant (C7 FR9) -------------------------------------

impl AlertModal {
    fn handle_key_epidemic(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Left => {
                self.focus = (self.focus + EPIDEMIC_BUTTONS.len() - 1) % EPIDEMIC_BUTTONS.len();
                Action::None
            }
            KeyCode::Right => {
                self.focus = (self.focus + 1) % EPIDEMIC_BUTTONS.len();
                Action::None
            }
            KeyCode::Enter => match self.focus {
                0 => {
                    app.dismiss_alert(true);
                    Action::Pop
                }
                1 => self.show_outbreak(app),
                _ => {
                    app.dismiss_alert(false);
                    Action::Pop
                }
            },
            KeyCode::Char('o') => self.show_outbreak(app),
            KeyCode::Char(' ') => {
                app.dismiss_alert(false);
                Action::Pop
            }
            KeyCode::Esc => {
                app.dismiss_alert(true);
                Action::Pop
            }
            _ => Action::Unhandled,
        }
    }

    /// *Show outbreak*: pop the modal, turn the Disease mark on for this
    /// pathogen (the base and other marks stay as they are, S14 item 22) and
    /// centre the viewport on the origin region; the sim stays paused.
    fn show_outbreak(&self, app: &mut AppState) -> Action {
        let Some((pathogen, outbreak)) = self.epidemic else { return Action::Pop };
        app.dismiss_alert(false);
        app.overlay.show_disease(Some(pathogen));
        let centre = app.sim.as_ref().and_then(|sim| {
            let ob = sim.disease.outbreak(outbreak)?;
            let ri = crate::cast!(ob.origin_region => usize);
            (ri < sim.world.regions.len()).then(|| sim.world.region_centre(ri))
        });
        if let Some((cx, cy)) = centre {
            app.centre_viewport_on(cx, cy);
        }
        Action::Pop
    }

    fn render_epidemic(&self, sim: &Sim, f: &mut Frame<'_>, area: Rect, pathogen: PathogenId, outbreak: u16) {
        let sick_bold = Style::default().fg(theme::SICK).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD);
        let ob = sim.disease.outbreak(outbreak);
        let pth = sim.disease.pathogen(pathogen);
        let name = sim.disease.name(pathogen).to_string();
        // Host species = argmax of the outbreak's per-species case counts.
        let host: SpeciesId = ob
            .and_then(|o| (0..o.species_cases.len()).max_by_key(|&i| (o.species_cases[i], usize::MAX - i)))
            .map_or_else(SpeciesId::default, SpeciesId::from_index);
        let headline = if pth.is_some_and(crate::sim::disease::Pathogen::is_strain) {
            format!("A new strain: {name}")
        } else {
            format!("{} is epidemic among the {}", name, sim.roster().plural(host))
        };
        // When and who: the clock, then the index case.
        let (case_label, case_species) = match ob {
            Some(o) => index_case_label(sim, o.index_case),
            None => ("#000".to_string(), host),
        };
        // Spread: sick · dead · regions · began.
        let (sick, dead, began, origin) = match ob {
            Some(o) => (
                o.active,
                o.deaths,
                o.started_day % 360 + 1,
                sim.world.regions.get(crate::cast!(o.origin_region => usize)).map_or("the wild", |r| r.0.as_str()),
            ),
            None => (0, 0, 1, "the wild"),
        };
        let regions = regions_with_active_cases(sim, outbreak);
        // Resistance summary for the host species.
        let hs = &sim.species[host.index()];
        let mean = hs.mean.resistance();
        let base = sim.roster().base_genome(host).resistance();
        let immune = sim.disease.stats.get(crate::cast!(pathogen.0 => usize)).map_or(0, |s| s.immune);
        let pct = if hs.count > 0 { crate::cast!((crate::cast!(immune => f32) * 100.0 / crate::cast!(hs.count => f32)).round() => u32) } else { 0 };

        let rows: Rows<'static> = vec![
            Box::new(Spacer::rows(1)),
            Box::new(Text::new(headline).style(sick_bold).center()),
            Box::new(Spacer::rows(1)),
            Box::new(Text::spans(vec![
                sp(format!("  Year {}, Day {} of {} ", sim.time.year(), sim.time.day_of_season(), sim.time.season().name()), theme::text()),
                sp(sim.time.season().glyph().to_string(), Style::default().fg(sim.time.season().color()).bg(theme::PANEL_BG)),
                sp("   index case: ", theme::dim_text()),
                sp(case_label, Style::default().fg(sim.roster().color(case_species)).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            ])),
            Box::new(Text::spans(vec![
                sp("  ", theme::text()),
                sp(format!("{sick} sick"), Style::default().fg(theme::SICK).bg(theme::PANEL_BG)),
                sp(" · ", theme::dim_text()),
                sp(format!("{dead} dead"), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
                sp(" · ", theme::dim_text()),
                sp(format!("{}/{} regions", regions, sim.world.regions.len().max(1)), theme::text()),
                sp(" · ", theme::dim_text()),
                // Keep the row inside the 62-cell modal: `since D36, Southern Thicket`.
                sp(format!("since D{began}, {origin}"), theme::text()),
            ])),
            Box::new(Spacer::rows(1)),
            Box::new(Text::spans(vec![
                sp("  mean Resistance ", theme::dim_text()),
                sp(format!("{mean:.2}"), Style::default().fg(theme::SICK).bg(theme::PANEL_BG)),
                sp(format!(" (base {base:.2})"), theme::dim_text()),
                sp("   ", theme::text()),
                sp(format!("{pct}% immune"), Style::default().fg(theme::IMMUNE).bg(theme::PANEL_BG)),
            ])),
        ];
        let modal = Modal::new(64, 12)
            .banner(format!("{} EPIDEMIC {}", glyphs::DISEASE, glyphs::DISEASE), theme::SICK)
            .buttons(&EPIDEMIC_BUTTONS, Some(self.focus))
            .hint(HINT);
        let buf = f.buffer_mut();
        let body = modal.render(buf, area);
        VStack::from_boxes(&rows).render(buf, body);

        let keys: &[(&str, &str)] = &[("Enter", "select"), ("←→", "move"), ("o", "show outbreak"), ("Space", "pause"), ("Esc", "continue")];
        draw_alert_status(f, area, keys, &format!("{} paused on epidemic", glyphs::PAUSE_STR));
    }
}

/// How many regions hold at least one active case of `outbreak`.
fn regions_with_active_cases(sim: &Sim, outbreak: u16) -> usize {
    let mut region_hit = [false; 8];
    for c in sim.creatures.living() {
        if c.infection.as_ref().is_some_and(|i| i.outbreak == outbreak) {
            let r = sim.world.region_index(c.x, c.y);
            if r < region_hit.len() {
                region_hit[r] = true;
            }
        }
    }
    region_hit.iter().filter(|&&b| b).count()
}

/// The alert status bar, repainted undimmed over the underlying screen.
fn draw_alert_status(f: &mut Frame<'_>, area: Rect, keys: &[(&str, &str)], right: &str) {
    let status_row = area.y + area.height - 1;
    StatusBar::new(keys).right(right).render(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1));
}

/// `Name tag` and species of the index case: the creature record when still
/// stored, else its lineage node, else a bare `#id` in the default colour.
fn index_case_label(sim: &Sim, id: CreatureId) -> (String, SpeciesId) {
    if let Some(c) = sim.creatures.get(id) {
        return (c.label(sim.roster()), c.species);
    }
    if let Some(n) = sim.lineage.get(id) {
        return (format!("{} {}", n.name_str(sim.roster()), n.tag), n.species);
    }
    (format!("#{:03}", id.0), SpeciesId::default())
}

const fn cause_kind(cause: Cause) -> EventKind {
    match cause {
        Cause::Starved => EventKind::DeathStarved,
        Cause::Thirst => EventKind::DeathThirst,
        Cause::Predation => EventKind::DeathPredation,
        Cause::Age | Cause::Injury => EventKind::DeathAge,
        Cause::Disease => EventKind::DeathDisease,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::disease::Outbreak;
    use crate::widgets::map::Disease;
    use crate::sim::Params;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyModifiers;
    use ratatui::Terminal;

    fn app_with_outbreak() -> AppState {
        let mut app = AppState::new(Params::default());
        let mut sim = Sim::new(7, Params::default());
        let index_case = sim.creatures.living().next().map_or(CreatureId(0), |c| c.id);
        sim.disease.first_index = 0;
        sim.disease.outbreaks.push(Outbreak {
            pathogen: PathogenId(0),
            started_day: 12,
            ended_day: None,
            origin_region: 3,
            index_case,
            cases: 40,
            deaths: 7,
            recovered: 5,
            peak_active: 30,
            peak_day: 20,
            species_cases: vec![30, 10, 0, 0, 0, 0],
            species_deaths: vec![5, 2, 0, 0, 0, 0],
            epidemic: true,
            resist_at_start: vec![0.3; 6],
            resist_at_end: vec![0.0; 6],
            active: 28,
            cases_today: 3,
        });
        app.sim = Some(sim);
        app.viewport_size.set((110, 40));
        app
    }

    fn render(app: &AppState, modal: &AlertModal) -> String {
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| modal.render(app, f, Rect::new(0, 0, 155, 45))).unwrap();
        let buf = terminal.backend().buffer();
        (0..45)
            .map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n")
            .collect()
    }

    #[test]
    fn s12b_renders_buttons() {
        let mut app = app_with_outbreak();
        let alert = Alert::Epidemic { event_index: 1, pathogen: PathogenId(0), outbreak: 0 };
        app.alert_shown = Some(alert.clone());
        let modal = AlertModal::new(&alert);
        let text = render(&app, &modal);
        assert!(text.contains("EPIDEMIC"), "title missing:\n{text}");
        assert!(text.contains("is epidemic among the Voles") || text.contains("A new strain:"), "headline missing:\n{text}");
        assert!(text.contains("index case:"));
        assert!(text.contains("28 sick · 7 dead ·"));
        assert!(text.contains("mean Resistance"));
        assert!(text.contains("[ Continue ]    [ Show outbreak ]    [ Pause ]"));
        assert!(text.contains("paused on epidemic"));
    }

    #[test]
    fn s12b_show_outbreak_shortcuts() {
        let mut app = app_with_outbreak();
        let alert = Alert::Epidemic { event_index: 1, pathogen: PathogenId(0), outbreak: 0 };
        app.alert_shown = Some(alert.clone());
        let mut modal = AlertModal::new(&alert);

        // `l` is not a shortcut on S12b.
        let a = modal.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE), &mut app);
        assert!(matches!(a, Action::Unhandled));
        assert!(!app.overlay.disease.is_on());

        // Right → Show outbreak, Enter activates it.
        modal.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &mut app);
        assert_eq!(modal.focus, 1);
        let a = modal.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut app);
        assert!(matches!(a, Action::Pop));
        assert_eq!(app.overlay.disease, Disease::On(Some(PathogenId(0))));
        assert!(app.alert_shown.is_none());

        // `o` is the shortcut for the same button.
        let mut app = app_with_outbreak();
        app.alert_shown = Some(alert.clone());
        let mut modal = AlertModal::new(&alert);
        let a = modal.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), &mut app);
        assert!(matches!(a, Action::Pop));
        assert_eq!(app.overlay.disease, Disease::On(Some(PathogenId(0))));
    }
}
