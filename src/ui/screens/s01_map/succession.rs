//! S02k succession overlay (C2 FR12): every ladder cell's climb toward forest
//! or wear toward dirt, and where the map is changing.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::{Sim, Terrain};
use crate::widgets::map::OverlayStack;
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};

use super::stack_sidebar::succession_ramp;
use super::WorldMap;

/// Per region: cells climbing (a thriving count toward a rung above), cells
/// wearing (a worn count above dirt), and cells on the ladder at all.
fn tally_by_region(sim: &Sim) -> Vec<(u32, u32, u32)> {
    let world = &sim.world;
    let mut out = vec![(0u32, 0u32, 0u32); world.regions.len()];
    for (i, c) in world.cells.iter().enumerate() {
        if !c.terrain.on_ladder() {
            continue;
        }
        let ri = world.region_index(i % world.width, i.div_euclid(world.width));
        let Some(t) = out.get_mut(ri) else { continue };
        t.2 += 1;
        if c.thrive_days > 0 && c.next_rung().is_some() {
            t.0 += 1;
        } else if c.wear_days > 0 && c.terrain.wear().is_some() {
            t.1 += 1;
        }
    }
    out
}

impl WorldMap {
    /// S02k sidebar: what succession is, the two-sided ramp, the climbing and
    /// wearing shares per region, the forest and bare counts, a reading note
    /// and the Stack section.
    pub(super) fn succession_sidebar(f: &mut Frame<'_>, area: Rect, sim: &Sim, stack: &OverlayStack) {
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let mut row = 0u16;
        let world = &sim.world;

        panel::section(f, inner, row, "Succession");
        row += 1;
        for note in [" untrodden lush ground climbs to forest;", " grazed, trodden ground wears to dirt."] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }

        panel::section(f, inner, row, "Legend");
        row += 1;
        for i in 0..24 {
            let t = (f32::from(i) + 0.5) / 24.0;
            let c = succession_ramp(t);
            if let Some(cell) = f.buffer_mut().cell_mut((inner.x + 4 + i, inner.y + row)) {
                cell.set_char(glyphs::shade((2.0 * t - 1.0).abs()));
                cell.set_style(Style::default().fg(c).bg(theme::dim(c, 0.75)));
            }
        }
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" wearing … climbing  ", theme::dim_text()),
            Span::styled(format!("{}/{} ripe to flip", glyphs::DOWN, glyphs::UP), theme::text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" dim: unchanging   ▲ rock  ≈ deep water", theme::dim_text())));
        row += 1;

        panel::section(f, inner, row, "By region · climbing share");
        row += 1;
        let tally = tally_by_region(sim);
        for (ri, r) in world.regions.iter().enumerate() {
            let (climbing, _, ladder) = tally[ri];
            let share = crate::cast!(climbing => f32) / crate::cast!(ladder.max(1) => f32);
            bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", r.0), share, theme::veg(0.8), 18, 14);
            row += 1;
        }
        let climbing: u32 = tally.iter().map(|t| t.0).sum();
        let wearing: u32 = tally.iter().map(|t| t.1).sum();
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {climbing} climbing"), if climbing == 0 { theme::dim_text() } else { theme::text() }),
            Span::styled(format!("   {wearing} wearing"), if wearing == 0 { theme::dim_text() } else { theme::text() }),
        ]));
        row += 2;

        panel::section(f, inner, row, "Terrain");
        row += 1;
        let (mut forest, mut bare, mut land) = (0u32, 0u32, 0u32);
        for c in &world.cells {
            if c.terrain.is_water() {
                continue;
            }
            land += 1;
            match c.terrain {
                Terrain::Forest => forest += 1,
                Terrain::Dirt | Terrain::Sand => bare += 1,
                _ => {}
            }
        }
        let pct = |n: u32| crate::cast!((crate::cast!(n => f32) / crate::cast!(land.max(1) => f32) * 100.0).round() => u32);
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} forest {forest:>5} {:>3}%", glyphs::FOREST, pct(forest)), theme::text()),
            Span::styled(format!("   {} bare {bare:>5} {:>3}%", glyphs::DIRT, pct(bare)), theme::text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" share of land; y ecology lists the rest", theme::dim_text())));
        row += 2;

        panel::section(f, inner, row, "Reading the map");
        row += 1;
        for note in [" creatures & resources faded", " k look names the cell's terrain", " Esc restores the plain map"] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }
        row += 1;
        Self::stack_rows(f, inner, row, stack);
    }
}
