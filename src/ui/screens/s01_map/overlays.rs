//! The shared overlay selector row and the generic overlay sidebar.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use crate::sim::World;
use crate::ui::app::AppState;
use crate::widgets::map::Overlay;
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};
use super::WorldMap;

impl WorldMap {
    /// The `Overlays` selector shared by the heatmap and region sidebars.
    pub(super) fn overlays_selector(&self, f: &mut Frame<'_>, inner: Rect, mut row: u16) -> u16 {
        panel::section(f, inner, row, "Overlays");
        row += 1;
        for (key, label, active) in [
            ("1", "vegetation", self.overlay == Overlay::Vegetation),
            ("2", "pressure", self.overlay == Overlay::Pressure),
            ("3", "moisture", self.overlay == Overlay::Moisture),
            ("4", "sense", matches!(self.overlay, Overlay::Sense(_))),
            ("5", "regions", self.overlay == Overlay::Region),
            ("6", "species", matches!(self.overlay, Overlay::Species(_))),
            ("7", "health", self.overlay == Overlay::Health),
            ("8", "disease", matches!(self.overlay, Overlay::Disease(_))),
            ("9", "parasites", self.overlay == Overlay::Parasites),
        ] {
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {key} "), if active { theme::selected() } else { theme::key() }),
                Span::styled(format!("{label:<12}"), if active { theme::selected() } else { theme::text() }),
                Span::styled(if active { "►" } else { " " }, theme::text()),
            ]));
            row += 1;
        }
        row
    }
}

impl WorldMap {
    pub(super) fn overlay_sidebar(&self, f: &mut Frame<'_>, area: Rect, app: &AppState, world: &World) {
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let mut row = 0u16;

        let (name, desc1, desc2, low, high, note) = match self.overlay {
            Overlay::Vegetation => ("Vegetation density", "standing biomass per cell;", "prey graze it down, regrowth (*) restores it.", "bare", "lush", "≈ deep water  ▲ rock (not shaded)"),
            Overlay::Pressure => ("Population pressure", "traffic of prey (x0.5) and predators (x0.7)", "prey leave pressure as they move", "quiet", "crowded", "≈ deep water  ▲ rock (not shaded)"),
            Overlay::Moisture => ("Water & moisture", "soil moisture; open water is shown saturated;", "drives regrowth and thirst.", "arid", "wet", "open water counts as 100% moisture"),
            _ => return,
        };

        panel::section(f, inner, row, name);
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {desc1}"), theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {desc2}"), theme::dim_text())));
        row += 1;

        panel::section(f, inner, row, "Legend");
        row += 1;
        let ramp = |t: f32| -> Color {
            match self.overlay {
                Overlay::Vegetation => theme::veg(t),
                Overlay::Pressure => theme::heat(t),
                Overlay::Moisture => theme::water(t),
                _ => theme::DIM,
            }
        };
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
        util::line(f, inner, row, Line::from(Span::styled(format!(" {low} … {high}"), theme::dim_text())));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" {note}"), theme::dim_text())));
        row += 2;

        panel::section(f, inner, row, "By region");
        row += 1;
        for r in &world.regions {
            let mean = match self.overlay {
                Overlay::Vegetation => crate::sim::ecology::region_land_veg_mean(world, r),
                Overlay::Moisture => crate::sim::ecology::region_display_moisture_mean(world, r),
                _ => 0.0,
            };
            bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", r.0), mean, ramp(0.8), 18, 14);
            row += 1;
        }
        row += 1;

        row = self.overlays_selector(f, inner, row);
        row += 1;

        panel::section(f, inner, row, "Reading the map");
        row += 1;
        for note in [" creatures & resources faded", " Esc restores the plain map", " ░ <25%  ▒ <50%  ▓ <75%  █ ≥75%"] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }
        let _ = app;
    }
}
