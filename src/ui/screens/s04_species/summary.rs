//! S04a — the selected-species summary panel and its map/interaction sections.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::buffer::Buffer;
use crate::sim::stats::SpeciesStats;
use crate::sim::{Genome, Kind, Sim, SpeciesId, TRAIT_NAMES};
use crate::ui::screens::common::{arrow_color, delta_style, downsample, sp, trait_color, trend_arrow};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};
use super::table::narrative;

/// Narrowest right column worth placing beside the left one; below that the
/// history column stacks under the identity column instead.
const LEFT_W: u16 = 64;
const MIN_RIGHT_W: u16 = 40;
/// Cells the plot reserves on its left for the max/min axis labels.
const PLOT_GUTTER: u16 = 6;
/// Widest the population plot grows (one 240-day series over 100 buckets).
const PLOT_MAX_COLS: u16 = 100;

/// The summary body, drawn into a scroll canvas; returns the rows it used.
///
/// The left column (identity, genome, interactions, notable individuals) and
/// the right one (population history, habitat) sit side by side when the
/// canvas is at least `LEFT_W + 1 + MIN_RIGHT_W` wide, and stack otherwise.
pub(super) fn summary_body(buf: &mut Buffer, canvas: Rect, sim: &Sim, id: SpeciesId) -> u16 {
    let s = &sim.species[id.index()];
    let side_by_side = canvas.width >= LEFT_W + 1 + MIN_RIGHT_W;
    let left_w = if side_by_side { LEFT_W } else { canvas.width };
    let left = Rect::new(canvas.x, canvas.y, left_w, canvas.height);
    let left_rows = summary_left(buf, left, sim, id, s);
    let right = if side_by_side {
        Rect::new(canvas.x + LEFT_W + 1, canvas.y, canvas.width - LEFT_W - 1, canvas.height)
    } else {
        let y = canvas.y + (left_rows + 1).min(canvas.height);
        Rect::new(canvas.x, y, canvas.width, canvas.height.saturating_sub(left_rows + 1))
    };
    let right_rows = summary_history(buf, right, sim, id, s);
    if side_by_side {
        left_rows.max(right_rows)
    } else {
        (left_rows + 1 + right_rows).min(canvas.height)
    }
}

/// Identity, genome, interactions and notable individuals; returns rows used.
fn summary_left(buf: &mut Buffer, left: Rect, sim: &Sim, id: SpeciesId, s: &SpeciesStats) -> u16 {
    let mut row = 0u16;
    util::line_in(buf, left, row, Line::from(vec![
        sp(format!(" {} ", id.glyph().to_ascii_uppercase()), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(id.plural(), theme::title()),
        sp(format!("   {}   diet: {}", if id.kind() == Kind::Prey { "prey" } else { "predator" }, id.diet()), theme::text()),
    ]));
    row += 1;
    util::line_in(buf, left, row, Line::from(vec![
        sp(format!(" {} alive  {} adults  {} juveniles  generation {}  peak {}", s.count, s.adults, s.juveniles, s.generation, s.peak), theme::dim_text()),
    ]));
    row += 1;
    // C8 follow-up: the group sizes the cohesion rule holds together, on the
    // blank row the counts line used to leave (the left column has no spare
    // rows; the full size distribution is S05e).
    summary_group_line(buf, left, row, sim, id);
    row += 1;
    panel::section_in(buf, left, row, "Base genome vs current mean");
    row += 1;
    util::line_in(buf, left, row, Line::from(sp(" trait        base   current            delta   spread", theme::dim_text())));
    row += 1;
    row = summary_genome(buf, left, row, id, s);
    row = summary_interactions(buf, left, row, sim, id);
    summary_notable(buf, left, row, sim, id)
}

/// Base genome vs the current mean, per trait.
fn summary_genome(buf: &mut Buffer, left: Rect, mut row: u16, id: SpeciesId, s: &SpeciesStats) -> u16 {
    let base = id.base_genome();
    for t in 0..Genome::LEN {
        let b = base.0[t];
        let m = s.mean.0[t];
        let d = m - b;
        let color = trait_color(t);
        let y = left.y + row;
        buf.set_stringn(left.x, y, format!(" {:<12}", TRAIT_NAMES[t]), 13, theme::text());
        buf.set_stringn(left.x + 13, y, format!("{b:.2}"), 4, theme::dim_text());
        buf.set_stringn(left.x + 20, y, format!("{m:.2}"), 4, theme::text());
        bars::bar(buf, left.x + 25, y, 14, m, color);
        let bx = left.x + 26 + crate::cast!((b * 11.0).round() => u16);
        buf.set_stringn(bx, y, glyphs::V_LINE.to_string(), 1, Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG));
        buf.set_stringn(left.x + 42, y, format!("{d:+.2}"), 5, delta_style(d));
        bars::range(buf, left.x + 49, y, 13, s.min.0[t], m, s.max.0[t], color);
        row += 1;
    }
    util::line_in(buf, left, row, Line::from(vec![
        sp(format!(" {} base marker   spread = min/mean/max across living {}", glyphs::V_LINE, id.plural()), theme::dim_text()),
    ]));
    row += 2;
    row
}

/// Kill shares, competition and pathogens/parasites for the selected species.
fn summary_interactions(buf: &mut Buffer, left: Rect, row: u16, sim: &Sim, id: SpeciesId) -> u16 {
    panel::section_in(buf, left, row, "Interactions");
    let kills = kill_matrix(sim);
    if id.kind() == Kind::Prey {
        prey_interactions(buf, left, row + 1, sim, id, &kills)
    } else {
        predator_interactions(buf, left, row + 1, sim, id, &kills)
    }
}

/// Total kills per `[predator][prey]` pair, living and carcasses alike.
fn kill_matrix(sim: &Sim) -> [[u32; 6]; 6] {
    let mut kills = [[0u32; 6]; 6]; // [predator][prey]
    for c in sim.creatures.living().chain(sim.creatures.carcasses()) {
        if c.species.kind() == Kind::Predator {
            for (i, k) in c.kills_by_species.iter().enumerate() {
                kills[c.species.index()][i] += k;
            }
        }
    }
    kills
}

/// Prey view: who eats it, and what it competes with for grass.
fn prey_interactions(buf: &mut Buffer, left: Rect, row: u16, sim: &Sim, id: SpeciesId, kills: &[[u32; 6]; 6]) -> u16 {
    let mut row = row;
        let by_pred: u32 = SpeciesId::ALL.iter().map(|p| kills[p.index()][id.index()]).sum();
        let hunters: Vec<String> = SpeciesId::ALL
            .iter()
            .filter(|p| p.kind() == Kind::Predator && (sim.params.predation.preference(**p, id) > 0.0 || kills[p.index()][id.index()] > 0))
            .map(|p| {
                if by_pred >= 5 {
                    format!("{} {} {:.0}%", p.glyph().to_ascii_uppercase(), p.name(), crate::cast!(kills[p.index()][id.index()] => f32) / crate::cast!(by_pred => f32) * 100.0)
                } else {
                    format!("{} {}", p.glyph().to_ascii_uppercase(), p.name())
                }
            })
            .collect();
        if hunters.is_empty() {
            util::line_in(buf, left, row, Line::from(vec![sp(" eaten by: ", theme::dim_text()), sp("none", theme::text())]));
        } else {
            util::line_in(buf, left, row, Line::from(vec![sp(" eaten by: ", theme::dim_text()), sp(hunters.join(" and "), theme::text())]));
        }
        row += 1;
        let others: Vec<String> = SpeciesId::ALL
            .iter()
            .filter(|o| o.kind() == Kind::Prey && **o != id)
            .map(|o| format!("{} {}", o.glyph().to_ascii_uppercase(), o.name()))
            .collect();
        util::line_in(buf, left, row, Line::from(vec![
            sp(" competes with ", theme::dim_text()),
            sp(others.join(" and "), theme::text()),
            sp(" for grass", theme::dim_text()),
        ]));
        row += 1;
    susceptibility_lines(buf, left, row, sim, id)
}

/// Predator view: what it hunts, and its rivals.
fn predator_interactions(buf: &mut Buffer, left: Rect, row: u16, sim: &Sim, id: SpeciesId, kills: &[[u32; 6]; 6]) -> u16 {
    let mut row = row;
        let total: u32 = kills[id.index()].iter().sum();
        let prey: Vec<String> = SpeciesId::ALL
            .iter()
            .filter(|p| p.kind() == Kind::Prey && (sim.params.predation.preference(id, **p) > 0.0 || kills[id.index()][p.index()] > 0))
            .map(|p| {
                let share = if total >= 5 {
                    crate::cast!(kills[id.index()][p.index()] => f32) / crate::cast!(total => f32)
                } else {
                    sim.params.predation.preference(id, *p)
                };
                format!("{} {} {:.0}%", p.glyph().to_ascii_uppercase(), p.name(), share * 100.0)
            })
            .collect();
        util::line_in(buf, left, row, Line::from(vec![
            sp(" hunts ", theme::dim_text()),
            sp(if prey.is_empty() { "nothing".to_string() } else { prey.join(", ") }, theme::text()),
            sp(if total >= 5 { format!("  ({total} kills)") } else { "  (preference)".to_string() }, theme::dim_text()),
        ]));
        row += 1;
        // Rivals share at least one prey species.
        let rivals: Vec<String> = SpeciesId::ALL
            .iter()
            .filter(|o| {
                o.kind() == Kind::Predator
                    && **o != id
                    && SpeciesId::ALL.iter().any(|p| sim.params.predation.preference(id, *p) > 0.0 && sim.params.predation.preference(**o, *p) > 0.0)
            })
            .map(|o| format!("{} {}", o.glyph().to_ascii_uppercase(), o.name()))
            .collect();
        util::line_in(buf, left, row, Line::from(vec![
            sp(" competes with ", theme::dim_text()),
            sp(if rivals.is_empty() { "no one".to_string() } else { rivals.join(" and ") }, theme::text()),
            sp(" for prey", theme::dim_text()),
        ]));
        row += 1;
    susceptibility_lines(buf, left, row, sim, id)
}

/// C7: pathogens that can infect this species, and the mean worm load.
fn susceptibility_lines(buf: &mut Buffer, left: Rect, row: u16, sim: &Sim, id: SpeciesId) -> u16 {
    let mut row = row;
    // C7: pathogens that can infect this species, and the mean worm load.
    let pathogens: Vec<&str> = sim.disease.pathogens.iter().filter(|p| !p.extinct && p.host(id) > 0.0).map(crate::sim::disease::Pathogen::name).collect();
    util::line_in(buf, left, row, Line::from(vec![
        sp(" susceptible to: ", theme::dim_text()),
        if pathogens.is_empty() {
            sp("none", theme::dim_text())
        } else {
            sp(pathogens.join(", "), Style::default().fg(theme::SICK).bg(theme::PANEL_BG))
        },
    ]));
    row += 1;
    let (load_sum, load_n) = sim.creatures.living().filter(|c| c.species == id).fold((0.0f32, 0u32), |(a, n), c| (a + c.parasite_load, n + 1));
    let load = if load_n > 0 { load_sum / crate::cast!(load_n => f32) } else { 0.0 };
    util::line_in(buf, left, row, Line::from(vec![
        sp(format!(" {} worms: mean load ", glyphs::PARASITE), theme::dim_text()),
        sp(format!(".{:02}", (crate::cast!((load * 100.0).round() => u32)).min(99)), Style::default().fg(if load >= 0.2 { theme::WARN } else { theme::TEXT }).bg(theme::PANEL_BG)),
    ]));
    row += 2;
    row
}

/// C8 follow-up: one line on the group sizes the cohesion rule holds together —
/// how many herds/packs, their mean and max size, and how much of the species
/// is grouped. The full size distribution is S05e. Fits the blank row under the
/// counts line, so the left column below it does not move.
fn summary_group_line(buf: &mut Buffer, left: Rect, row: u16, sim: &Sim, id: SpeciesId) {
    let g = &sim.group_stats;
    let i = id.index();
    if sim.species[i].count == 0 {
        util::line_in(buf, left, row, Line::from(sp(" groups: none living", theme::dim_text())));
        return;
    }
    let word = if id.kind() == Kind::Prey { "herd" } else { "pack" };
    if g.groups[i] == 0 {
        util::line_in(buf, left, row, Line::from(sp(format!(" groups: none forming — all {} alone", sim.species[i].count), theme::dim_text())));
        return;
    }
    let noun = if g.groups[i] == 1 { word.to_string() } else { format!("{word}s") };
    util::line_in(buf, left, row, Line::from(vec![
        sp(" groups: ", theme::dim_text()),
        sp(format!("{} {noun}", g.groups[i]), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(
            format!(
                ", mean {:.1}, max {}, {:.0}% grouped, {} alone",
                g.mean[i],
                g.max[i],
                g.grouped_share(i, sim.species[i].count) * 100.0,
                g.solo(i),
            ),
            theme::dim_text(),
        ),
    ]));
}

/// Notable living individuals of the selected species.
fn summary_notable(buf: &mut Buffer, left: Rect, mut row: u16, sim: &Sim, id: SpeciesId) -> u16 {
    panel::section_in(buf, left, row, "Notable individuals");
    row += 1;
    let day = sim.time.day_index();
    let mut members: Vec<&crate::sim::Creature> = sim.creatures.living().filter(|c| c.species == id).collect();
    members.sort_by(|a, b| b.offspring.cmp(&a.offspring).then(b.age_days(day).cmp(&a.age_days(day))).then(a.id.cmp(&b.id)));
    if members.is_empty() {
        util::line_in(buf, left, row, Line::from(sp(" none living", theme::dim_text())));
    }
    for c in members.iter().take(5) {
        if row >= left.height {
            break;
        }
        util::line_in(buf, left, row, Line::from(vec![
            sp(format!(" {} ", if c.adult { id.glyph().to_ascii_uppercase() } else { id.glyph() }), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{:<9}{:<7}", c.name_str(), c.tag()), theme::text()),
            sp(format!("{:>3} offspring  gen {:<3} {:>4} days  ", c.offspring, c.generation, c.age_days(day)), theme::dim_text()),
            sp(sim.world.region_name(c.x, c.y).to_string(), theme::dim_text()),
        ]));
        row += 1;
    }
    row
}

/// The right-hand 240-day population history plot; returns rows used.
fn summary_history(buf: &mut Buffer, right: Rect, sim: &Sim, id: SpeciesId, s: &SpeciesStats) -> u16 {
    if right.width == 0 || right.height == 0 {
        return 0;
    }
    // ---- right: population history
    let mut row = 0u16;
    panel::section_in(buf, right, row, "Population, last 240 days");
    row += 1;
    let samples = sim.series.samples();
    let start = samples.len().saturating_sub(240);
    let series: Vec<f32> = samples[start..].iter().map(|x| crate::cast!(x.population[id.index()] => f32)).collect();
    let drought: Vec<bool> = samples[start..].iter().map(|x| x.drought_regions >= 2).collect();
    // The plot never grows past its column: one bucket per cell after the
    // axis-label gutter, capped at 100 buckets for the 240-day series.
    let cols = crate::cast!(right.width.saturating_sub(PLOT_GUTTER).clamp(1, PLOT_MAX_COLS) => usize);
    let data = downsample(&series, cols);
    let max = f32::from(*data.iter().max().unwrap_or(&1));
    let min = f32::from(*data.iter().min().unwrap_or(&0));
    let rows = 6u16;
    history_plot(buf, right, row, id, &data, &drought, cols, rows, max, min);
    row += rows;
    let span_days = series.len().max(1);
    history_axis(buf, right, row, cols, span_days, drought.iter().any(|&d| d));
    row += 2;
    util::line_in(buf, right, row, Line::from(sp(" 30-day trend ", theme::dim_text())));
    if !s.trend.is_empty() {
        bars::sparkline(buf, right.x + 14, right.y + row, 30, &s.trend, id.color());
    }
    let a = trend_arrow(&s.trend);
    buf.set_stringn(
        right.x + 45,
        right.y + row,
        format!(" {} {}", a, if a == glyphs::UP { "growing" } else if a == glyphs::DOWN { "declining" } else { "stable" }),
        14,
        Style::default().fg(arrow_color(a)).bg(theme::PANEL_BG),
    );
    row += 2;
    let first = series.first().copied().unwrap_or(0.0);
    let last = crate::cast!(s.count => f32);
    let lo = series.iter().copied().fold(f32::MAX, f32::min);
    let hi = series.iter().copied().fold(0.0f32, f32::max);
    let stats: Vec<(String, String, Style)> = vec![
        (format!("{span_days} days ago"), format!("{first:.0}"), theme::text()),
        ("today".into(), format!("{}", s.count), theme::text()),
        ("change".into(), format!("{:+.0} ({:+.0}%)", last - first, (last - first) / first.max(1.0) * 100.0), delta_style(last - first)),
        ("low / high".into(), format!("{:.0} / {:.0}", if lo == f32::MAX { 0.0 } else { lo }, hi), theme::text()),
        ("births today".into(), format!("{}  (yesterday {})", sim.births_today(id.index()), s.births_yesterday), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        ("deaths today".into(), format!("{}  (yesterday {})", sim.deaths_today(id.index()), s.deaths_yesterday), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
        (
            "first birth".into(),
            match s.first_birth_day {
                Some(d) => crate::ui::screens::common::day_stamp(i64::from(d), sim.time.season_days),
                None => "none yet".into(),
            },
            theme::text(),
        ),
    ];
    for (k, v, st) in stats {
        util::line_in(buf, right, row, Line::from(vec![sp(format!(" {k:<14}"), theme::dim_text()), sp(v, st)]));
        row += 1;
    }
    row += 1;
    let (text, st) = narrative(s);
    util::line_in(buf, right, row, Line::from(vec![sp(format!(" {} ", glyphs::NOTE), theme::label()), sp(text, st)]));
    row += 2;
    habitat_section(buf, right, row, sim, id)
}

/// The `D-240 … drought bands shaded … today` axis line under the plot,
/// laid out over the plot's own width.
fn history_axis(buf: &mut Buffer, right: Rect, row: u16, cols: usize, span_days: usize, drought: bool) {
    if row >= right.height {
        return;
    }
    let y = right.y + row;
    let plot_w = crate::cast!(cols => u16);
    let x0 = right.x + PLOT_GUTTER;
    let start = format!("D-{span_days}");
    let start_w = crate::cast!(start.len() => u16);
    let end = "today";
    let end_w = crate::cast!(end.len() => u16);
    buf.set_stringn(x0, y, &start, cols, theme::dim_text());
    if plot_w > start_w + end_w + 1 {
        buf.set_stringn(x0 + plot_w - end_w, y, end, cols, theme::dim_text());
    }
    let note = "drought bands shaded";
    let note_w = crate::cast!(note.len() => u16);
    if drought && plot_w > start_w + end_w + note_w + 4 {
        buf.set_stringn(x0 + (plot_w - note_w).div_euclid(2), y, note, cols, Style::default().fg(theme::WARN).bg(theme::PANEL_BG));
    }
}

/// The bar-style population history plot.
#[allow(clippy::too_many_arguments)]
fn history_plot(buf: &mut Buffer, right: Rect, row: u16, id: SpeciesId, data: &[u16], drought: &[bool], cols: usize, rows: u16, max: f32, min: f32) {
    if row + rows > right.height {
        return;
    }
    {
        for (i, v) in data.iter().enumerate() {
            let t = if max > min { (f32::from(*v) - min) / (max - min) } else { 0.5 };
            let halves = crate::cast!((t * f32::from(rows) * 2.0).round() => u16);
            let di = (i * drought.len().max(1)).div_euclid(cols);
            let dry = drought.get(di).copied().unwrap_or(false);
            for r in 0..rows {
                let y = right.y + row + rows - 1 - r;
                let level = halves.saturating_sub(r * 2);
                let ch = if level >= 2 {
                    glyphs::FULL_BLOCK
                } else if level == 1 {
                    glyphs::HALF_LOWER
                } else {
                    glyphs::SHADE_1
                };
                let color = if level >= 1 { id.color() } else { theme::dim(theme::DIM, 0.6) };
                let bg = if dry { theme::dim(theme::WARN, 0.78) } else { theme::PANEL_BG };
                buf.set_stringn(right.x + PLOT_GUTTER + crate::cast!(i => u16), y, ch.to_string(), 1, Style::default().fg(color).bg(bg));
            }
        }
        buf.set_stringn(right.x, right.y + row, format!("{:>4} ", crate::cast!(max => u32)), 5, theme::dim_text());
        buf.set_stringn(right.x, right.y + row + rows - 1, format!("{:>4} ", crate::cast!(min => u32)), 5, theme::dim_text());
    }

}

/// Living individuals per region, two columns at a time; returns rows used.
fn habitat_section(buf: &mut Buffer, right: Rect, mut row: u16, sim: &Sim, id: SpeciesId) -> u16 {
    // The section header the cleanup wave dropped (S04 content requirement 14).
    panel::section_in(buf, right, row, "Habitat (living individuals by region)");
    row += 1;
    let mut per_region: Vec<(&str, usize)> = sim.world.regions.iter().map(|r| (r.0.as_str(), 0usize)).collect();
    for c in sim.creatures.living().filter(|c| c.species == id) {
        let ri = sim.world.region_index(c.x, c.y);
        if let Some(e) = per_region.get_mut(ri) {
            e.1 += 1;
        }
    }
    per_region.sort_by(|a, b| b.1.cmp(&a.1));
    let max = crate::cast!(per_region.first().map_or(1, |e| e.1).max(1) => f32);
    // Two columns of 37-cell entries when they fit, one otherwise.
    let ncols = if right.width >= 2 * 37 { 2usize } else { 1 };
    let half = right.width.div_euclid(crate::cast!(ncols => u16));
    let mut last = row;
    for (i, (name, n)) in per_region.iter().enumerate() {
        let col = crate::cast!((i % ncols) => u16);
        let r = row + crate::cast!((i.div_euclid(ncols)) => u16);
        if r >= right.height {
            break;
        }
        let x = right.x + 1 + col * half;
        let y = right.y + r;
        buf.set_stringn(x, y, format!("{name:<17}"), 17, theme::text());
        bars::bar(buf, x + 17, y, 14, crate::cast!(*n => f32) / max, id.color());
        buf.set_stringn(x + 32, y, format!("{n:>4}"), 4, theme::dim_text());
        last = r + 1;
    }
    last
}
