//! S05d — infections and resistance (C7 FR13).

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::Frame;
use crate::sim::{Outbreak, PathogenId};
use crate::sim::{Sim, SpeciesId};
use crate::ui::screens::common::{day_stamp, sp};
use crate::ui::style::SpeciesStyle;
use crate::widgets::chart::{Band, Chart, Series};
use crate::widgets::{panel, util, Component};
use crate::{glyphs, theme};
use super::{Window, round_up};

/// Line colours for the eight pathogen slots (roster first, strains appended).
/// Lower-case delta (CP437 0xEB); the capital Δ is not in CP437.
const DELTA: char = 'δ';

const PATHOGEN_COLORS: [Color; 8] = [theme::SICK, theme::WARN, theme::MAGENTA, theme::INFO, theme::ACCENT, theme::ROSE, theme::TAN, theme::PREY];

const fn slot_color(slot: usize) -> Color {
    PATHOGEN_COLORS[slot % PATHOGEN_COLORS.len()]
}

/// Pathogen slots with any active case inside the window, with their series.
fn case_series(w: &Window<'_>) -> Vec<(usize, Vec<f32>)> {
    (0..8)
        .filter_map(|slot| {
            let v: Vec<f32> = w.samples.iter().map(|s| crate::cast!(s.active_by_pathogen[slot] => f32)).collect();
            if v.iter().any(|&x| x > 0.0) { Some((slot, v)) } else { None }
        })
        .collect()
}

/// Species with a nonzero population inside the window, with their mean
/// Resistance series.
fn resistance_series(w: &Window<'_>, roster: &crate::sim::Roster) -> Vec<(SpeciesId, Vec<f32>)> {
    roster
        .ids()
        .filter(|id| w.samples.iter().any(|s| s.population[id.index()] > 0))
        .map(|id| (id, w.samples.iter().map(|s| s.genome_mean[id.index()].resistance()).collect()))
        .collect()
}

/// The species an outbreak hit hardest (most cases), or `None` before any case.
fn outbreak_host(o: &Outbreak) -> Option<SpeciesId> {
    let (i, n) = o.species_cases.iter().enumerate().max_by_key(|(i, n)| (**n, std::cmp::Reverse(*i)))?;
    if *n == 0 { None } else { Some(SpeciesId::from_index(i)) }
}

/// Several half-block series over one y scale, with epidemic windows shaded;
/// `reference` rows are drawn as dim dotted lines behind the series. Thin
/// adapter over [`Chart`].
fn multi_line_chart(f: &mut Frame<'_>, area: Rect, w: &Window<'_>, series: &[(Vec<f32>, Color)], reference: &[(f32, Color)], y_max: f32, y_label: impl Fn(f32) -> String) {
    let labels = |i: usize| w.day_label(i);
    let mut chart = Chart::new().y_max(y_max).y_label(&y_label).x_label(&labels);
    for (values, color) in series {
        chart = chart.series(Series::new(values).color(*color));
    }
    for &(v, color) in reference {
        chart = chart.reference(v, color);
    }
    chart.band(Band::new(&w.epidemic).color(theme::SICK).label(glyphs::DISEASE, "epidemic")).render(f.buffer_mut(), area);
}

pub(super) fn infection_chart(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw_with_hint(f, area, "Infections — cases and resistance", &format!("last {} days", w.len()), panel::Kind::Outer);
    let half = (inner.height.saturating_sub(1)).div_euclid(2);
    let upper = Rect::new(inner.x, inner.y, inner.width, half);
    let lower = Rect::new(inner.x, inner.y + half + 1, inner.width, inner.height - half - 1);

    // Top: active cases per pathogen slot.
    let cases = case_series(w);
    let mut spans = vec![sp(" Active cases", Style::default().fg(theme::SICK).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))];
    if cases.is_empty() {
        spans.push(sp("   no infections in the window", theme::dim_text()));
    }
    for (slot, _) in &cases {
        spans.push(sp(format!("   {}{} ", glyphs::HALF_UPPER, glyphs::HALF_LOWER), Style::default().fg(slot_color(*slot)).bg(theme::PANEL_BG)));
        spans.push(sp(sim.disease.name(PathogenId(crate::cast!(*slot => u8))).to_string(), theme::text()));
    }
    util::line(f, upper, 0, Line::from(spans));
    let max_cases = cases.iter().flat_map(|(_, v)| v.iter().copied()).fold(0.0f32, f32::max);
    let case_lines: Vec<(Vec<f32>, Color)> = cases.iter().map(|(slot, v)| (v.clone(), slot_color(*slot))).collect();
    multi_line_chart(f, Rect::new(upper.x, upper.y + 1, upper.width, upper.height - 1), w, &case_lines, &[], round_up(max_cases, 10.0), |v| format!("{v:.0}"));

    // Bottom: mean Resistance per species with the base value as a reference.
    panel::section(f, inner, half, "Mean Resistance (host species)");
    let resist = resistance_series(w, sim.roster());
    let mut spans = vec![sp(" 0..1", theme::dim_text())];
    for &(id, _) in &resist {
        spans.push(sp(format!("   {}{} ", glyphs::HALF_UPPER, glyphs::HALF_LOWER), Style::default().fg(sim.roster().color(id)).bg(theme::PANEL_BG)));
        spans.push(sp(sim.roster().plural(id).to_string(), theme::text()));
    }
    spans.push(sp(format!("   {} base", glyphs::DOT), theme::dim_text()));
    util::line(f, lower, 0, Line::from(spans));
    let resist_lines: Vec<(Vec<f32>, Color)> = resist.iter().map(|(id, v)| (v.clone(), sim.roster().color(*id))).collect();
    let reference: Vec<(f32, Color)> = resist.iter().map(|(id, _)| (sim.roster().base_genome(*id).resistance(), sim.roster().color(*id))).collect();
    multi_line_chart(f, Rect::new(lower.x, lower.y + 1, lower.width, lower.height - 1), w, &resist_lines, &reference, 1.0, |v| format!("{v:.2}"));
}

pub(super) fn infection_sidebar(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw(f, area, "Outbreaks", panel::Kind::Outer);
    let mut row = 0u16;
    let today = crate::cast!(sim.time.day_index() => u32);
    let recent: Vec<&Outbreak> = sim.disease.outbreaks.iter().rev().take(8).collect();
    if recent.is_empty() {
        util::line(f, inner, row, Line::from(sp(" no outbreaks yet", theme::dim_text())));
        row += 1;
    }
    for o in recent {
        let strain = sim.disease.pathogen(o.pathogen).is_some_and(crate::sim::disease::Pathogen::is_strain);
        let color = if strain { theme::MAGENTA } else { theme::SICK };
        let name = sim.disease.name(o.pathogen).to_string();
        let stamp = day_stamp(i64::from(o.started_day), w.season_days);
        let tag = if o.epidemic { " EPIDEMIC" } else { "" };
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", glyphs::DISEASE), Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{name:<14}"), Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{stamp:<9}"), theme::dim_text()),
            sp(tag, Style::default().fg(theme::SICK).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        let (host_glyph, delta) = match outbreak_host(o) {
            Some(id) => {
                let i = id.index();
                let delta = if o.ended_day.is_none() { "open".to_string() } else { format!("{:+.2}", o.resist_at_end[i] - o.resist_at_start[i]).replace("0.", ".") };
                (format!("{} ", sim.roster().adult_glyph(id)), delta)
            }
            None => ("- ".to_string(), "open".to_string()),
        };
        let _ = today;
        util::line(f, inner, row, Line::from(vec![
            sp(format!("   {host_glyph}"), Style::default().fg(outbreak_host(o).map_or(theme::DIM, |id| sim.roster().color(id))).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("cases {:<5} dead {:<5} {}resist {}", o.cases, o.deaths, DELTA, delta), theme::text()),
        ]));
        row += 1;
    }
    row += 1;

    panel::section(f, inner, row, "Pathogens");
    row += 1;
    let cases = case_series(w);
    if cases.is_empty() {
        util::line(f, inner, row, Line::from(sp(" none active in the window", theme::dim_text())));
        row += 1;
    }
    for (slot, v) in &cases {
        let id = PathogenId(crate::cast!(*slot => u8));
        let strain = sim.disease.pathogen(id).is_some_and(crate::sim::disease::Pathogen::is_strain);
        let peak = crate::cast!(v.iter().copied().fold(0.0f32, f32::max) => u32);
        let now = crate::cast!(v.last().copied().unwrap_or(0.0) => u32);
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {}{} ", glyphs::HALF_UPPER, glyphs::HALF_LOWER), Style::default().fg(slot_color(*slot)).bg(theme::PANEL_BG)),
            sp(format!("{:<14}", sim.disease.name(id)), theme::text()),
            sp(format!("now {now:<4} peak {peak:<4}"), theme::dim_text()),
            sp(if strain { "strain" } else { "" }, Style::default().fg(theme::MAGENTA).bg(theme::PANEL_BG)),
        ]));
        row += 1;
    }
    row += 1;

    panel::section(f, inner, row, "Legend");
    row += 1;
    for (glyph, desc, color) in [
        ("▀▄", "active cases per pathogen (top)", theme::SICK),
        ("▀▄", "mean Resistance per species (bottom)", theme::PREY),
        ("·", "base Resistance of the species", theme::DIM),
        ("░", "epidemic window", theme::SICK),
    ] {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {glyph:<3}"), Style::default().fg(color).bg(theme::PANEL_BG)),
            sp(desc, theme::text()),
        ]));
        row += 1;
    }
    row += 1;
    panel::section(f, inner, row, "Keys");
    row += 1;
    util::line(f, inner, row, Line::from(sp(" [+/-] zoom 60/240/720d", theme::dim_text())));
}
