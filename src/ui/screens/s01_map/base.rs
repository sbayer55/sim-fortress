//! Base sidebar sections shared by every map mode (clock, population, resources, legend).

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use crate::sim::{Season, Sim, SpeciesId, World};
use crate::ui::app::AppState;
use crate::ui::style::{SeasonStyle, SpeciesStyle};
use crate::widgets::map::{self};
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};
use super::group;

/// Clock rows for the S01 status sidebar.
pub(super) fn clock_section(f: &mut Frame<'_>, inner: Rect, mut row: u16, app: &AppState, time: &crate::sim::Time) -> u16 {
    let night = time.is_night();
        panel::section(f, inner, row, "Clock");
        row += 1;
        let season = time.season();
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" Year {:<3} Day {:<3}", time.year(), time.day_of_season()), theme::text()),
            Span::styled(format!("   {} {}", season.glyph(), season.name()), Style::default().fg(season.color()).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        let sky = if night { (glyphs::MOON, "night") } else { (glyphs::SUN, "day") };
        let (state_glyph, state_color) = if app.paused { (glyphs::PAUSE_STR, theme::WARN) } else { (glyphs::FAST_STR, theme::GOOD) };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {:02}:00  {} {}", time.hour(), sky.0, sky.1), theme::text()),
            Span::styled(format!("      {} x{}  {}", state_glyph, app.speed(), if app.paused { "paused" } else { "running" }), Style::default().fg(state_color).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" tick {}", group(time.tick)), theme::dim_text())));
        row += 2;
    row
}

/// Live population table with trend arrows and sparklines.
pub(super) fn population_section(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim) -> u16 {
        // Live population (FR15).
        panel::section(f, inner, row, "Population");
        row += 1;
        let samples = sim.series.samples();
        let mut prey = 0u32;
        let mut pred = 0u32;
        for (i, id) in sim.roster().ids().map(|id| (id.index(), id)) {
            let count = crate::cast!(sim.creatures.living().filter(|c| c.species == id).count() => u32);
            if sim.roster().kind(id) == crate::sim::Kind::Prey {
                prey += count;
            } else {
                pred += count;
            }
            let arrow = trend_arrow(samples, i);
            let arrow_color = match arrow {
                glyphs::UP => theme::GOOD,
                glyphs::DOWN => theme::BAD,
                _ => theme::DIM,
            };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {} ", sim.roster().adult_glyph(id)), Style::default().fg(sim.roster().color(id)).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<6}", sim.roster().display_name(id)), theme::text()),
                Span::styled(format!("{count:>5} "), theme::text()),
                Span::styled(arrow.to_string(), Style::default().fg(arrow_color).bg(theme::PANEL_BG)),
                Span::styled("  ", theme::text()),
            ]));
            let spark = sparkline(samples, i);
            bars::sparkline(f.buffer_mut(), inner.x + 20, inner.y + row, 18, &spark, sim.roster().color(id));
            row += 1;
        }
        let ratio = crate::cast!(prey => f32) / crate::cast!(pred.max(1) => f32);
        util::line(f, inner, row, Line::from(Span::styled(format!(" prey {prey}  pred {pred}  ratio {ratio:.1}:1"), theme::dim_text())));
        row += 2;
    row
}

/// Resource bars and the scarcity note.
pub(super) fn resources_section(f: &mut Frame<'_>, inner: Rect, mut row: u16, app: &AppState, world: &World, time: &crate::sim::Time) -> u16 {
    let winter = time.season() == Season::Winter;
        panel::section(f, inner, row, "Resources");
        row += 1;
        let veg = mean_veg_land(world);
        let water_cells = world.cells.iter().filter(|c| c.terrain.is_water()).count();
        let water_level = if world.water_cells_at_generation > 0 {
            crate::cast!(water_cells => f32) / crate::cast!(world.water_cells_at_generation => f32)
        } else {
            0.0
        };
        bars::labeled(f.buffer_mut(), inner, row, " vegetation", veg, theme::VEGETATION, 12, 20);
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " water", water_level, theme::SHALLOW_FG, 12, 20);
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(
            format!(" carcasses {:<3} dens {:<3} regrowth {}", world.carcasses.len(), world.dens.len(), world.seeds.len()),
            theme::text(),
        )));
        row += 1;
        let strained = app.params.ui.scarcity_thresholds.strained;
        let below = world.regions.iter().filter(|r| crate::sim::ecology::region_land_veg_mean(world, r) < strained).count();
        if winter || below > 0 {
            util::line(f, inner, row, Line::from(Span::styled(
                format!(" {} scarcity: {} regions below forage line", glyphs::ALERT, below),
                Style::default().fg(theme::WARN).bg(theme::PANEL_BG),
            )));
            row += 1;
        }
        row += 1;
    row
}

/// Terrain and species legend.
pub(super) fn legend_section(f: &mut Frame<'_>, inner: Rect, mut row: u16, roster: &crate::sim::Roster) {
        panel::section(f, inner, row, "Legend");
        row += 1;
        let legend = map::legend();
        for pair in legend.chunks(2) {
            let mut spans = vec![Span::styled(" ", theme::text())];
            for (g, color, label) in pair {
                spans.push(Span::styled(g.to_string(), Style::default().fg(*color).bg(theme::PANEL_BG)));
                spans.push(Span::styled(format!(" {label:<17}"), theme::dim_text()));
            }
            util::line(f, inner, row, Line::from(spans));
            row += 1;
        }
        let ids: Vec<SpeciesId> = roster.ids().collect();
        for chunk in ids.chunks(3) {
            let mut spans = vec![Span::styled(" ", theme::text())];
            for &id in chunk {
                spans.push(Span::styled(format!("{}{}", roster.adult_glyph(id), roster.glyph(id)), Style::default().fg(roster.color(id)).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)));
                spans.push(Span::styled(format!(" {:<10}", roster.display_name(id)), theme::dim_text()));
            }
            util::line(f, inner, row, Line::from(spans));
            row += 1;
        }
        util::line(f, inner, row, Line::from(Span::styled(" UPPER adult   lower juvenile", theme::dim_text())));
}

fn mean_veg_land(world: &World) -> f32 {
    let mut sum = 0.0f32;
    let mut n = 0usize;
    for cell in &world.cells {
        if !cell.terrain.is_water() {
            sum += cell.vegetation;
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        sum / crate::cast!(n => f32)
    }
}

/// 30-day trend arrow per the FR15 rule: > +3% ↑, < −3% ↓, else ↔.
pub(super) fn trend_arrow(samples: &[crate::sim::Sample], i: usize) -> char {
    if samples.len() < 2 {
        return glyphs::FLAT;
    }
    let a = crate::cast!(samples[samples.len().saturating_sub(30).min(samples.len() - 1)].population[i] => f32);
    let b = crate::cast!(samples.last().map_or(0, |s| s.population[i]) => f32);
    let pct = if a > 0.0 { (b - a) / a * 100.0 } else if b > 0.0 { f32::INFINITY } else { 0.0 };
    if pct > 3.0 {
        glyphs::UP
    } else if pct < -3.0 {
        glyphs::DOWN
    } else {
        glyphs::FLAT
    }
}

/// Last 30 days of per-species counts, for sparklines.
fn sparkline(samples: &[crate::sim::Sample], i: usize) -> Vec<u16> {
    let start = samples.len().saturating_sub(30);
    samples[start..].iter().map(|s| crate::cast!(s.population[i].min(u32::from(u16::MAX)) => u16)).collect()
}
