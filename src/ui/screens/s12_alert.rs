//! S12: the live alert modals — S12a extinction (C5 FR8) and S12b epidemic
//! (C7 FR9). Drawn non-opaque so the map beneath is dimmed; owns its own
//! status bar repaint.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::{Alert, Cause, CreatureId, EventKind, ExtinctionRecord, PathogenId, Sim, SpeciesId};
use crate::ui::app::AppState;
use crate::ui::screens::s08_lineage::LineageScreen;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{EventKindStyle, SeasonStyle, SpeciesStyle};
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

const BUTTONS: [&str; 3] = ["[ Continue ]", "[ View lineage ]", "[ Pause ]"];
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
            Alert::Epidemic { pathogen, outbreak, .. } => Self { species: SpeciesId::Vole, last: CreatureId(0), focus: 0, epidemic: Some((*pathogen, *outbreak)) },
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

        let modal = util::centered(area, 64, 12);
        let inner = panel::draw(f, modal, "", panel::Kind::Focus);
        {
            let buf = f.buffer_mut();
            let title = format!(" {} EXTINCTION {} ", glyphs::EXTINCTION, glyphs::EXTINCTION);
            let x = modal.x + (modal.width - crate::cast!(title.chars().count() => u16)).div_euclid(2);
            buf.set_stringn(x, modal.y, &title, title.chars().count(), Style::default().fg(theme::MAGENTA).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
        }

        let s = &sim.species[self.species.index()];
        let record: Option<&ExtinctionRecord> = sim.last_extinct[self.species.index()].as_ref();

        let center = |s: &str| -> String {
            let pad = (crate::cast!(inner.width => usize)).saturating_sub(s.chars().count()).div_euclid(2);
            format!("{}{}", " ".repeat(pad), s)
        };

        let mut row = 1u16;

        // Headline.
        util::line(f, inner, row, Line::from(Span::styled(
            center(&format!("The {} are extinct", self.species.plural())),
            Style::default().fg(theme::MAGENTA).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD),
        )));
        row += 2;

        // When and who.
        let last_label = record.map_or_else(|| "#000".to_string(), |r| format!("{} {}", r.name, r.tag));
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!("  Year {}, Day {} of {} ", sim.time.year(), sim.time.day_of_season(), sim.time.season().name()), theme::text()),
            Span::styled(sim.time.season().glyph().to_string(), Style::default().fg(sim.time.season().color()).bg(theme::PANEL_BG)),
            Span::styled("   last individual: ", theme::dim_text()),
            Span::styled(last_label, Style::default().fg(self.species.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        ]));
        row += 1;

        // How.
        let (cause, pos, age) = match record {
            Some(r) => (r.cause, r.pos, r.age),
            None => (Cause::Age, (0, 0), 0),
        };
        let kind = cause_kind(cause);
        util::line(f, inner, row, Line::from(vec![
            Span::styled("  ", theme::text()),
            Span::styled(format!("{} ", glyphs::DEATH), Style::default().fg(kind.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled(cause.label(), Style::default().fg(kind.color()).bg(theme::PANEL_BG)),
            Span::styled(
                match record {
                    Some(r) => format!(" in {} at ({}, {}), age {} days", r.region, pos.0, pos.1, age),
                    None => format!(" in the wild at ({}, {})", pos.0, pos.1),
                },
                theme::text(),
            ),
        ]));
        row += 2;

        // Line summary.
        let years = match s.first_birth_day {
            Some(fb) => record.map_or(0, |r| r.day.saturating_sub(fb).div_euclid(360)),
            None => sim.time.year().saturating_sub(1),
        };
        util::line(f, inner, row, Line::from(vec![
            Span::styled("  peak population ", theme::dim_text()),
            Span::styled(format!("{}", s.peak), theme::text()),
            Span::styled("   generations survived ", theme::dim_text()),
            Span::styled(format!("{}", s.generation), theme::text()),
            Span::styled("   years ", theme::dim_text()),
            Span::styled(format!("{years}"), theme::text()),
        ]));
        row += 2;

        // Buttons.
        let total: usize = BUTTONS.iter().map(|b| b.chars().count()).sum::<usize>() + BUTTONS.len() * 4;
        let pad = (crate::cast!(inner.width => usize)).saturating_sub(total).div_euclid(2);
        let mut spans = vec![Span::styled(" ".repeat(pad), theme::text())];
        for (i, b) in BUTTONS.iter().enumerate() {
            let st = if i == self.focus { theme::selected() } else { theme::text() };
            spans.push(Span::styled(*b, st));
            spans.push(Span::styled("    ", theme::text()));
        }
        util::line(f, inner, row, Line::from(spans));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(center("Enter select   ←→ move   Esc continue"), theme::dim_text())));

        // Status bar, repainted undimmed.
        let status_row = area.y + area.height - 1;
        util::fill(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1), Style::default().bg(theme::STATUS_BG));
        let keys: &[(&str, &str)] = &[("Enter", "select"), ("←→", "move"), ("l", "lineage"), ("Space", "pause"), ("Esc", "continue")];
        let right = format!("{} paused on extinction", glyphs::PAUSE_STR);
        status::render(f, Rect::new(area.x, status_row, area.width, 1), keys, &right);
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

    /// *Show outbreak*: pop the modal, ask the map to open the disease overlay
    /// on this pathogen and centre the viewport on the origin region; the sim
    /// stays paused.
    fn show_outbreak(&self, app: &mut AppState) -> Action {
        let Some((pathogen, outbreak)) = self.epidemic else { return Action::Pop };
        app.dismiss_alert(false);
        app.pending_overlay = Some(pathogen);
        let centre = app.sim.as_ref().and_then(|sim| {
            let ob = sim.disease.outbreak(outbreak)?;
            let r = sim.world.regions.get(crate::cast!(ob.origin_region => usize))?;
            Some(((r.1 + r.3).div_euclid(2), (r.2 + r.4).div_euclid(2)))
        });
        if let Some((cx, cy)) = centre {
            app.centre_viewport_on(cx, cy);
        }
        Action::Pop
    }

    fn render_epidemic(&self, sim: &Sim, f: &mut Frame<'_>, area: Rect, pathogen: PathogenId, outbreak: u16) {
        let modal = util::centered(area, 64, 12);
        let inner = panel::draw(f, modal, "", panel::Kind::Focus);
        let sick_bold = Style::default().fg(theme::SICK).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD);
        draw_epidemic_title(f, modal, sick_bold);

        let center = |s: &str| -> String { center_line(inner, s) };

        let ob = sim.disease.outbreak(outbreak);
        let pth = sim.disease.pathogen(pathogen);
        let name = sim.disease.name(pathogen).to_string();
        // Host species = argmax of the outbreak's per-species case counts.
        let host: SpeciesId = ob
            .and_then(|o| (0..6).max_by_key(|&i| (o.species_cases[i], usize::MAX - i)))
            .and_then(|i| SpeciesId::ALL.get(i).copied())
            .unwrap_or(SpeciesId::Vole);

        let mut row = 1u16;

        // Headline.
        let headline = if pth.is_some_and(crate::sim::disease::Pathogen::is_strain) {
            format!("A new strain: {name}")
        } else {
            format!("{} is epidemic among the {}", name, host.plural())
        };
        util::line(f, inner, row, Line::from(Span::styled(center(&headline), sick_bold)));
        row += 2;

        // When and who: the clock, then the index case.
        let (case_label, case_species) = match ob {
            Some(o) => index_case_label(sim, o.index_case),
            None => ("#000".to_string(), host),
        };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!("  Year {}, Day {} of {} ", sim.time.year(), sim.time.day_of_season(), sim.time.season().name()), theme::text()),
            Span::styled(sim.time.season().glyph().to_string(), Style::default().fg(sim.time.season().color()).bg(theme::PANEL_BG)),
            Span::styled("   index case: ", theme::dim_text()),
            Span::styled(case_label, Style::default().fg(case_species.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        ]));
        row += 1;

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
        util::line(f, inner, row, Line::from(vec![
            Span::styled("  ", theme::text()),
            Span::styled(format!("{sick} sick"), Style::default().fg(theme::SICK).bg(theme::PANEL_BG)),
            Span::styled(" · ", theme::dim_text()),
            Span::styled(format!("{dead} dead"), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
            Span::styled(" · ", theme::dim_text()),
            Span::styled(format!("{}/{} regions", regions, sim.world.regions.len().max(1)), theme::text()),
            Span::styled(" · ", theme::dim_text()),
            // Keep the row inside the 62-cell modal: `since D36, Southern Thicket`.
            Span::styled(format!("since D{began}, {origin}"), theme::text()),
        ]));
        row += 2;

        // Resistance summary for the host species.
        let hs = &sim.species[host.index()];
        let mean = hs.mean.resistance();
        let base = host.base_genome().resistance();
        let immune = sim.disease.stats.get(crate::cast!(pathogen.0 => usize)).map_or(0, |s| s.immune);
        let pct = if hs.count > 0 { crate::cast!((crate::cast!(immune => f32) * 100.0 / crate::cast!(hs.count => f32)).round() => u32) } else { 0 };
        util::line(f, inner, row, Line::from(vec![
            Span::styled("  mean Resistance ", theme::dim_text()),
            Span::styled(format!("{mean:.2}"), Style::default().fg(theme::SICK).bg(theme::PANEL_BG)),
            Span::styled(format!(" (base {base:.2})"), theme::dim_text()),
            Span::styled("   ", theme::text()),
            Span::styled(format!("{pct}% immune"), Style::default().fg(theme::IMMUNE).bg(theme::PANEL_BG)),
        ]));
        row += 2;

        draw_epidemic_buttons(f, inner, row, self.focus);
        draw_alert_status(f, area, &format!("{} paused on epidemic", glyphs::PAUSE_STR));
    }
}

/// Paint the bordered EPIDEMIC heading across the modal's top edge.
fn draw_epidemic_title(f: &mut Frame<'_>, modal: Rect, style: Style) {
    let buf = f.buffer_mut();
    let title = format!(" {} EPIDEMIC {} ", glyphs::DISEASE, glyphs::DISEASE);
    let x = modal.x + (modal.width - crate::cast!(title.chars().count() => u16)).div_euclid(2);
    buf.set_stringn(x, modal.y, &title, title.chars().count(), style);
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

/// The action buttons and the key hint under them.
fn draw_epidemic_buttons(f: &mut Frame<'_>, inner: Rect, row: u16, focus: usize) {
    let total: usize = EPIDEMIC_BUTTONS.iter().map(|b| b.chars().count()).sum::<usize>() + EPIDEMIC_BUTTONS.len() * 4;
    let pad = (crate::cast!(inner.width => usize)).saturating_sub(total).div_euclid(2);
    let mut spans = vec![Span::styled(" ".repeat(pad), theme::text())];
    for (i, b) in EPIDEMIC_BUTTONS.iter().enumerate() {
        let st = if i == focus { theme::selected() } else { theme::text() };
        spans.push(Span::styled(*b, st));
        spans.push(Span::styled("    ", theme::text()));
    }
    util::line(f, inner, row, Line::from(spans));
    util::line(f, inner, row + 1, Line::from(Span::styled(center_line(inner, "Enter select   ←→ move   Esc continue"), theme::dim_text())));
}

/// A string centred inside `inner`.
fn center_line(inner: Rect, s: &str) -> String {
    let pad = (crate::cast!(inner.width => usize)).saturating_sub(s.chars().count()).div_euclid(2);
    format!("{}{}", " ".repeat(pad), s)
}

/// The alert status bar, repainted undimmed over the underlying screen.
fn draw_alert_status(f: &mut Frame<'_>, area: Rect, right: &str) {
    let status_row = area.y + area.height - 1;
    util::fill(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1), Style::default().bg(theme::STATUS_BG));
    let keys: &[(&str, &str)] = &[("Enter", "select"), ("←→", "move"), ("o", "show outbreak"), ("Space", "pause"), ("Esc", "continue")];
    status::render(f, Rect::new(area.x, status_row, area.width, 1), keys, right);
}

/// `Name tag` and species of the index case: the creature record when still
/// stored, else its lineage node, else a bare `#id` in the default colour.
fn index_case_label(sim: &Sim, id: CreatureId) -> (String, SpeciesId) {
    if let Some(c) = sim.creatures.get(id) {
        return (format!("{} {}", c.name_str(), c.tag()), c.species);
    }
    if let Some(n) = sim.lineage.get(id) {
        return (format!("{} {}", crate::sim::species::name_for(n.species, n.name), n.tag), n.species);
    }
    (format!("#{:03}", id.0), SpeciesId::Vole)
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
            species_cases: [30, 10, 0, 0, 0, 0],
            species_deaths: [5, 2, 0, 0, 0, 0],
            epidemic: true,
            resist_at_start: [0.3; 6],
            resist_at_end: [0.0; 6],
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
        assert!(app.pending_overlay.is_none());

        // Right → Show outbreak, Enter activates it.
        modal.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &mut app);
        assert_eq!(modal.focus, 1);
        let a = modal.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut app);
        assert!(matches!(a, Action::Pop));
        assert_eq!(app.pending_overlay, Some(PathogenId(0)));
        assert!(app.alert_shown.is_none());

        // `o` is the shortcut for the same button.
        let mut app = app_with_outbreak();
        app.alert_shown = Some(alert.clone());
        let mut modal = AlertModal::new(&alert);
        let a = modal.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), &mut app);
        assert!(matches!(a, Action::Pop));
        assert_eq!(app.pending_overlay, Some(PathogenId(0)));
    }
}
