//! S02f species overlay.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use crate::sim::{Sim, SpeciesId};
use crate::ui::style::{SpeciesStyle};
use crate::widgets::map::{self};
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};
use super::WorldMap;
use super::base::trend_arrow;

/// Per-region counts of the shown species, as a share of its population.
#[allow(clippy::too_many_arguments)]
fn species_by_region(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, sp: SpeciesId, color: Color) -> u16 {
    let world = &sim.world;
    let ramp = |t: f32| theme::species_ramp(color, t);
        // Per-region counts of the shown species, as a share of its population.
        let total = sim.creatures.living().filter(|c| c.species == sp).count();
        panel::section(f, inner, row, "By region");
        row += 1;
        let mut densest: Option<(usize, f32)> = None;
        for (i, r) in world.regions.iter().enumerate() {
            let n = sim.creatures.living().filter(|c| c.species == sp && world.region_index(c.x, c.y) == i).count();
            let share = if total > 0 { crate::cast!(n => f32) / crate::cast!(total => f32) } else { 0.0 };
            let cells = crate::cast!(world.region_size(i).max(1) => f32);
            let per_cell = crate::cast!(n => f32) / cells;
            if n > 0 && densest.is_none_or(|(_, d)| per_cell > d) {
                densest = Some((i, per_cell));
            }
            util::line(f, inner, row, Line::from(Span::styled(format!(" {:<16}", r.0), theme::text())));
            bars::bar(f.buffer_mut(), inner.x + 18, inner.y + row, 12, share, ramp(0.8));
            util::line(f, Rect::new(inner.x + 31, inner.y, inner.width.saturating_sub(31), inner.height), row, Line::from(vec![
                Span::styled(format!("{n:>4}"), theme::text()),
                Span::styled(format!("{:>4}%", crate::cast!((share * 100.0).round() => u32)), theme::dim_text()),
            ]));
            row += 1;
        }
        let samples = sim.series.samples();
        let arrow = trend_arrow(samples, sp.index());
        let densest = densest.map_or("—", |(i, _)| world.regions[i].0.as_str());
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {total} alive {arrow}"), if total == 0 { theme::dim_text() } else { theme::text() }),
            Span::styled("  densest: ", theme::dim_text()),
            Span::styled(densest, Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        ]));
        row += 2;
    row
}

/// The species switcher list.
fn species_list(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, sp: SpeciesId) -> u16 {
        panel::section(f, inner, row, "Species");
        row += 1;
        for id in sim.roster().ids() {
            let active = id == sp;
            let n = sim.creatures.living().filter(|c| c.species == id).count();
            let text = if active { theme::selected() } else { theme::text() };
            let bg = if active { theme::SELECT_BG } else { theme::PANEL_BG };
            let status = if n == 0 { "extinct" } else { "" };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(if active { "►" } else { " " }, text),
                Span::styled(format!("{} ", sim.roster().adult_glyph(id)), Style::default().fg(sim.roster().color(id)).bg(bg).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<8}{n:>5}  ", sim.roster().display_name(id)), text),
                Span::styled(status, Style::default().fg(theme::DIM).bg(bg)),
            ]));
            row += 1;
        }
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" Tab", theme::key()),
            Span::styled(" next  ", theme::dim_text()),
            Span::styled("Shift+Tab", theme::key()),
            Span::styled(" previous", theme::dim_text()),
        ]));
        row += 1;
    row
}

impl WorldMap {
    /// S02f sidebar: what the density shows, its legend, the species' spread by
    /// region, the species selector and reading notes.
    pub(super) fn species_sidebar(&self, f: &mut Frame<'_>, area: Rect, sim: &Sim, sp: SpeciesId) {
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let mut row = 0u16;
        let color = sim.roster().color(sp);
        let ramp = |t: f32| theme::species_ramp(color, t);

        panel::section(f, inner, row, &format!("{} density", sim.roster().display_name(sp)));
        row += 1;
        for note in [
            format!(" living {} within {} cells of a spot;", sim.roster().plural(sp).to_lowercase(), map::DENSITY_RADIUS),
            " one animal reads faint, a herd bright.".to_string(),
        ] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }

        panel::section(f, inner, row, "Legend");
        row += 1;
        for i in 0..24 {
            let t = (f32::from(i) + 0.5) / 24.0;
            let g = glyphs::shade(t);
            if let Some(c) = f.buffer_mut().cell_mut((inner.x + 4 + i, inner.y + row)) {
                c.set_char(g);
                c.set_style(Style::default().fg(ramp(t)).bg(theme::dim(ramp(t), 0.75)));
            }
        }
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled("  0%      25%      50%      75%      100%", theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" none … crowded", theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" 100% ≈ {} together  ≈ deep water  ▲ rock", crate::cast!(map::DENSITY_CAP => u32)), theme::dim_text())));
        row += 2;

        row = species_by_region(f, inner, row, sim, sp, color);
        row = species_list(f, inner, row, sim, sp);

        row = self.overlays_selector(f, inner, row);
        row += 1;

        panel::section(f, inner, row, "Reading the map");
        row += 1;
        for note in [" shown species bright, others faded", " Esc restores the plain map", " ░ <25%  ▒ <50%  ▓ <75%  █ ≥75%"] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }
    }
}
