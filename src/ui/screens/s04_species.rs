//! S04: the live species browser — S04a table + selected-species summary and
//! S04b per-species trait distributions and drift (C4 FR7).
//!
//! S04a has two panels. `Tab` / `Shift+Tab` (or `← →`) move the focus between
//! them: with the table focused `↑ ↓` move the selection; with the summary
//! focused `↑ ↓ PgUp PgDn Home End` scroll it. The focused panel draws the
//! Focus border.

use std::cell::Cell;

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use crate::sim::{Sim, SpeciesId};
use crate::ui::app::AppState;
use crate::ui::screens::{Action, Screen};
use crate::widgets::scroll::{self, Overflow};
use crate::widgets::{panel, status};
use crate::glyphs;

use table::table;
use summary::summary_body;
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
    let mut idx: Vec<usize> = (0..sim.roster().len()).collect();
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
        SortCol::Name => idx.sort_by_key(|&i| (sim.roster().name(SpeciesId::from_index(i)).to_string(), i)),
        _ => idx.sort_by_key(|&i| key(i)),
    }
    idx
}

/// The two S04a panels that can hold the keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Table,
    Summary,
}

impl Pane {
    const fn other(self) -> Self {
        match self {
            Self::Table => Self::Summary,
            Self::Summary => Self::Table,
        }
    }
}

#[derive(Debug)]
pub struct SpeciesBrowser {
    /// Position in the sorted table.
    pub sel: usize,
    pub sort: SortCol,
    /// The panel `↑ ↓` act on.
    pub focus: Pane,
    /// Requested summary scroll offset; clamped at render time.
    offset: u16,
    /// Rows the summary body used and the rows visible, measured by the last
    /// render (the same interior-mutability trick as `viewport_size`).
    measured: Cell<(u16, u16)>,
}

impl Default for SpeciesBrowser {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeciesBrowser {
    pub const fn new() -> Self {
        Self { sel: 0, sort: SortCol::Count, focus: Pane::Table, offset: 0, measured: Cell::new((0, 0)) }
    }

    fn selected_species(&self, sim: &Sim) -> SpeciesId {
        let order = sorted_indices(sim, self.sort);
        SpeciesId::from_index(order[self.sel.min(order.len().saturating_sub(1))])
    }

    /// Scroll the summary by `delta` rows, clamped to its measured content.
    fn scroll_by(&mut self, delta: i32) {
        let (content, visible) = self.measured.get();
        let max = i32::from(Overflow::max_offset(content, visible));
        self.offset = crate::cast!((i32::from(self.offset) + delta).clamp(0, max) => u16);
    }

    fn panel_kind(&self, pane: Pane) -> panel::Kind {
        if self.focus == pane { panel::Kind::Focus } else { panel::Kind::Outer }
    }

    /// Move the table selection to `sel`; a new species starts its summary at the top.
    const fn select(&mut self, sel: usize) {
        if sel != self.sel {
            self.sel = sel;
            self.offset = 0;
        }
    }
}

impl Screen for SpeciesBrowser {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        let page = i32::from(self.measured.get().1.max(1));
        match key.code {
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Left | KeyCode::Right => {
                self.focus = self.focus.other();
                Action::None
            }
            KeyCode::Up if self.focus == Pane::Table => {
                self.select(self.sel.saturating_sub(1));
                Action::None
            }
            KeyCode::Down if self.focus == Pane::Table => {
                self.select((self.sel + 1).min(5));
                Action::None
            }
            KeyCode::Up => {
                self.scroll_by(-1);
                Action::None
            }
            KeyCode::Down => {
                self.scroll_by(1);
                Action::None
            }
            KeyCode::PageUp => {
                self.scroll_by(-page);
                Action::None
            }
            KeyCode::PageDown => {
                self.scroll_by(page);
                Action::None
            }
            KeyCode::Home => {
                self.scroll_by(i32::MIN.div_euclid(2));
                Action::None
            }
            KeyCode::End => {
                self.scroll_by(i32::MAX.div_euclid(2));
                Action::None
            }
            KeyCode::Char('s') => {
                // Keep the same species selected across the re-sort.
                let species = app.sim.as_ref().map(|sim| self.selected_species(sim));
                self.sort = self.sort.next();
                if let (Some(sim), Some(species)) = (app.sim.as_ref(), species) {
                    let order = sorted_indices(sim, self.sort);
                    self.sel = order.iter().position(|&i| i == species.index()).unwrap_or(0);
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
        let body_h = area.height.saturating_sub(1);
        let species = self.selected_species(sim);
        let table_h = TABLE_H.min(body_h);
        table(f, Rect::new(area.x, area.y, area.width, table_h), sim, self.sort, species, self.panel_kind(Pane::Table));

        let summary_area = Rect::new(area.x, area.y + table_h, area.width, body_h - table_h);
        let inner = panel::draw_with_hint(f, summary_area, &format!("Selected: {}", sim.roster().display_name(species)), "Enter for full detail", self.panel_kind(Pane::Summary));
        let ov = scroll::draw(f, summary_area, inner, self.offset, |buf, canvas| summary_body(buf, canvas, sim, species));
        self.measured.set((ov.content, inner.height));

        let updown = if self.focus == Pane::Table { "select" } else { "scroll" };
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("↑↓", updown), ("Tab", "panel"), ("Enter", "detail"), ("s", "sort"), ("Esc", "back")],
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

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        let n = app.params.species.len().max(1);
        match key.code {
            KeyCode::Esc => Action::Pop,
            KeyCode::Left | KeyCode::Up => {
                self.species = SpeciesId::from_index((self.species.index() + n - 1) % n);
                Action::None
            }
            KeyCode::Right | KeyCode::Down => {
                self.species = SpeciesId::from_index((self.species.index() + 1) % n);
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
            &format!("{} detail  {}", sim.roster().display_name(self.species), sim.time.clock_label()),
        );
    }
}

mod table;
mod summary;
mod histograms;
mod drift;
#[cfg(test)]
mod tests;
