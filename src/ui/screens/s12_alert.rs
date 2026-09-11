//! S12: the live extinction alert modal (C5 FR8). Drawn non-opaque so the map
//! beneath is dimmed; owns its own status bar repaint.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::{Alert, Cause, EventKind, ExtinctionRecord, SpeciesId};
use crate::ui::app::AppState;
use crate::ui::screens::s08_lineage::LineageScreen;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::{EventKindStyle, SeasonStyle, SpeciesStyle};
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

const BUTTONS: [&str; 3] = ["[ Continue ]", "[ View lineage ]", "[ Pause ]"];

pub struct AlertModal {
    pub species: SpeciesId,
    pub last: crate::sim::CreatureId,
    pub focus: usize,
}

impl AlertModal {
    pub fn new(alert: &Alert) -> Self {
        match alert {
            Alert::Extinction { species, last, .. } => AlertModal { species: *species, last: *last, focus: 0 },
        }
    }
}

impl Screen for AlertModal {
    fn opaque(&self) -> bool {
        false // modal: dims whatever is drawn beneath
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
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

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
        let Some(sim) = &app.sim else { return };

        let modal = util::centered(area, 64, 12);
        let inner = panel::draw(f, modal, "", panel::Kind::Focus);
        {
            let buf = f.buffer_mut();
            let title = format!(" {} EXTINCTION {} ", glyphs::EXTINCTION, glyphs::EXTINCTION);
            let x = modal.x + (modal.width - title.chars().count() as u16) / 2;
            buf.set_stringn(x, modal.y, &title, title.chars().count(), Style::default().fg(theme::MAGENTA).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
        }

        let s = &sim.species[self.species.index()];
        let record: Option<&ExtinctionRecord> = sim.last_extinct[self.species.index()].as_ref();

        let center = |s: &str| -> String {
            let pad = (inner.width as usize).saturating_sub(s.chars().count()) / 2;
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
        let last_label = record.map(|r| format!("{} {}", r.name, r.tag)).unwrap_or_else(|| "#000".to_string());
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
            Some(fb) => record.map(|r| r.day.saturating_sub(fb) / 360).unwrap_or(0),
            None => sim.time.year().saturating_sub(1),
        };
        util::line(f, inner, row, Line::from(vec![
            Span::styled("  peak population ", theme::dim_text()),
            Span::styled(format!("{}", s.peak), theme::text()),
            Span::styled("   generations survived ", theme::dim_text()),
            Span::styled(format!("{}", s.generation), theme::text()),
            Span::styled("   years ", theme::dim_text()),
            Span::styled(format!("{}", years), theme::text()),
        ]));
        row += 2;

        // Buttons.
        let total: usize = BUTTONS.iter().map(|b| b.chars().count()).sum::<usize>() + BUTTONS.len() * 4;
        let pad = (inner.width as usize).saturating_sub(total) / 2;
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
    fn activate(&mut self, app: &mut AppState) -> Action {
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

fn cause_kind(cause: Cause) -> EventKind {
    match cause {
        Cause::Starved => EventKind::DeathStarved,
        Cause::Thirst => EventKind::DeathThirst,
        Cause::Predation => EventKind::DeathPredation,
        Cause::Age => EventKind::DeathAge,
        Cause::Injury => EventKind::DeathAge,
    }
}
