//! S02i parasite overlay (C7).

use std::collections::HashMap;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use crate::sim::creatures::CreatureId;
use crate::sim::{Sim, World};
use crate::ui::style::{SpeciesStyle};
use crate::widgets::map::{self};
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};
use super::WorldMap;
use super::disease_overlay::fmt2;
use super::regions::{region_load_mean, worst_region};

/// Mean parasite load per species and the most fouled region.
pub(super) fn disease_parasites(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, world: &World) -> u16 {
    let tone = |c: Color| Style::default().fg(c).bg(theme::PANEL_BG);
        // Parasites: mean load per species and the most fouled region.
        panel::section(f, inner, row, "Parasites");
        row += 1;
        let (means, _heavy) = parasite_by_species(sim);
        let mut spans = vec![Span::styled(" mean load", theme::dim_text())];
        for id in sim.roster().ids() {
            spans.push(Span::styled(format!(" {}", sim.roster().glyph(id)), tone(sim.roster().color(id)).add_modifier(Modifier::BOLD)));
            spans.push(Span::styled(fmt2(means[id.index()]), theme::text()));
        }
        util::line(f, inner, row, Line::from(spans));
        row += 1;
        let (worst, worst_mean) = worst_region(world);
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" worst ground: ", theme::dim_text()),
            Span::styled(worst, tone(theme::WARN)),
            Span::styled(format!(" {}", fmt2(worst_mean)), theme::text()),
        ]));
        row += 1;
    row
}

/// Per-species mean parasite load, heavy carriers and litter penalty.
fn parasite_species_section(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, dp: &crate::sim::params::DiseaseParams) -> u16 {
    let tone = |c: Color| Style::default().fg(c).bg(theme::PANEL_BG);
        // By species: mean load bar, heavy carriers and the litter penalty.
        panel::section(f, inner, row, "By species");
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(format!("{:<3}{:<5}{:>4} {:<12}{:>4}{:>4}{:>7}", "", "name", "n", "load", "mean", "hvy", "litter"), theme::dim_text())));
        row += 1;
        let (means, heavy) = parasite_by_species(sim);
        for id in sim.roster().ids() {
            let i = id.index();
            let n = sim.creatures.living().filter(|c| c.species == id).count();
            let text = if n == 0 { theme::dim_text() } else { theme::text() };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {} ", sim.roster().adult_glyph(id)), tone(sim.roster().color(id)).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<5}{n:>4}", sim.roster().display_name(id)), text),
            ]));
            bars::bar(f.buffer_mut(), inner.x + 13, inner.y + row, 12, means[i], theme::WARN);
            let litter = crate::cast!((dp.parasite_fertility_w * means[i] * 100.0).round() => u32);
            util::line(f, Rect::new(inner.x + 25, inner.y, inner.width.saturating_sub(25), inner.height), row, Line::from(vec![
                Span::styled(if n == 0 { "   —".to_string() } else { format!(" {}", fmt2(means[i])) }, text),
                Span::styled(format!("{:>4}", heavy[i]), if heavy[i] > 0 { tone(theme::BAD) } else { theme::dim_text() }),
                Span::styled(format!("{:>7}", format!("−{litter}%")), if litter > 0 { tone(theme::WARN) } else { theme::dim_text() }),
            ]));
            row += 1;
        }
    row
}

/// The three heaviest living carriers.
fn parasite_carriers(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, world: &World) -> u16 {
    let tone = |c: Color| Style::default().fg(c).bg(theme::PANEL_BG);
        // Carriers: the three heaviest living carriers.
        panel::section(f, inner, row, "Carriers");
        row += 1;
        let mut carriers: Vec<&crate::sim::creatures::Creature> = sim.creatures.living().filter(|c| c.parasite_load > 0.0).collect();
        carriers.sort_by(|a, b| b.parasite_load.partial_cmp(&a.parasite_load).unwrap_or(std::cmp::Ordering::Equal).then(a.id.0.cmp(&b.id.0)));
        for k in 0..3 {
            match carriers.get(k) {
                Some(c) => {
                    let (color, _) = map::parasite_tint(sim.roster().color(c.species), c.parasite_load);
                    let name: String = c.name_str(sim.roster()).chars().take(8).collect();
                    util::line(f, inner, row, Line::from(vec![
                        Span::styled(format!(" {} ", sim.roster().glyph(c.species)), tone(sim.roster().color(c.species)).add_modifier(Modifier::BOLD)),
                        Span::styled(format!("{:<6} {name:<8} ", c.tag(sim.roster())), theme::text()),
                        Span::styled(fmt2(c.parasite_load), tone(color)),
                        Span::styled(format!(" {}", world.region_name(c.x, c.y)), theme::dim_text()),
                    ]));
                }
                None if k == 0 => util::line(f, inner, row, Line::from(Span::styled(" no carriers", theme::dim_text()))),
                None => {}
            }
            row += 1;
        }
    row
}

/// S02i: every living creature's colour under the parasite overlay.
pub(super) fn parasite_tints(sim: &Sim) -> HashMap<CreatureId, (Color, bool)> {
    sim.creatures.living().map(|c| (c.id, map::parasite_tint(sim.roster().color(c.species), c.parasite_load))).collect()
}

/// Mean parasite load and heavy-carrier count (`≥ PARASITE_HEAVY`) per species.
fn parasite_by_species(sim: &Sim) -> (Vec<f32>, Vec<u32>) {
    let k = sim.roster().len();
    let (mut sum, mut n, mut heavy) = (vec![0.0f32; k], vec![0u32; k], vec![0u32; k]);
    for c in sim.creatures.living() {
        let i = c.species.index();
        sum[i] += c.parasite_load;
        n[i] += 1;
        if c.parasite_load >= map::PARASITE_HEAVY {
            heavy[i] += 1;
        }
    }
    ((0..k).map(|i| if n[i] > 0 { sum[i] / crate::cast!(n[i] => f32) } else { 0.0 }).collect(), heavy)
}

impl WorldMap {
    /// S02i sidebar: the parasite ramp and creature bands, mean cell load per
    /// region, per-species load / heavy count / litter penalty, the three
    /// heaviest carriers, the selector and a reading note. Exactly 40 rows.
    pub(super) fn parasite_sidebar(&self, f: &mut Frame<'_>, area: Rect, sim: &Sim) {
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let mut row = 0u16;
        let tone = |c: Color| Style::default().fg(c).bg(theme::PANEL_BG);
        let world = &sim.world;
        let dp = &sim.params.disease;

        panel::section(f, inner, row, "Parasites");
        row += 1;
        for note in [" worms build up where animals graze,", " drink and rest; carcasses pass them on."] {
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
                c.set_style(Style::default().fg(theme::parasite(t)).bg(theme::dim(theme::parasite(t), 0.75)));
            }
        }
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" clean … fouled   ", theme::dim_text()),
            Span::styled("~", tone(theme::WARN)),
            Span::styled(" fouled water  ▲ rock", theme::dim_text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" animals: ", theme::dim_text()),
            Span::styled("dim", theme::dim_text()),
            Span::styled(format!(" <{}%  ", crate::cast!((map::PARASITE_LIGHT * 100.0) => u32)), theme::dim_text()),
            Span::styled("amber", tone(theme::WARN)),
            Span::styled(format!(" <{}%  ", crate::cast!((map::PARASITE_HEAVY * 100.0) => u32)), theme::dim_text()),
            Span::styled("red", tone(theme::BAD).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" ≥{}%", crate::cast!((map::PARASITE_HEAVY * 100.0) => u32)), theme::dim_text()),
        ]));
        row += 1;

        // By region: mean cell load, then the worst region and the fouled count.
        panel::section(f, inner, row, "By region");
        row += 1;
        for r in &world.regions {
            bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", r.0), region_load_mean(world, r), theme::WARN, 18, 14);
            row += 1;
        }
        let (worst, _) = worst_region(world);
        let fouled = world.cells.iter().filter(|c| c.parasite_load >= map::PARASITE_TINT_THRESHOLD).count();
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" worst: ", theme::dim_text()),
            Span::styled(format!("{worst:<16}"), tone(theme::WARN)),
            Span::styled(format!("  {fouled} cells ≥{}%", crate::cast!((map::PARASITE_TINT_THRESHOLD * 100.0) => u32)), theme::text()),
        ]));
        row += 1;

        row = parasite_species_section(f, inner, row, sim, dp);
        row = parasite_carriers(f, inner, row, sim, world);

        row = self.overlays_selector(f, inner, row);
        util::line(f, inner, row, Line::from(Span::styled(" k look = exact cell load · Esc restores", theme::dim_text())));
    }
}
