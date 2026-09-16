//! S02e region overlay.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use crate::sim::{Sim, World};
use crate::ui::screens::s06_ecology::region_status;
use crate::ui::app::AppState;
use crate::widgets::{panel, util};
use crate::{glyphs, theme};
use super::WorldMap;

impl WorldMap {
    pub(super) fn region_count(app: &AppState) -> usize {
        app.sim.as_ref().map_or(0, |s| s.world.regions.len())
    }

    /// S02e sidebar: the region table, the selected region and the selector.
    pub(super) fn region_sidebar(&self, f: &mut Frame<'_>, area: Rect, app: &AppState, sim: &Sim) {
        let world = &sim.world;
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let mut row = 0u16;

        panel::section(f, inner, row, "Regions");
        row += 1;
        for note in [" named areas of the valley; rain and", " drought are tracked per region (y)"] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }
        row += 1;

        panel::section(f, inner, row, "By region");
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {:<2} {:<16}{:>5}{:>6}  status", "▪", "region", "veg", "moist"), theme::dim_text())));
        row += 1;
        let th = &app.params.ui.scarcity_thresholds;
        let prey_configured = app.params.species.initial_total(crate::sim::Kind::Prey) > 0;
        for (i, r) in world.regions.iter().enumerate() {
            let veg = crate::sim::ecology::region_land_veg_mean(world, i);
            let moist = crate::sim::ecology::region_display_moisture_mean(world, i);
            let status = region_status(veg, 0, prey_configured, th);
            let status_color = match status {
                "Scarce" => theme::BAD,
                "Strained" => theme::WARN,
                "Plenty" => theme::GOOD,
                _ => theme::TEXT,
            };
            let selected = i == self.region_sel;
            let bg = if selected { theme::SELECT_BG } else { theme::PANEL_BG };
            let text = if selected { theme::selected() } else { theme::text() };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(if selected { "►" } else { " " }, text),
                Span::styled(format!("{}{} ", glyphs::FULL_BLOCK, glyphs::FULL_BLOCK), Style::default().fg(theme::region(i)).bg(bg)),
                Span::styled(format!("{:<16}", r.0), text),
                Span::styled(format!("{:>4}%{:>5}%  ", crate::cast!((veg * 100.0).round() => u32), crate::cast!((moist * 100.0).round() => u32)), text),
                Span::styled(status, Style::default().fg(status_color).bg(bg).add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() })),
            ]));
            row += 1;
        }
        row += 1;

        panel::section(f, inner, row, "Selected");
        row += 1;
        if let Some(r) = world.regions.get(self.region_sel) {
            let cells = world.region_size(self.region_sel);
            let water = world.region_cells(self.region_sel).filter(|&(x, y)| world.cell(x, y).terrain.is_water()).count();
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {}", glyphs::FULL_BLOCK), Style::default().fg(theme::region(self.region_sel)).bg(theme::PANEL_BG)),
                Span::styled(format!(" {}", r.0), theme::title()),
            ]));
            row += 1;
            util::line(f, inner, row, Line::from(Span::styled(format!(" x {}–{}  y {}–{}", r.1, r.3.saturating_sub(1), r.2, r.4.saturating_sub(1)), theme::text())));
            row += 1;
            util::line(f, inner, row, Line::from(Span::styled(format!(" {cells} cells, {water} water"), theme::text())));
            row += 1;
            let drought = sim.drought.get(self.region_sel).copied().unwrap_or(false);
            if drought {
                util::line(f, inner, row, Line::from(Span::styled(format!(" {} drought", glyphs::DROUGHT), Style::default().fg(theme::WARN).bg(theme::PANEL_BG))));
            } else {
                util::line(f, inner, row, Line::from(Span::styled(" no drought", theme::dim_text())));
            }
            row += 1;
            util::line(f, inner, row, Line::from(vec![
                Span::styled(" Enter", theme::key()),
                Span::styled(" centres the map", theme::dim_text()),
            ]));
            row += 1;
        }
        row += 1;

        row = self.overlays_selector(f, inner, row);
        row += 1;

        panel::section(f, inner, row, "Reading the map");
        row += 1;
        for note in [" tint = region, bright = selected", " labels are clipped at the edge", " Esc restores the plain map"] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }
    }
}

/// Mean `parasite_load` over a region's cells.
pub(super) fn region_load_mean(world: &World, ri: usize) -> f32 {
    let (mut sum, mut n) = (0.0f32, 0usize);
    for (x, y) in world.region_cells(ri) {
        sum += world.cell(x, y).parasite_load;
        n += 1;
    }
    if n == 0 {
        0.0
    } else {
        sum / crate::cast!(n => f32)
    }
}

/// The region with the highest mean parasite load (`—` with no regions).
pub(super) fn worst_region(world: &World) -> (&str, f32) {
    world
        .regions
        .iter()
        .enumerate()
        .map(|(ri, r)| (r.0.as_str(), region_load_mean(world, ri)))
        .fold(("—", -1.0), |best, cur| if cur.1 > best.1 { cur } else { best })
}
