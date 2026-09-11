//! S05: the live charts screen (S05a/S05c — vegetation only in C2).

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::SpeciesId;
use crate::ui::app::AppState;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

#[derive(Clone, Copy, PartialEq)]
pub enum Variant {
    Time,
    Stacked,
}

pub struct Charts {
    variant: Variant,
    zoom: usize,
}

impl Default for Charts {
    fn default() -> Self {
        Self::new()
    }
}

impl Charts {
    pub fn new() -> Self {
        Charts { variant: Variant::Time, zoom: 240 }
    }
}

impl Screen for Charts {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, _app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Char('g') => {
                self.variant = match self.variant {
                    Variant::Time => Variant::Stacked,
                    Variant::Stacked => Variant::Time,
                };
                Action::None
            }
            KeyCode::Char('1') => {
                self.variant = Variant::Time;
                Action::None
            }
            KeyCode::Char('3') => {
                self.variant = Variant::Stacked;
                Action::None
            }
            KeyCode::Char('+') => {
                self.zoom = match self.zoom {
                    60 => 240,
                    240 => 720,
                    _ => 720,
                };
                Action::None
            }
            KeyCode::Char('-') => {
                self.zoom = match self.zoom {
                    720 => 240,
                    240 => 60,
                    _ => 60,
                };
                Action::None
            }
            KeyCode::Esc => Action::Pop,
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
        let Some(sim) = &app.sim else {
            return;
        };

        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let chart_area = Rect::new(area.x, area.y, area.width * 2 / 3, body_h);
        let side_area = Rect::new(area.x + chart_area.width, area.y, area.width - chart_area.width, body_h);

        match self.variant {
            Variant::Time => self.time_chart(f, chart_area, side_area, sim),
            Variant::Stacked => self.stacked_chart(f, chart_area, side_area, sim),
        }

        let view = if self.variant == Variant::Time { "chart 1/3  vegetation" } else { "chart 3/3  stacked species" };
        status::render(f, Rect::new(area.x, status_row, area.width, 1), &[("g", "next chart"), ("1/3", "pick"), ("+/-", "zoom"), ("Esc", "back")], view);
    }
}

impl Charts {
    fn time_chart(&self, f: &mut Frame, chart_area: Rect, side_area: Rect, sim: &crate::sim::Sim) {
        let inner = panel::draw_with_hint(f, chart_area, "Vegetation (mean biomass, % of max)", &format!("last {} days", self.zoom), panel::Kind::Outer);
        let plot = Rect::new(inner.x + 4, inner.y + 1, inner.width - 4, inner.height - 3);
        veg_plot(f, plot, sim, self.zoom);
        util::line(f, inner, inner.height - 1, Line::from(Span::styled(" no population data yet", theme::dim_text())));

        let side = panel::draw(f, side_area, "Drought", panel::Kind::Outer);
        let mut row = 0u16;
        panel::section(f, side, row, "Drought");
        row += 1;
        let droughts = sim.series.samples().iter().filter(|s| s.drought_regions >= 2).count();
        util::line(f, side, row, Line::from(Span::styled(format!(" {} drought days (≥2 regions)", droughts), theme::text())));
        row += 2;
        panel::section(f, side, row, "Legend");
        row += 1;
        util::line(f, side, row, Line::from(vec![
            Span::styled(" █ vegetation ", Style::default().fg(theme::VEGETATION).bg(theme::PANEL_BG)),
            Span::styled("  ░ drought band", Style::default().fg(theme::WARN).bg(theme::PANEL_BG)),
        ]));
    }

    fn stacked_chart(&self, f: &mut Frame, chart_area: Rect, side_area: Rect, sim: &crate::sim::Sim) {
        let inner = panel::draw_with_hint(f, chart_area, "Stacked populations + vegetation", &format!("{} days", self.zoom), panel::Kind::Outer);
        // Dim species legend row.
        let mut spans = vec![Span::styled(" ", theme::dim_text())];
        for id in SpeciesId::ALL {
            spans.push(Span::styled(format!("█ {}  ", id.name()), Style::default().fg(id.color()).bg(theme::PANEL_BG)));
        }
        spans.push(Span::styled(" · vegetation biomass (right axis, % of max)", theme::dim_text()));
        util::line(f, inner, 1, Line::from(spans));
        let plot = Rect::new(inner.x + 4, inner.y + 3, inner.width - 4, inner.height - 6);
        veg_plot(f, plot, sim, self.zoom);

        let side = panel::draw(f, side_area, "Composition", panel::Kind::Outer);
        util::line(f, side, 1, Line::from(Span::styled(" no population data yet", theme::dim_text())));
    }
}

/// Plot the vegetation series (0..1) into `plot`, shading drought bands.
fn veg_plot(f: &mut Frame, plot: Rect, sim: &crate::sim::Sim, zoom: usize) {
    let samples = sim.series.samples();
    if samples.is_empty() {
        return;
    }
    // Use the most recent `zoom` samples.
    let start = samples.len().saturating_sub(zoom);
    let slice = &samples[start..];
    let buf = f.buffer_mut();
    let cols = plot.width as usize;
    let rows = plot.height as usize;
    for col in 0..cols {
        let idx = col * slice.len() / cols;
        let s = &slice[idx.min(slice.len() - 1)];
        // Drought band background.
        if s.drought_regions >= 2 {
            for r in 0..rows {
                if let Some(c) = buf.cell_mut((plot.x + col as u16, plot.y + r as u16)) {
                    c.set_bg(theme::dim(theme::WARN, 0.78));
                }
            }
        }
        // Vegetation line (full blocks, height = veg% of plot).
        let h = ((s.veg_mean * rows as f32).round() as usize).min(rows);
        for r in 0..h {
            let y = plot.y + (rows - 1 - r) as u16;
            if let Some(c) = buf.cell_mut((plot.x + col as u16, y)) {
                c.set_char(glyphs::FULL_BLOCK);
                c.set_style(Style::default().fg(theme::VEGETATION).bg(c.bg));
            }
        }
    }
    // Y-axis labels (0%, 50%, 100%).
    util::line(f, Rect::new(plot.x.saturating_sub(4), plot.y, 4, plot.height), 0, Line::from(Span::styled("100%", theme::dim_text())));
    util::line(f, Rect::new(plot.x.saturating_sub(4), plot.y, 4, plot.height), (plot.height - 1) / 2, Line::from(Span::styled(" 50%", theme::dim_text())));
    util::line(f, Rect::new(plot.x.saturating_sub(4), plot.y, 4, plot.height), plot.height - 1, Line::from(Span::styled("  0%", theme::dim_text())));
}
