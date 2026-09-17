//! S04a — the species table: rows, sorting and the totals line.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::Frame;
use crate::sim::species::TRAIT_ABBR;
use crate::sim::stats::SpeciesStats;
use crate::sim::{Genome, Kind, Sim, SpeciesId};
use crate::ui::screens::common::{sp, trait_color, two};
use crate::ui::style::SpeciesStyle;
use crate::widgets::Constraint::{Fill, Fixed};
use crate::widgets::{panel, Column, Component, Panel, Sparkline, Table, TableCell, TableRow, Text, TrendArrow};
use crate::{glyphs, theme};
use super::{SortCol, sorted_indices};

const fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Prey => "prey",
        Kind::Predator => "pred",
    }
}

/// The columns after the Marker: glyph, name, kind, the counts, the trend
/// strip and arrow, the twelve trait means and the diet.
fn columns() -> Vec<Column> {
    let mut cols = vec![
        Column::new(Fixed(2)),
        Column::titled("Species", Fixed(8)),
        Column::titled("Kind", Fixed(6)),
        Column::titled("Count", Fixed(5)).right(),
        Column::titled("Adults", Fixed(7)).right(),
        Column::titled("Juv", Fixed(6)).right(),
        Column::titled("Birth/d", Fixed(8)).right(),
        Column::titled("Death/d", Fixed(8)).right(),
        Column::titled("Sick", Fixed(6)).right(),
        Column::titled("Peak", Fixed(6)).right(),
        Column::titled("Gen", Fixed(5)).right(),
        Column::new(Fixed(2)),
        Column::titled("30-day trend", Fixed(14)),
        Column::new(Fixed(1)),
        Column::new(Fixed(1)),
    ];
    // Twelve genome columns (C8 made it eleven, Mutability twelve), so the
    // header comes from `TRAIT_ABBR` (never hand-typed) and the trend strip is
    // trimmed to 14 cells to keep the row inside 153.
    cols.extend(TRAIT_ABBR.iter().map(|a| Column::titled(a, Fixed(4)).right()));
    cols.push(Column::new(Fixed(3)));
    cols.push(Column::titled(" Diet", Fill(1)));
    cols
}

pub(super) fn table(f: &mut Frame<'_>, area: Rect, sim: &Sim, sort: SortCol, selected: SpeciesId, kind: panel::Kind) {
    let buf = f.buffer_mut();
    let inner = Panel::new("Species").info(Table::sort_info(sort.label())).kind(kind).render(buf, area);
    let order = sorted_indices(sim, sort);
    let rows: Vec<TableRow<'_>> = order.iter().map(|&i| table_row(sim, i)).collect();
    let sel = order.iter().position(|&i| SpeciesId::from_index(i) == selected);
    let cols = columns();
    Table::new(&cols, &rows).selected(sel).totals(table_totals(sim)).spaced(true).render(buf, inner);
}

/// One species row of the S04 table.
fn table_row(sim: &Sim, i: usize) -> TableRow<'static> {
    let s = &sim.species[i];
    let id = s.species;
    let absent = s.count == 0;
    let or_dim = |c| if absent { theme::DIM } else { c };
    let mut cells = vec![
        TableCell::Glyph(sim.roster().adult_glyph(id), or_dim(sim.roster().color(id))),
        TableCell::text(sim.roster().display_name(id)),
        TableCell::dim(kind_label(sim.roster().kind(id))),
        TableCell::text(s.count.to_string()),
        TableCell::text(s.adults.to_string()),
        TableCell::text(s.juveniles.to_string()),
        TableCell::styled(s.births_yesterday.to_string(), or_dim(theme::GOOD)),
        TableCell::styled(s.deaths_yesterday.to_string(), or_dim(theme::BAD)),
        // C7: living infected members, in SICK when any.
        if s.sick > 0 { TableCell::styled(s.sick.to_string(), theme::SICK) } else { TableCell::dim(s.sick.to_string()) },
        TableCell::text(s.peak.to_string()),
        TableCell::text(s.generation.to_string()),
        TableCell::Blank,
        if s.trend.is_empty() { TableCell::Blank } else { TableCell::widget(Sparkline::new(&s.trend).color(or_dim(sim.roster().color(id)))) },
        TableCell::widget(TrendArrow::new(&s.trend).bold()),
        TableCell::Blank,
    ];
    cells.extend(trait_cells(s, absent));
    cells.push(TableCell::Blank);
    cells.push(TableCell::dim(sim.roster().get(id).diet.as_str().to_owned()));
    TableRow::new(cells).absent(absent)
}

/// The per-trait genome columns.
fn trait_cells(s: &SpeciesStats, absent: bool) -> Vec<TableCell<'static>> {
    (0..Genome::LEN).map(|t| TableCell::styled(two(s.mean.0[t]), if absent { theme::DIM } else { trait_color(t) })).collect()
}

/// The totals row under the S04 table: the count under its column, the rest as a tail.
fn table_totals(sim: &Sim) -> TableRow<'static> {
    let prey: u32 = sim.species.iter().filter(|s| sim.roster().kind(s.species) == Kind::Prey).map(|s| s.count).sum();
    let pred: u32 = sim.species.iter().filter(|s| sim.roster().kind(s.species) == Kind::Predator).map(|s| s.count).sum();
    let births: u32 = (0..sim.roster().len()).map(|i| sim.births_today(i)).sum();
    let deaths: u32 = (0..sim.roster().len()).map(|i| sim.deaths_today(i)).sum();
    let tail = Text::spans(vec![
        sp(format!("    prey {}  pred {}  ratio {:.1}:1", prey, pred, crate::cast!(prey => f32) / crate::cast!(pred.max(1) => f32)), theme::dim_text()),
        sp(format!("   births {births}"), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        sp(format!("  deaths {deaths}"), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
        sp(format!("   net {:+} today", crate::cast!(births => i32) - crate::cast!(deaths => i32)), theme::text()),
        sp("     traits = species means x100;  /d = yesterday", theme::dim_text()),
    ]);
    TableRow::new([TableCell::Blank, TableCell::text("totals"), TableCell::Blank, TableCell::text((prey + pred).to_string())]).tail(tail)
}

/// Narrative line from the 30-day change (FR7).
pub(super) fn narrative(s: &SpeciesStats) -> (String, Style) {
    match s.change_pct() {
        Some(p) if p > 3.0 => (
            format!("{} {:+.0} % — births outpaced deaths over the last 30 days", glyphs::UP, p),
            Style::default().fg(theme::GOOD).bg(theme::PANEL_BG),
        ),
        Some(p) if p < -3.0 => (
            format!("{} {:+.0} % — deaths outpaced births over the last 30 days", glyphs::DOWN, p),
            Style::default().fg(theme::BAD).bg(theme::PANEL_BG),
        ),
        Some(_) => ("stable — births and deaths balanced over the last 30 days".to_string(), theme::dim_text()),
        None => ("no 30-day history yet".to_string(), theme::dim_text()),
    }
}
