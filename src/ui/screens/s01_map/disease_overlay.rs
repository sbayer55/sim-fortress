//! S02h disease overlay (C7).

use std::collections::HashMap;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use crate::sim::creatures::CreatureId;
use crate::sim::disease::{self, PathogenId};
use crate::sim::{Sim, World};
use crate::ui::style::{SpeciesStyle};
use crate::widgets::map::{self, OverlayStack};
use crate::widgets::{panel, util};
use crate::{glyphs, theme};
use super::WorldMap;
use super::parasites::disease_parasites;

/// The pathogen roster, one row per slot, strains indented under their parent.
#[allow(clippy::too_many_arguments)]
fn disease_pathogens(f: &mut Frame<'_>, inner: Rect, mut row: u16, ds: &disease::DiseaseState, shown: Option<PathogenId>, day: u32) -> u16 {
    let tone = |c: Color| Style::default().fg(c).bg(theme::PANEL_BG);
        // Pathogens: one row per slot, strains indented under their parent.
        let title = match shown {
            None => "Pathogens · all".to_string(),
            Some(p) => format!("Pathogens · {}", ds.name(p)),
        };
        panel::section(f, inner, row, &title);
        row += 1;
        for (i, p) in ds.pathogens.iter().enumerate() {
            let id = PathogenId(crate::cast!(i => u8));
            let sel = shown == Some(id);
            let st = &ds.stats[i];
            let (status, color, bold) = if p.extinct {
                ("extinct", theme::DIM, false)
            } else {
                match ds.open_outbreak(id) {
                    Some((_, o)) if o.epidemic => ("EPIDEMIC", theme::SICK, true),
                    Some(_) => ("outbreak", theme::SICK, false),
                    None => ("dormant", theme::DIM, false),
                }
            };
            let is_new = p.is_strain() && p.born_day.is_some_and(|b| day < b.saturating_add(30));
            let name: String = if p.is_strain() { format!("└ {}", p.name()) } else { p.name().to_string() }.chars().take(11).collect();
            let text = if sel { theme::selected() } else if p.extinct { theme::dim_text() } else { theme::text() };
            let bg = if sel { theme::SELECT_BG } else { theme::PANEL_BG };
            let mut status_style = Style::default().fg(color).bg(bg);
            if bold {
                status_style = status_style.add_modifier(Modifier::BOLD);
            }
            util::line(f, inner, row, Line::from(vec![
                Span::styled(if sel { "►" } else { " " }, text),
                Span::styled(format!("{name:<11}"), text),
                Span::styled(format!(" act{:>3}", st.active), if st.active > 0 { tone(theme::SICK).bg(bg) } else { Style::default().fg(theme::DIM).bg(bg) }),
                Span::styled(format!(" dead{:>3} ", st.total_deaths), if p.extinct { Style::default().fg(theme::DIM).bg(bg) } else { text }),
                Span::styled(format!("{status:<8}"), status_style),
                Span::styled(if is_new { " new" } else { "" }, Style::default().fg(theme::MAGENTA).bg(bg).add_modifier(Modifier::BOLD)),
            ]));
            row += 1;
        }
    row
}

/// The shown pathogen's open outbreak (or its latest), with the episode summary.
#[allow(clippy::too_many_arguments)]
fn disease_outbreak(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, ds: &disease::DiseaseState, world: &World, shown: Option<PathogenId>, day: u32) -> u16 {
    let tone = |c: Color| Style::default().fg(c).bg(theme::PANEL_BG);
        // This outbreak: the shown pathogen's open outbreak, else its latest;
        // for `all`, the latest open outbreak of any pathogen, else the latest.
        let outbreak = match shown {
            Some(p) => ds.open_outbreak(p).map(|(_, o)| o).or_else(|| ds.outbreaks.iter().rev().find(|o| o.pathogen == p)),
            None => ds.outbreaks.iter().rev().find(|o| o.ended_day.is_none()).or_else(|| ds.outbreaks.last()),
        };
        panel::section(f, inner, row, "This outbreak");
        row += 1;
        if let Some(o) = outbreak {
            let (y, d) = (o.started_day.div_euclid(360) + 1, o.started_day % 360 + 1);
            let dur = o.duration_days(day);
            let when = if o.ended_day.is_some() { format!(" · over after {dur} d") } else { format!(" · day {}", dur + 1) };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {}", ds.name(o.pathogen)), tone(theme::SICK).add_modifier(Modifier::BOLD)),
                Span::styled(format!(" began Year {y}, Day {d}"), theme::text()),
                Span::styled(when, theme::dim_text()),
            ]));
            row += 1;
            let origin = world.regions.get(crate::cast!(o.origin_region => usize)).map_or("The Wilds", |r| r.0.as_str());
            util::line(f, inner, row, Line::from(vec![Span::styled(" origin ", theme::dim_text()), Span::styled(origin, theme::text())]));
            row += 1;
            let index = match sim.creatures.get(o.index_case) {
                Some(c) => format!("{} {} ({})", c.tag(sim.roster()), c.name_str(sim.roster()), sim.roster().name(c.species)),
                None => format!("#{}", o.index_case.0),
            };
            util::line(f, inner, row, Line::from(vec![Span::styled(" index case ", theme::dim_text()), Span::styled(index, theme::text())]));
            row += 1;
            util::line(f, inner, row, Line::from(vec![
                Span::styled(" cases ", theme::dim_text()),
                Span::styled(o.cases.to_string(), tone(theme::SICK)),
                Span::styled("  deaths ", theme::dim_text()),
                Span::styled(o.deaths.to_string(), tone(theme::BAD)),
                Span::styled("  recovered ", theme::dim_text()),
                Span::styled(o.recovered.to_string(), tone(theme::GOOD)),
            ]));
            row += 1;
            let arrow = active_arrow(sim.series.samples(), Some(o.pathogen), 7);
            util::line(f, inner, row, Line::from(vec![
                Span::styled(" today ", theme::dim_text()),
                Span::styled(format!("+{} new ", o.cases_today), theme::text()),
                Span::styled(arrow.to_string(), tone(crate::ui::screens::common::arrow_color(arrow))),
                Span::styled(format!("  active {}", o.active), theme::dim_text()),
            ]));
            row += 1;
        } else {
            util::line(f, inner, row, Line::from(Span::styled(" no outbreak recorded yet", theme::dim_text())));
            row += 5;
        }
    row
}

/// Per-species sick / immune counts and mean Resistance.
#[allow(clippy::too_many_arguments)]
fn disease_by_species(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, shown: Option<PathogenId>, slots: usize, day: u32) -> u16 {
    let tone = |c: Color| Style::default().fg(c).bg(theme::PANEL_BG);
        // By species: living, sick with / immune to the shown pathogen(s), and
        // mean Resistance against the species base.
        panel::section(f, inner, row, "By species");
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled("   species    n  sick immune resist", theme::dim_text())));
        row += 1;
        let resist = disease::mean_resistance(&sim.creatures, sim.roster().len());
        for id in sim.roster().ids() {
            let (mut n, mut sick, mut immune) = (0u32, 0u32, 0u32);
            for c in sim.creatures.living().filter(|c| c.species == id) {
                n += 1;
                if c.infection.is_some_and(|inf| shown.is_none_or(|p| p == inf.pathogen)) {
                    sick += 1;
                }
                if immune_to_shown(c, shown, slots, day) {
                    immune += 1;
                }
            }
            let base = sim.roster().base_genome(id).resistance();
            let mean = resist[id.index()];
            let text = if n == 0 { theme::dim_text() } else { theme::text() };
            let arrow = if n == 0 {
                ' '
            } else if mean > base + 0.005 {
                glyphs::UP
            } else if mean < base - 0.005 {
                glyphs::DOWN
            } else {
                glyphs::FLAT
            };
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {} ", sim.roster().adult_glyph(id)), tone(sim.roster().color(id)).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<8}{n:>4}", sim.roster().display_name(id)), text),
                Span::styled(format!("{sick:>6}"), if sick > 0 { tone(theme::SICK) } else { theme::dim_text() }),
                Span::styled(format!("{immune:>7}"), if immune > 0 { tone(theme::IMMUNE) } else { theme::dim_text() }),
                Span::styled(if n == 0 { "    —".to_string() } else { format!("  {}", fmt2(mean)) }, text),
                Span::styled(arrow.to_string(), tone(crate::ui::screens::common::arrow_color(arrow))),
            ]));
            row += 1;
        }
    row
}

/// S02h: every living creature's colour under the disease overlay.
pub(super) fn disease_tints(sim: &Sim, shown: Option<PathogenId>) -> HashMap<CreatureId, (Color, bool)> {
    let day = crate::cast!(sim.time.day_index() => u32);
    let slots = sim.disease.pathogens.len();
    sim.creatures
        .living()
        .map(|c| {
            let infection = c.infection.map(|i| (i.pathogen, i.stage));
            (c.id, map::disease_tint(sim.roster().color(c.species), shown, infection, immune_to_shown(c, shown, slots, day), c.parasite_load))
        })
        .collect()
}

/// Immune to the shown pathogen, or to any of the `slots` live slots when all
/// are shown.
pub(super) fn immune_to_shown(c: &crate::sim::creatures::Creature, shown: Option<PathogenId>, slots: usize, day: u32) -> bool {
    match shown {
        Some(p) => disease::is_immune(c, p, day),
        None => (0..slots).any(|k| disease::is_immune(c, PathogenId(crate::cast!(k => u8)), day)),
    }
}

/// A 0..1 value as `.xx`.
pub(super) fn fmt2(v: f32) -> String {
    format!(".{:02}", crate::cast!((v * 100.0).round().clamp(0.0, 99.0) => u32))
}

/// Trend arrow of the active case count over the last `window` samples: one
/// pathogen slot, or every slot summed. Same ±3 % rule as the population arrow.
fn active_arrow(samples: &[crate::sim::Sample], shown: Option<PathogenId>, window: usize) -> char {
    if samples.len() < 2 {
        return glyphs::FLAT;
    }
    let active = |s: &crate::sim::Sample| match shown {
        Some(p) => s.active_by_pathogen.get(crate::cast!(p.0 => usize)).copied().unwrap_or(0),
        None => s.active_by_pathogen.iter().sum(),
    };
    let a = crate::cast!(active(&samples[samples.len().saturating_sub(window).min(samples.len() - 1)]) => f32);
    let b = crate::cast!(samples.last().map_or(0, active) => f32);
    let pct = if a > 0.0 { (b - a) / a * 100.0 } else if b > 0.0 { f32::INFINITY } else { 0.0 };
    if pct > 3.0 {
        glyphs::UP
    } else if pct < -3.0 {
        glyphs::DOWN
    } else {
        glyphs::FLAT
    }
}

impl WorldMap {
    /// S02h sidebar: what the colours mean, the pathogen roster with the shown
    /// slot marked, that pathogen's outbreak, every species' sick / immune
    /// counts and mean Resistance, a parasite summary, the reading notes and
    /// the Stack section. Fixed sections take 24 rows plus one per pathogen
    /// slot; the rows left over separate the sections and lengthen the
    /// reading notes.
    pub(super) fn disease_sidebar(f: &mut Frame<'_>, area: Rect, sim: &Sim, shown: Option<PathogenId>, stack: &OverlayStack) {
        let inner = panel::draw(f, area, "Overlay", panel::Kind::Outer);
        let mut row = 0u16;
        let tone = |c: Color| Style::default().fg(c).bg(theme::PANEL_BG);
        let day = crate::cast!(sim.time.day_index() => u32);
        let ds = &sim.disease;
        let world = &sim.world;
        let slots = ds.pathogens.len();
        let mut spare = crate::cast!(disease::MAX_PATHOGENS.saturating_sub(slots) => u16);
        let reading_extra = spare.min(2);
        spare -= reading_extra;
        let gap = |row: &mut u16, spare: &mut u16| {
            if *spare > 0 {
                *spare -= 1;
                *row += 1;
            }
        };

        panel::section(f, inner, row, "Disease");
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} sick", glyphs::DISEASE), tone(theme::SICK).add_modifier(Modifier::BOLD)),
            Span::styled(" bright, ", theme::dim_text()),
            Span::styled("incubating", tone(theme::dim(theme::SICK, 0.4))),
            Span::styled(" dim, ", theme::dim_text()),
            Span::styled(format!("{} immune", glyphs::IMMUNE), tone(theme::IMMUNE)),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" amber", tone(theme::WARN)),
            Span::styled(" = worms; fouled ground is tinted.", theme::dim_text()),
        ]));
        row += 1;
        gap(&mut row, &mut spare);

        row = disease_pathogens(f, inner, row, ds, shown, day);
        gap(&mut row, &mut spare);
        row = disease_outbreak(f, inner, row, sim, ds, world, shown, day);
        gap(&mut row, &mut spare);

        row = disease_by_species(f, inner, row, sim, shown, slots, day);
        gap(&mut row, &mut spare);
        row = disease_parasites(f, inner, row, sim, world);
        gap(&mut row, &mut spare);

        let notes = [" colour = animal · amber ground = fouled", " Tab pathogen · k look · Esc restores map"];
        if reading_extra == 0 {
            util::line(f, inner, row, Line::from(Span::styled(notes[1], theme::dim_text())));
            row += 1;
        } else {
            panel::section(f, inner, row, "Reading the map");
            row += 1;
            for note in notes.iter().take(crate::cast!(reading_extra => usize)) {
                util::line(f, inner, row, Line::from(Span::styled(*note, theme::dim_text())));
                row += 1;
            }
        }
        gap(&mut row, &mut spare);
        Self::stack_rows(f, inner, row, stack);
    }
}
