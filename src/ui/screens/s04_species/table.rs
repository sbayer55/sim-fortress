//! S04a — the species table: rows, sorting and the totals line.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use crate::sim::stats::SpeciesStats;
use crate::sim::species::TRAIT_ABBR;
use crate::sim::{Genome, Kind, Sim, SpeciesId};
use crate::ui::screens::common::{arrow_color, sp, trait_color, trend_arrow, two};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};
use super::{SortCol, sorted_indices};

const fn kind_label(id: SpeciesId) -> &'static str {
    match id.kind() {
        Kind::Prey => "prey",
        Kind::Predator => "pred",
    }
}

pub(super) fn table(f: &mut Frame<'_>, area: Rect, sim: &Sim, sort: SortCol, selected: SpeciesId) {
    let alive = sim.species.iter().filter(|s| s.count > 0).count();
    let prey_n = sim.species.iter().filter(|s| s.count > 0 && s.species.kind() == Kind::Prey).count();
    let inner = panel::draw_with_hint(f, area, "Species", &format!("{} species, {} prey / {} predator", alive, prey_n, alive - prey_n), panel::Kind::Outer);
    let dim = theme::dim_text();
    // C8: eleven genome columns, so the header is built from `TRAIT_ABBR` (never
    // hand-typed) and the sparkline slot is trimmed to keep the row inside 153.
    let trait_header: String = std::iter::once("  ".to_string()).chain(TRAIT_ABBR.iter().map(|a| format!("{a:>3} "))).collect();
    let header = vec![
        sp("   ", dim),
        sp(format!("{:<8}", "Species"), dim),
        sp(format!("{:<5}", "Kind"), dim),
        sp(format!("{:>6}", "Count"), dim),
        sp(format!("{:>7}", "Adults"), dim),
        sp(format!("{:>6}", "Juv"), dim),
        sp(format!("{:>8}", "Birth/d"), dim),
        sp(format!("{:>8}", "Death/d"), dim),
        sp(format!("{:>6}", "Sick"), dim),
        sp(format!("{:>6}", "Peak"), dim),
        sp(format!("{:>5}", "Gen"), dim),
        sp(format!("{:<18}", "  30-day trend"), dim),
        sp("  ", dim),
        sp(trait_header, dim),
        sp("   Diet", dim),
    ];
    util::line(f, inner, 0, Line::from(header));
    let mut row = 2u16;
    for i in sorted_indices(sim, sort) {
        table_row(f, inner, row, sim, i, selected);
        row += 1;
    }

    table_totals(f, inner, row, sim);
}

/// One species row of the S04 table, with its selection highlight and sparkline.
fn table_row(f: &mut Frame<'_>, inner: Rect, row: u16, sim: &Sim, i: usize, selected: SpeciesId) {
    let s = &sim.species[i];
    let id = s.species;
    let is_sel = id == selected;
    let absent = s.count == 0;
    let base = if is_sel {
        theme::selected()
    } else if absent {
        theme::dim_text()
    } else {
        theme::text()
    };
    let st = RowStyle { base, bg: if is_sel { theme::SELECT_BG } else { theme::PANEL_BG }, dimmed: if is_sel { base } else { theme::dim_text() }, absent };
    let arrow = trend_arrow(&s.trend);
    let marker = if is_sel { glyphs::PLAY } else { ' ' };
    let mut spans = vec![
            sp(marker.to_string(), Style::default().fg(theme::KEY).bg(st.bg).add_modifier(Modifier::BOLD)),
            sp(format!("{} ", id.glyph().to_ascii_uppercase()), Style::default().fg(if st.absent { theme::DIM } else { id.color() }).bg(st.bg).add_modifier(Modifier::BOLD)),
            sp(format!("{:<8}", id.name()), st.base),
            sp(format!("{:<5}", kind_label(id)), st.dimmed),
            sp(format!("{:>6}", s.count), st.base),
            sp(format!("{:>7}", s.adults), st.base),
            sp(format!("{:>6}", s.juveniles), st.base),
            sp(format!("{:>8}", s.births_yesterday), Style::default().fg(if st.absent { theme::DIM } else { theme::GOOD }).bg(st.bg)),
            sp(format!("{:>8}", s.deaths_yesterday), Style::default().fg(if st.absent { theme::DIM } else { theme::BAD }).bg(st.bg)),
            // C7: living infected members, in SICK when any.
            sp(format!("{:>6}", s.sick), if s.sick > 0 { Style::default().fg(theme::SICK).bg(st.bg) } else { st.dimmed }),
            sp(format!("{:>6}", s.peak), st.base),
            sp(format!("{:>5}", s.generation), st.base),
            sp(format!("{:<18}", ""), st.base), // sparkline slot
            sp(format!("{arrow} "), Style::default().fg(arrow_color(arrow)).bg(st.bg).add_modifier(Modifier::BOLD)),
            sp("  ", st.base),
        ];
    trait_spans(s, &st, &mut spans);
    spans.push(sp(format!("  {}", id.diet()), st.dimmed));
    util::line(f, inner, row, Line::from(spans));
    if is_sel {
        let buf = f.buffer_mut();
        for x in 0..inner.width {
            if let Some(c) = buf.cell_mut((inner.x + x, inner.y + row)) {
                c.set_bg(st.bg);
            }
        }
    }
    if !s.trend.is_empty() {
        bars::sparkline(f.buffer_mut(), inner.x + 71, inner.y + row, 14, &s.trend, if absent { theme::DIM } else { id.color() });
    }
}

/// Colours shared by the cells of one species table row.
struct RowStyle {
    base: Style,
    bg: Color,
    dimmed: Style,
    absent: bool,
}

/// The per-trait genome columns.
fn trait_spans(s: &SpeciesStats, st: &RowStyle, spans: &mut Vec<Span<'static>>) {
    for t in 0..Genome::LEN {
        let style = if st.absent { Style::default().fg(theme::DIM).bg(st.bg) } else { Style::default().fg(trait_color(t)).bg(st.bg) };
        spans.push(sp(format!("{:>3} ", two(s.mean.0[t])), style));
    }
}

/// The totals row under the S04 table.
fn table_totals(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim) {
    // Totals row.
    row += 1;
    let prey: u32 = sim.species.iter().filter(|s| s.species.kind() == Kind::Prey).map(|s| s.count).sum();
    let pred: u32 = sim.species.iter().filter(|s| s.species.kind() == Kind::Predator).map(|s| s.count).sum();
    let births: u32 = (0..6).map(|i| sim.births_today(i)).sum();
    let deaths: u32 = (0..6).map(|i| sim.deaths_today(i)).sum();
    util::line(f, inner, row, Line::from(vec![
        sp(format!("   {:<14}", "totals"), theme::label()),
        sp(format!("{:>6}", prey + pred), theme::text()),
        sp(format!("   prey {}  pred {}  ratio {:.1}:1", prey, pred, crate::cast!(prey => f32) / crate::cast!(pred.max(1) => f32)), theme::dim_text()),
        sp(format!("   births {births}"), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        sp(format!("  deaths {deaths}"), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
        sp(format!("   net {:+} today", crate::cast!(births => i32) - crate::cast!(deaths => i32)), theme::text()),
        sp("     traits = species means x100;  /d = yesterday", theme::dim_text()),
    ]));
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
