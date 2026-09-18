//! S04b — drift, selection pressure and the population/disease sections.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::Frame;
use crate::sim::stats::SpeciesStats;
use crate::sim::species::{IDX_MATURITY, IDX_MUTABILITY};
use crate::sim::{Genome, Sim, SpeciesId, TRAIT_NAMES};
use crate::ui::screens::common::{arrow_color, delta_style, sp, trait_color, trend_arrow, two};
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};
use std::fmt::Write as _;

/// Selection-pressure lines (FR7): traits whose drift over the last 3 samples
/// exceeds ±0.02.
pub fn selection_pressure(s: &SpeciesStats) -> Vec<String> {
    let n = s.drift.len();
    let mut out = Vec::new();
    if n >= 2 {
        let a = &s.drift[n.saturating_sub(3)];
        let b = &s.drift[n - 1];
        let gens = b.0.saturating_sub(a.0).max(1);
        for t in 0..Genome::LEN {
            let d = b.1 .0[t] - a.1 .0[t];
            if d.abs() > 0.02 {
                out.push(pressure_note(t, d, gens));
            }
        }
    }
    if out.is_empty() {
        out.push(format!("{} no trait moving more than 0.02", glyphs::NOTE));
    }
    out
}

/// One selection-pressure sentence. C8: the maturity trait is the r/K dial, so
/// its line names which way the life history moved; Mutability's names which
/// way evolvability moved.
fn pressure_note(t: usize, d: f32, gens: u32) -> String {
    let dir = if d > 0.0 { "rising" } else { "falling" };
    let head = format!("{} {} {} ({:+.2} over {} generations)", glyphs::MUTATION, TRAIT_NAMES[t], dir, d, gens);
    let tail = match t {
        IDX_MATURITY if d > 0.0 => "later, larger litters (K)",
        IDX_MATURITY => "earlier, smaller litters (r)",
        IDX_MUTABILITY if d > 0.0 => "lineages loosening (more, larger mutations)",
        IDX_MUTABILITY => "genes settling (fewer, smaller mutations)",
        _ => return head,
    };
    format!("{head}: {tail}")
}

pub(super) fn drift(f: &mut Frame<'_>, area: Rect, sim: &Sim, id: SpeciesId) {
    let s = &sim.species[id.index()];
    let inner = panel::draw_with_hint(f, area, "Drift over generations", &format!("{} sampled generations", s.drift.len()), panel::Kind::Outer);
    let n = s.drift.len();
    let first_gen = s.drift.first().map_or(1, |d| d.0);
    {
        let y = inner.y;
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x, y, format!(" trait        g{first_gen:<4}"), 19, theme::dim_text());
        buf.set_stringn(inner.x + 21, y, "oldest", 6, theme::dim_text());
        buf.set_stringn(inner.x + 51, y, "newest", 6, theme::dim_text());
        buf.set_stringn(inner.x + 58, y, format!(" g{:<3} change", s.generation), 12, theme::dim_text());
    }
    let row = drift_trait_rows(f, inner, s, n, 1);
    let row = drift_means_table(f, inner, s, n, row + 1);
    population_section(f, inner, row + 2, sim, id, s);
}

/// One sparkline row per trait, oldest to newest sample.
fn drift_trait_rows(f: &mut Frame<'_>, inner: Rect, s: &SpeciesStats, n: usize, row: u16) -> u16 {
    let mut row = row;
    for t in 0..Genome::LEN {
        let color = trait_color(t);
        let y = inner.y + row;
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x, y, format!(" {:<12}", TRAIT_NAMES[t]), 13, Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
        if n > 0 {
            let first = s.drift[0].1 .0[t];
            let last = s.drift[n - 1].1 .0[t];
            let vals: Vec<u16> = s.drift.iter().map(|g| crate::cast!((g.1 .0[t] * 100.0).round() => u16)).collect();
            buf.set_stringn(inner.x + 14, y, format!("{first:.2}"), 4, theme::dim_text());
            let wide: Vec<u16> = vals.iter().flat_map(|v| [*v, *v, *v]).collect();
            bars::sparkline(buf, inner.x + 21, y, 36, &wide, color);
            buf.set_stringn(inner.x + 59, y, format!("{last:.2}"), 4, theme::text());
            buf.set_stringn(inner.x + 66, y, format!("{:+.2}", last - first), 5, delta_style(last - first));
        } else {
            buf.set_stringn(inner.x + 14, y, "no samples yet", 14, theme::dim_text());
        }
        row += 1;
    }
    row += 1;
    row
}

/// The per-generation mean table.
fn drift_means_table(f: &mut Frame<'_>, inner: Rect, s: &SpeciesStats, n: usize, row: u16) -> u16 {
    let mut row = row;
    panel::section(f, inner, row, "Per-generation means (x100)");
    row += 1;
    let mut hdr = String::from("              ");
    for g in &s.drift {
        let _ = write!(hdr, "g{:<3}", g.0);
    }
    util::line(f, inner, row, Line::from(sp(hdr, theme::dim_text())));
    row += 1;
    for t in 0..Genome::LEN {
        let mut spans = vec![sp(format!(" {:<12} ", TRAIT_NAMES[t]), theme::text())];
        for g in 0..n {
            let v = s.drift[g].1 .0[t];
            let prev = if g == 0 { v } else { s.drift[g - 1].1 .0[t] };
            let st = if g == 0 { theme::dim_text() } else { delta_style(v - prev) };
            spans.push(sp(format!("{:<4}", two(v)), st));
        }
        util::line(f, inner, row, Line::from(spans));
        row += 1;
    }
    util::line(f, inner, row, Line::from(sp(" green = rose vs previous sample, red = fell", theme::dim_text())));
    row += 2;
    row
}

/// The population summary above the disease section.
fn population_section(f: &mut Frame<'_>, inner: Rect, row: u16, sim: &Sim, id: SpeciesId, s: &SpeciesStats) {
    let mut row = row;
    row += 1;
    let arrow = trend_arrow(&s.trend);
    let arrow_st = Style::default().fg(arrow_color(arrow)).bg(theme::PANEL_BG);
    // C8: two lines instead of six. The per-field breakdown is already on S04a
    // (the S04 spec flags the duplication); the two extra genome rows need the
    // space to keep the Disease section on screen.
    util::line(f, inner, row, Line::from(vec![
        sp(
            format!(
                " {}  ({} adults, {} juveniles)  peak {} ({}%)  generation {}",
                s.count,
                s.adults,
                s.juveniles,
                s.peak,
                (s.count * 100).div_euclid(s.peak.max(1)),
                s.generation
            ),
            theme::text(),
        ),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" births today ", theme::dim_text()),
        sp(format!("{}", sim.births_today(id.index())), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        sp("  deaths today ", theme::dim_text()),
        sp(format!("{}", sim.deaths_today(id.index())), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
        sp(format!("   {arrow} over 30 days (see S04a)"), arrow_st),
    ]));
    row += 2;
    disease_section(f, inner, row, sim, id);
}

/// C7 (S04b): the species' current disease picture and the outbreaks that
/// touched it in the last three years, newest first (at most four).
fn disease_section(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, id: SpeciesId) {
    let s = &sim.species[id.index()];
    let si = id.index();
    panel::section(f, inner, row, "Disease");
    row += 1;
    let pct = (s.immune * 100).checked_div(s.count).unwrap_or(0);
    util::line(f, inner, row, Line::from(vec![
        sp(" active ", theme::dim_text()),
        sp(format!("{}", s.sick), Style::default().fg(if s.sick > 0 { theme::SICK } else { theme::TEXT }).bg(theme::PANEL_BG)),
        sp(format!(" {} immune ", glyphs::DOT), theme::dim_text()),
        sp(format!("{} ({}%)", s.immune, pct), Style::default().fg(theme::IMMUNE).bg(theme::PANEL_BG)),
        sp(format!(" {} disease deaths yesterday ", glyphs::DOT), theme::dim_text()),
        sp(format!("{}", s.deaths_disease_yesterday), Style::default().fg(if s.deaths_disease_yesterday > 0 { theme::BAD } else { theme::TEXT }).bg(theme::PANEL_BG)),
    ]));
    row += 1;
    let today = crate::cast!(sim.time.day_index() => u32);
    let year_len = 4 * sim.time.season_days;
    let horizon = today.saturating_sub(3 * year_len);
    let recent: Vec<_> = sim
        .disease
        .outbreaks
        .iter()
        .rev()
        .filter(|o| o.species_cases[si] > 0 && o.started_day >= horizon)
        .take(4)
        .collect();
    if recent.is_empty() {
        util::line(f, inner, row, Line::from(sp(" no outbreaks in the last 3 years", theme::dim_text())));
        return;
    }
    for o in recent {
        if row >= inner.height {
            break;
        }
        // Resistance at the end of an open outbreak is the current species mean.
        let end = if o.ended_day.is_some() { o.resist_at_end[si] } else { s.mean.resistance() };
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", glyphs::DISEASE), Style::default().fg(theme::SICK).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{:<8} ", crate::ui::screens::common::day_stamp(i64::from(o.started_day), sim.time.season_days)), theme::dim_text()),
            sp(format!("{:<10}", sim.disease.name(o.pathogen)), Style::default().fg(theme::SICK).bg(theme::PANEL_BG)),
            sp(format!(" {} cases, {} dead, ", o.species_cases[si], o.species_deaths[si]), theme::text()),
            sp(format!("resist .{:02} {} .{:02}", (crate::cast!((o.resist_at_start[si] * 100.0).round() => u32)).min(99), glyphs::RIGHT, (crate::cast!((end * 100.0).round() => u32)).min(99)), theme::dim_text()),
        ]));
        row += 1;
    }
}
