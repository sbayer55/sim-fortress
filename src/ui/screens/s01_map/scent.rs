//! S02j scent overlay (C5 FR13): one predator species' scent grid and who
//! holds it.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use crate::sim::creatures::CreatureId;
use crate::sim::{Kind, Sim, SpeciesId};
use crate::ui::style::SpeciesStyle;
use crate::widgets::map::OverlayStack;
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};
use super::WorldMap;

/// Holders listed in the sidebar.
const HOLDER_ROWS: usize = 3;

/// Cells of `sp`'s block held at or above `hold_min`, per region.
fn held_by_region(sim: &Sim, sp: SpeciesId) -> Vec<u32> {
    let world = &sim.world;
    let hold_min = sim.params.territory.hold_min;
    let mut out = vec![0u32; world.regions.len()];
    for (i, m) in world.scent_block(sp).iter().enumerate() {
        if m.strength >= hold_min {
            let ri = world.region_index(i % world.width, i.div_euclid(world.width));
            if let Some(n) = out.get_mut(ri) {
                *n += 1;
            }
        }
    }
    out
}

/// Every holder of `sp` with the cells it holds, most first (ties by id).
fn holders(sim: &Sim, sp: SpeciesId) -> Vec<(CreatureId, u32)> {
    let mut counts = sim.world.held_cells_by_holder(sp, sim.params.territory.hold_min);
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    counts
}

/// The species switcher list, as S02f draws it.
fn species_list(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, sp: SpeciesId) -> u16 {
    panel::section(f, inner, row, "Species");
    row += 1;
    for id in sim.roster().ids() {
        let active = id == sp;
        let n = sim.creatures.living().filter(|c| c.species == id).count();
        let tone = if active { Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD) } else if n == 0 { theme::dim_text() } else { theme::text() };
        let held: u32 = held_by_region(sim, id).iter().sum();
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} ", sim.roster().adult_glyph(id)), Style::default().fg(sim.roster().color(id)).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:<6}{n:>4}", sim.roster().display_name(id)), tone),
            Span::styled(if sim.roster().kind(id) == Kind::Predator { format!("{held:>6} cells held") } else { "      no scent".to_string() }, if active { tone } else { theme::dim_text() }),
        ]));
        row += 1;
    }
    row
}

impl WorldMap {
    /// S02j sidebar: what scent is, the species ramp, held cells per region,
    /// the largest holders, the species list, a reading note and the Stack section.
    pub(super) fn scent_sidebar(f: &mut Frame<'_>, area: Rect, sim: &Sim, stack: &OverlayStack) {
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let mut row = 0u16;
        let sp = stack.species;
        let color = sim.roster().color(sp);
        let tone = |c: Color| Style::default().fg(c).bg(theme::PANEL_BG);
        let world = &sim.world;

        panel::section(f, inner, row, &format!("{} scent", sim.roster().display_name(sp)));
        row += 1;
        for note in [" adults mark where they walk and kill; marks", " fade daily and the strongest recent marker holds"] {
            util::line(f, inner, row, Line::from(Span::styled(note, theme::dim_text())));
            row += 1;
        }

        panel::section(f, inner, row, "Legend");
        row += 1;
        for i in 0..24 {
            let t = (f32::from(i) + 0.5) / 24.0;
            let c = theme::species_ramp(color, t);
            if let Some(cell) = f.buffer_mut().cell_mut((inner.x + 4 + i, inner.y + row)) {
                cell.set_char(glyphs::shade(t));
                cell.set_style(Style::default().fg(c).bg(theme::dim(c, 0.75)));
            }
        }
        row += 1;
        let hold = crate::cast!((sim.params.territory.hold_min * 100.0).round() => u32);
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" faint … strong   held from ", theme::dim_text()),
            Span::styled(format!("{hold}%"), tone(color)),
            Span::styled("  ▲ rock", theme::dim_text()),
        ]));
        row += 1;

        panel::section(f, inner, row, "By region");
        row += 1;
        let held = held_by_region(sim, sp);
        for (ri, r) in world.regions.iter().enumerate() {
            let share = crate::cast!(held[ri] => f32) / crate::cast!(world.region_size(ri).max(1) => f32);
            bars::labeled(f.buffer_mut(), inner, row, &format!(" {}", r.0), share, color, 18, 14);
            row += 1;
        }
        let total: u32 = held.iter().sum();
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {total} cells held"), if total == 0 { theme::dim_text() } else { theme::text() }),
            Span::styled("  of the land, by any holder", theme::dim_text()),
        ]));
        row += 1;

        panel::section(f, inner, row, "Holders");
        row += 1;
        let list = holders(sim, sp);
        if sim.roster().kind(sp) != Kind::Predator {
            util::line(f, inner, row, Line::from(Span::styled(" prey lay no scent", theme::dim_text())));
            row += 1;
        } else if list.is_empty() {
            util::line(f, inner, row, Line::from(Span::styled(" no ground is held yet", theme::dim_text())));
            row += 1;
        }
        for (id, cells) in list.iter().take(HOLDER_ROWS) {
            let (label, record) = match sim.creatures.get(*id) {
                Some(c) => (format!("{:<6} {:<8}", c.tag(sim.roster()), c.name_str(sim.roster()).chars().take(8).collect::<String>()), format!(" won {} lost {}", c.contests_won, c.contests_lost)),
                None => (format!("#{:<14}", id.0), " (dead)".to_string()),
            };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {} ", sim.roster().adult_glyph(sp)), tone(color).add_modifier(Modifier::BOLD)),
                Span::styled(label, theme::text()),
                Span::styled(format!("{cells:>4} cells"), tone(color)),
                Span::styled(record, theme::dim_text()),
            ]));
            row += 1;
        }
        row += 1;

        row = species_list(f, inner, row, sim, sp);
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" Tab next species · k look · Esc restores", theme::dim_text())));
        row += 2;
        Self::stack_rows(f, inner, row, stack);
    }
}
