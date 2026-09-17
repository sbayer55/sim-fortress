//! Base sidebar sections shared by every map mode: clock, population and
//! resources. Each section returns its rows; `WorldMap::sidebar` stacks them.

use ratatui::style::Style;
use crate::sim::{Kind, Sample, Season, Sim, World};
use crate::ui::app::AppState;
use crate::ui::screens::common::sp;
use crate::ui::style::{SeasonStyle, SpeciesStyle};
use crate::widgets::Constraint::{Fill, Fixed, Min};
use crate::widgets::{Align, Block, Columns, Divider, LabeledBar, Row, Rows, Spacer, Sparkline, Text, TrendArrow};
use crate::{glyphs, theme};
use super::group;

/// Clock rows for the S01 status sidebar.
pub(super) fn clock_section(app: &AppState, time: &crate::sim::Time) -> Rows<'static> {
    let season = time.season();
    let (sky_glyph, sky) = if time.is_night() { (glyphs::MOON, "night") } else { (glyphs::SUN, "day") };
    let (state_glyph, state_color, state) = if app.paused { (glyphs::PAUSE_STR, theme::WARN, "paused") } else { (glyphs::FAST_STR, theme::GOOD, "running") };
    vec![
        Box::new(Divider::new("Clock")),
        Box::new(Text::spans(vec![
            sp(format!(" Year {:<3} Day {:<3}", time.year(), time.day_of_season()), theme::text()),
            sp(format!("   {} {}", season.glyph(), season.name()), Style::default().fg(season.color()).bg(theme::PANEL_BG)),
        ])),
        Box::new(Text::spans(vec![
            sp(format!(" {:02}:00  {sky_glyph} {sky}", time.hour()), theme::text()),
            sp(format!("      {state_glyph} x{}  {state}", app.speed()), Style::default().fg(state_color).bg(theme::PANEL_BG)),
        ])),
        Box::new(Text::new(format!(" tick {}", group(time.tick))).style(theme::dim_text())),
        Box::new(Spacer::rows(1)),
    ]
}

/// The Population columns: ` V `, name, count, gap, arrow, gap, strip, rest.
fn population_columns() -> Columns {
    Columns::new(&[Fixed(3), Fixed(6), Min(5), Fixed(1), Fixed(1), Fixed(4), Fixed(18), Fill(1)]).align(2, Align::Right)
}

/// Live population table with trend arrows and sparklines (FR15).
pub(super) fn population_section(sim: &Sim) -> Rows<'static> {
    let samples = sim.series.samples();
    let roster = sim.roster();
    let (mut prey, mut pred) = (0u32, 0u32);
    let mut table = Vec::new();
    for id in roster.ids() {
        let count = crate::cast!(sim.creatures.living().filter(|c| c.species == id).count() => u32);
        if roster.kind(id) == Kind::Prey {
            prey += count;
        } else {
            pred += count;
        }
        let series = last_30(samples, id.index());
        let color = roster.color(id);
        table.push(
            Row::new()
                .cell(Text::new(format!(" {} ", roster.adult_glyph(id))).fg(color).bold())
                .cell(Text::new(roster.display_name(id)))
                .cell(Text::new(count.to_string()))
                .cell(Spacer::cols(1))
                .cell(TrendArrow::new(&series))
                .cell(Spacer::cols(4))
                .cell(Sparkline::new(&series).color(color))
                .cell(Spacer::cols(0)),
        );
    }
    let ratio = crate::cast!(prey => f32) / crate::cast!(pred.max(1) => f32);
    vec![
        Box::new(Divider::new("Population")),
        Box::new(Block::new(population_columns()).rows(table)),
        Box::new(Text::new(format!(" prey {prey}  pred {pred}  ratio {ratio:.1}:1")).style(theme::dim_text())),
        Box::new(Spacer::rows(1)),
    ]
}

/// Resource bars and the scarcity note.
pub(super) fn resources_section(app: &AppState, world: &World, time: &crate::sim::Time) -> Rows<'static> {
    let winter = time.season() == Season::Winter;
    let veg = mean_veg_land(world);
    let water_cells = world.cells.iter().filter(|c| c.terrain.is_water()).count();
    let water_level = if world.water_cells_at_generation > 0 {
        crate::cast!(water_cells => f32) / crate::cast!(world.water_cells_at_generation => f32)
    } else {
        0.0
    };
    let mut rows: Rows<'static> = vec![
        Box::new(Divider::new("Resources")),
        Box::new(LabeledBar::new(" vegetation", veg).color(theme::VEGETATION)),
        Box::new(LabeledBar::new(" water", water_level).color(theme::SHALLOW_FG)),
        Box::new(Text::new(format!(" carcasses {:<3} dens {:<3} regrowth {}", world.carcasses.len(), world.dens.len(), world.seeds.len()))),
    ];
    let strained = app.params.ui.scarcity_thresholds.strained;
    let below = (0..world.regions.len()).filter(|&ri| crate::sim::ecology::region_land_veg_mean(world, ri) < strained).count();
    if winter || below > 0 {
        let note = format!(" {} scarcity: {} regions below forage line", glyphs::ALERT, below);
        rows.push(Box::new(Text::new(note).style(Style::default().fg(theme::WARN).bg(theme::PANEL_BG))));
    }
    rows.push(Box::new(Spacer::rows(1)));
    rows
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

/// The last 30 days of one species' counts, for its trend arrow and sparkline.
pub(super) fn last_30(samples: &[Sample], i: usize) -> Vec<u16> {
    let start = samples.len().saturating_sub(30);
    samples[start..].iter().map(|s| crate::cast!(s.population[i].min(u32::from(u16::MAX)) => u16)).collect()
}
