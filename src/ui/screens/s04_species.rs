//! S04: the live species browser — S04a table + selected-species summary and
//! S04b per-species trait distributions and drift (C4 FR7).

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use crate::sim::{Sim, SpeciesId};
use crate::ui::app::AppState;
use crate::ui::screens::{Action, Screen};
use crate::widgets::status;
use crate::glyphs;

use table::table;
use summary::summary;
use histograms::histograms;
use drift::drift;
pub use drift::selection_pressure;

const TABLE_H: u16 = 13;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortCol {
    Count,
    Births,
    Deaths,
    Generation,
    Name,
}

impl SortCol {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Count => Self::Births,
            Self::Births => Self::Deaths,
            Self::Deaths => Self::Generation,
            Self::Generation => Self::Name,
            Self::Name => Self::Count,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Births => "births",
            Self::Deaths => "deaths",
            Self::Generation => "generation",
            Self::Name => "name",
        }
    }
}

/// Species indices in display order for `sort`; ties keep species order.
pub fn sorted_indices(sim: &Sim, sort: SortCol) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..6).collect();
    let key = |i: usize| -> (i64, usize) {
        let s = &sim.species[i];
        let v = match sort {
            SortCol::Count => i64::from(s.count),
            SortCol::Births => i64::from(s.births_yesterday),
            SortCol::Deaths => i64::from(s.deaths_yesterday),
            SortCol::Generation => i64::from(s.generation),
            SortCol::Name => 0,
        };
        (-v, i)
    };
    match sort {
        SortCol::Name => idx.sort_by_key(|&i| (SpeciesId::ALL[i].name(), i)),
        _ => idx.sort_by_key(|&i| key(i)),
    }
    idx
}

#[derive(Debug)]
pub struct SpeciesBrowser {
    /// Position in the sorted table.
    pub sel: usize,
    pub sort: SortCol,
}

impl Default for SpeciesBrowser {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeciesBrowser {
    pub const fn new() -> Self {
        Self { sel: 0, sort: SortCol::Count }
    }

    fn selected_species(&self, sim: &Sim) -> SpeciesId {
        let order = sorted_indices(sim, self.sort);
        SpeciesId::ALL[order[self.sel.min(5)]]
    }
}

impl Screen for SpeciesBrowser {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Up => {
                self.sel = self.sel.saturating_sub(1);
                Action::None
            }
            KeyCode::Down => {
                self.sel = (self.sel + 1).min(5);
                Action::None
            }
            KeyCode::Char('s') => {
                // Keep the same species selected across the re-sort.
                let species = app.sim.as_ref().map(|sim| self.selected_species(sim));
                self.sort = self.sort.next();
                if let (Some(sim), Some(species)) = (app.sim.as_ref(), species) {
                    let order = sorted_indices(sim, self.sort);
                    self.sel = order.iter().position(|&i| SpeciesId::ALL[i] == species).unwrap_or(0);
                }
                Action::None
            }
            KeyCode::Enter => match app.sim.as_ref() {
                Some(sim) => Action::Push(Box::new(SpeciesDetail::new(self.selected_species(sim)))),
                None => Action::None,
            },
            KeyCode::Esc => Action::Pop,
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else { return };
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let species = self.selected_species(sim);
        table(f, Rect::new(area.x, area.y, area.width, TABLE_H), sim, self.sort, species);
        summary(f, Rect::new(area.x, area.y + TABLE_H, area.width, body_h - TABLE_H), sim, species);
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("↑↓", "select"), ("Enter", "detail"), ("s", "sort"), ("Esc", "back")],
            &format!("sorted by {} {}  {}", self.sort.label(), glyphs::DOWN, sim.time.clock_label()),
        );
    }
}

#[derive(Debug)]
pub struct SpeciesDetail {
    pub species: SpeciesId,
}

impl SpeciesDetail {
    pub const fn new(species: SpeciesId) -> Self {
        Self { species }
    }
}

impl Screen for SpeciesDetail {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, _app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Esc => Action::Pop,
            KeyCode::Left | KeyCode::Up => {
                self.species = SpeciesId::ALL[(self.species.index() + 5) % 6];
                Action::None
            }
            KeyCode::Right | KeyCode::Down => {
                self.species = SpeciesId::ALL[(self.species.index() + 1) % 6];
                Action::None
            }
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else { return };
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let left_w = 80u16;
        histograms(f, Rect::new(area.x, area.y, left_w, body_h), sim, self.species);
        drift(f, Rect::new(area.x + left_w, area.y, area.width - left_w, body_h), sim, self.species);
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("←→", "other species"), ("Esc", "back")],
            &format!("{} detail  {}", self.species.name(), sim.time.clock_label()),
        );
    }
}

mod table;
mod summary;
mod histograms;
mod drift;
#[cfg(test)]
mod tests;
