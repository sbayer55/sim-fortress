//! S02g health overlay.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use crate::sim::{Sim, SpeciesId};
use crate::ui::style::{SpeciesStyle};
use crate::widgets::{bars, panel, util};
use crate::theme;
use super::WorldMap;

/// Per-species condition band tally; returns the next row plus the totals the
/// "Weakest vital" section needs.
fn health_by_species(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim) -> (u16, [usize; 3], [usize; 4]) {
    use crate::ui::style::condition;

    let tone = |c: Color| Style::default().fg(c).bg(theme::PANEL_BG);
        // Per-species tally of the three bands and the mean condition.
        panel::section(f, inner, row, "By species");
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled("   species    n  good strn crit  mean", theme::dim_text())));
        row += 1;
        let mut all = [0usize; 3];
        let (mut all_n, mut all_sum) = (0usize, 0.0f32);
        let mut weakest = [0usize; 4];
        for id in SpeciesId::ALL {
            let mut bands = [0usize; 3];
            let (mut n, mut sum) = (0usize, 0.0f32);
            for c in sim.creatures.living().filter(|c| c.species == id) {
                let (t, which) = condition(c);
                let band = if t > 0.6 { 0 } else if t > 0.3 { 1 } else { 2 };
                bands[band] += 1;
                if band > 0 {
                    weakest[which] += 1;
                }
                n += 1;
                sum += t;
            }
            for (a, b) in all.iter_mut().zip(bands) {
                *a += b;
            }
            all_n += n;
            all_sum += sum;
            let text = if n == 0 { theme::dim_text() } else { theme::text() };
            let count = |k: usize, color: Color| Span::styled(format!("{:>5}", bands[k]), if n == 0 { theme::dim_text() } else { tone(color) });
            let mean = if n > 0 { format!("{:>4}%", crate::cast!((sum / crate::cast!(n => f32) * 100.0).round() => u32)) } else { "    —".to_string() };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {} ", id.glyph().to_ascii_uppercase()), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<8}{n:>4}", id.name()), text),
                count(0, theme::GOOD),
                count(1, theme::WARN),
                count(2, theme::BAD),
                Span::styled(mean, text),
            ]));
            row += 1;
        }
        let mean = if all_n > 0 { format!("{:>4}%", crate::cast!((all_sum / crate::cast!(all_n => f32) * 100.0).round() => u32)) } else { "    —".to_string() };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!("   {:<8}{all_n:>4}", "all"), theme::text()),
            Span::styled(format!("{:>5}", all[0]), tone(theme::GOOD)),
            Span::styled(format!("{:>5}", all[1]), tone(theme::WARN)),
            Span::styled(format!("{:>5}", all[2]), tone(theme::BAD)),
            Span::styled(mean, theme::text()),
        ]));
        row += 2;
    (row, all, weakest)
}

/// Which vital is dragging the strained and critical animals down.
fn health_weakest(f: &mut Frame<'_>, inner: Rect, mut row: u16, all: [usize; 3], weakest: [usize; 4]) -> u16 {
    use crate::ui::style::VITALS;
        // Which vital is dragging the strained and critical animals down.
        let unwell = all[1] + all[2];
        panel::section(f, inner, row, "Weakest vital");
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!(" of the {unwell} strained or critical:"), theme::dim_text())));
        row += 1;
        for (i, label) in VITALS.iter().enumerate() {
            let share = if unwell > 0 { crate::cast!(weakest[i] => f32) / crate::cast!(unwell => f32) } else { 0.0 };
            util::line(f, inner, row, Line::from(Span::styled(format!(" {label:<8}"), theme::text())));
            bars::bar(f.buffer_mut(), inner.x + 10, inner.y + row, 12, share, theme::WARN);
            util::line(f, Rect::new(inner.x + 23, inner.y, inner.width.saturating_sub(23), inner.height), row, Line::from(vec![
                Span::styled(format!("{:>4}", weakest[i]), theme::text()),
                Span::styled(format!("{:>4}%", crate::cast!((share * 100.0).round() => u32)), theme::dim_text()),
            ]));
            row += 1;
        }
        row += 1;
    row
}

impl WorldMap {
    /// S02g sidebar: what the colours mean, every species' condition tally,
    /// which vital is failing the strained animals, and reading notes.
    pub(super) fn health_sidebar(&self, f: &mut Frame<'_>, area: Rect, sim: &Sim) {
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let mut row = 0u16;
        let tone = |c: Color| Style::default().fg(c).bg(theme::PANEL_BG);

        panel::section(f, inner, row, "Health");
        row += 1;
        for note in [" colour = each animal's weakest vital:", " health, hunger, thirst or energy."] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }

        panel::section(f, inner, row, "Legend");
        row += 1;
        for (color, label, band) in [(theme::GOOD, "healthy ", "above 60%"), (theme::WARN, "strained", "30–60%"), (theme::BAD, "critical", "below 30%")] {
            util::line(f, inner, row, Line::from(vec![
                Span::styled(" ███ ", tone(color)),
                Span::styled(format!("{label}  "), theme::text()),
                Span::styled(band, theme::dim_text()),
            ]));
            row += 1;
        }
        util::line(f, inner, row, Line::from(Span::styled(" terrain dimmed  % carcass unchanged", theme::dim_text())));
        row += 2;

        let (next_row, all, weakest) = health_by_species(f, inner, row, sim);
        row = health_weakest(f, inner, next_row, all, weakest);

        row = self.overlays_selector(f, inner, row);
        row += 1;

        panel::section(f, inner, row, "Reading the map");
        row += 1;
        for note in [" colour is the animal, not the ground", " k look / Enter inspects one animal", " Esc restores the plain map"] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }
    }
}
