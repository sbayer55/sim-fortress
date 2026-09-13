//! S08: the live lineage / family tree (C4 FR8), rooted `lineage_up`
//! generations above the focused creature along the mother line.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::Frame;

use crate::sim::creatures::{Cause, CreatureId};
use crate::sim::lineage::{LineageNode, Tree, TreeItem};
use crate::sim::species::IDX_RESISTANCE;
use crate::sim::{Genome, Kind, Sim, TRAIT_NAMES};
use crate::ui::app::AppState;
use crate::ui::screens::common::{day_stamp, sp};
use crate::ui::screens::s03_inspector::Inspector;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};
use std::fmt::Write as _;

const SIDE_W: u16 = 40;
const LEGEND_ROWS: u16 = 6;

#[derive(Debug)]
pub struct LineageScreen {
    pub focus: CreatureId,
}

impl LineageScreen {
    pub const fn new(focus: CreatureId) -> Self {
        Self { focus }
    }

    fn tree(&self, sim: &Sim) -> Option<Tree> {
        let gp = &sim.params.genetics;
        sim.lineage.tree(self.focus, gp.lineage_up, gp.lineage_rows_max)
    }
}

fn years(n: &LineageNode, season_days: u32) -> String {
    let year_len = i64::from(4 * season_days);
    let born = if n.born_day < 0 { "founder".to_string() } else { format!("Y{}", i64::from(n.born_day).div_euclid(year_len) + 1) };
    match n.died_day {
        Some(d) => format!("{}-Y{}", born, i64::from(d).div_euclid(year_len) + 1),
        None => format!("{born}-"),
    }
}

impl Screen for LineageScreen {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        let Some(sim) = app.sim.as_ref() else { return Action::Unhandled };
        let Some(tree) = self.tree(sim) else { return Action::Unhandled };
        let ids = tree.node_ids();
        let pos = ids.iter().position(|&id| id == self.focus).unwrap_or(0);
        match key.code {
            KeyCode::Up => {
                if pos > 0 {
                    self.focus = ids[pos - 1];
                }
                Action::None
            }
            KeyCode::Down => {
                if pos + 1 < ids.len() {
                    self.focus = ids[pos + 1];
                }
                Action::None
            }
            KeyCode::Left => {
                // The mother, when drawn.
                if let Some(m) = sim.lineage.get(self.focus).and_then(LineageNode::mother) {
                    if ids.contains(&m) {
                        self.focus = m;
                    }
                }
                Action::None
            }
            KeyCode::Right => {
                // The first drawn child.
                if let Some(n) = sim.lineage.get(self.focus) {
                    if let Some(&c) = n.children.iter().find(|c| ids.contains(c)) {
                        self.focus = c;
                    }
                }
                Action::None
            }
            KeyCode::Enter => {
                if sim.creatures.get(self.focus).is_some() {
                    Action::Push(Box::new(Inspector::new(self.focus)))
                } else {
                    Action::None // the side panel already shows the lineage node
                }
            }
            KeyCode::Esc => Action::Pop,
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else { return };
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let tree_area = Rect::new(area.x, area.y, area.width - SIDE_W, body_h);
        let side_area = Rect::new(area.x + area.width - SIDE_W, area.y, SIDE_W, body_h);

        let Some(tree) = self.tree(sim) else {
            let inner = panel::draw(f, tree_area, "Lineage", panel::Kind::Outer);
            util::line(f, inner, 1, Line::from(sp(" no lineage record for this creature", theme::dim_text())));
            status::render(f, Rect::new(area.x, status_row, area.width, 1), &[("Esc", "back")], "");
            return;
        };
        draw_tree(f, tree_area, sim, &tree);
        details(f, side_area, sim, self.focus);

        let focus = sim.lineage.get(self.focus);
        let ids = tree.node_ids();
        let pos = ids.iter().position(|&id| id == self.focus).map_or(0, |p| p + 1);
        let right = match focus {
            Some(n) => format!("{} {}  {} of {} {}", n.name_str(), n.tag, pos, tree.node_count, n.species.plural().to_lowercase()),
            None => String::new(),
        };
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("↑↓←→", "navigate"), ("Enter", "inspect"), ("Esc", "back")],
            &right,
        );
    }
}

fn draw_tree(f: &mut Frame<'_>, area: Rect, sim: &Sim, tree: &Tree) {
    let lin = &sim.lineage;
    let root = lin.get(tree.root);
    let species = root.map(|n| n.species);
    let ids = tree.node_ids();
    let nodes: Vec<&LineageNode> = ids.iter().filter_map(|&id| lin.get(id)).collect();
    let g0 = nodes.iter().map(|n| n.generation).min().unwrap_or(1);
    let g1 = nodes.iter().map(|n| n.generation).max().unwrap_or(g0);
    let title = match root {
        Some(n) => format!("Lineage of {} {}", n.name_str(), n.tag),
        None => "Lineage".to_string(),
    };
    let plural = species.map_or_else(|| "creatures".into(), |s| s.plural().to_lowercase());
    let inner = panel::draw_with_hint(f, area, &title, &format!("{} {}, {} generations", tree.node_count, plural, g1 - g0 + 1), panel::Kind::Outer);
    let season_days = sim.time.season_days;

    // Column header.
    let mut row = 0u16;
    util::line(f, inner, row, Line::from(vec![
        sp(" ", theme::text()),
        sp(format!("{:<46}", "ancestry (older on the left)"), theme::dim_text()),
        sp(format!("{:<9}", "gen"), theme::dim_text()),
        sp(format!("{:<14}", "years"), theme::dim_text()),
        sp("mutations", theme::dim_text()),
    ]));
    row += 2;

    // Layout budget: legend at the bottom, a by-generation table above it,
    // the tree scrolled so the focus stays visible in what remains.
    let gen_rows = (crate::cast!((g1 - g0 + 1) => u16)).min(6) + 2;
    let tree_rows = crate::cast!(inner.height.saturating_sub(row + gen_rows + LEGEND_ROWS + 1) => usize);
    let (start, end) = tree_window(tree, tree_rows);
    if start > 0 {
        util::line(f, inner, row - 1, Line::from(sp(format!("   {} {} rows above", glyphs::UP, start), theme::dim_text())));
    }
    for it in &tree.items[start..end] {
        let y = inner.y + row;
        match it {
            TreeItem::More { prefix, count, .. } => draw_more_row(f, inner, y, prefix, *count),
            TreeItem::Node { id, prefix, .. } => {
                let Some(n) = lin.get(*id) else { continue };
                draw_node_row(f, inner, y, n, *id == tree.focus, season_days, prefix);
            }
        }
        row += 1;
    }
    if end < tree.items.len() {
        util::line(f, inner, row, Line::from(sp(format!("   {} {} rows below", glyphs::DOWN, tree.items.len() - end), theme::dim_text())));
    }

    // Per-generation summary.
    draw_generation_table(f, inner, &nodes, g0, g1, species, &plural);

    // Legend + stats pinned to the bottom.
    draw_tree_legend(f, inner, &nodes, tree, lin, g0, g1);
}

/// The slice of tree items that fits, scrolled so the focus stays visible.
fn tree_window(tree: &Tree, tree_rows: usize) -> (usize, usize) {
    let focus_pos = tree.items.iter().position(|it| matches!(it, TreeItem::Node { id, .. } if *id == tree.focus)).unwrap_or(0);
    let start = if tree.items.len() <= tree_rows { 0 } else { focus_pos.saturating_sub(tree_rows.div_euclid(2)).min(tree.items.len() - tree_rows) };
    let end = (start + tree_rows).min(tree.items.len());
    (start, end)
}

/// A collapsed-branch marker row.
fn draw_more_row(f: &mut Frame<'_>, inner: Rect, y: u16, prefix: &str, count: usize) {
    let buf = f.buffer_mut();

            buf.set_stringn(inner.x + 1, y, prefix, 40, theme::border());
            let x = inner.x + 1 + crate::cast!(prefix.chars().count() => u16);
            buf.set_stringn(x, y, format!(" {} and {} more", glyphs::DOT, count), 20, theme::dim_text());
}

/// One creature row: ancestry prefix, name, generation, years and mutations.
fn draw_node_row(f: &mut Frame<'_>, inner: Rect, y: u16, n: &LineageNode, is_focus: bool, season_days: u32, prefix: &str) {
    let dead = !n.alive();
    let col = inner.x + 47;
    let name_style = node_name_style(n, is_focus, dead);
    let (x, name_w) = {
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x + 1, y, prefix, 40, theme::border());
        let x = inner.x + 1 + crate::cast!(prefix.chars().count() => u16);
        let label = format!(" {} {} ", n.name_str(), n.tag);
        let name_w = crate::cast!(label.chars().count() => u16);
        buf.set_stringn(x, y, &label, crate::cast!(name_w => usize), name_style);
        (x, name_w)
    };
    if is_focus {
        f.buffer_mut().set_stringn(x + name_w, y, format!("{} you are here", glyphs::REWIND), 14, Style::default().fg(theme::KEY).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
    }
    draw_life_columns(f.buffer_mut(), y, col, n, dead, season_days);
    draw_mutation_columns(f.buffer_mut(), inner, y, col, n);
}

/// The name style for a tree row: focus beats notable beats dead beats living.
fn node_name_style(n: &LineageNode, is_focus: bool, dead: bool) -> Style {
    if is_focus {
        theme::selected()
    } else if n.notable {
        theme::title()
    } else if dead {
        theme::dim_text()
    } else {
        theme::text()
    }
}

/// The generation, lifespan and health columns of one tree row.
fn draw_life_columns(buf: &mut ratatui::buffer::Buffer, y: u16, col: u16, n: &LineageNode, dead: bool, season_days: u32) {
    let gen_style = if dead { theme::dim_text() } else { theme::text() };
    buf.set_stringn(col, y, format!(" g{:<3}", n.generation), 6, gen_style);
    let yrs = years(n, season_days);
    let yrs_style = if dead { theme::dim_text() } else { Style::default().fg(theme::GOOD).bg(theme::PANEL_BG) };
    buf.set_stringn(col + 7, y, format!("{yrs:<11}"), 11, yrs_style);
    // C7: a disease death shows ☻ (SICK); survived infections add ☺N.
    let of_disease = dead && n.cause == Some(Cause::Disease);
    let status = if of_disease {
        format!("{} ", glyphs::DISEASE)
    } else if dead {
        format!("{} ", glyphs::DEATH)
    } else {
        format!("{} ", glyphs::BIRTH)
    };
    let status_style = if of_disease {
        Style::default().fg(theme::SICK).bg(theme::PANEL_BG)
    } else if dead {
        theme::dim_text()
    } else {
        Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)
    };
    buf.set_stringn(col + 18, y, &status, 2, status_style);
    if n.infections_survived > 0 {
        buf.set_stringn(col + 20, y, format!("{}{}", glyphs::IMMUNE, n.infections_survived.min(9)), 2, Style::default().fg(theme::IMMUNE).bg(theme::PANEL_BG));
    }
}

/// The per-mutation chips to the right of the life columns.
fn draw_mutation_columns(buf: &mut ratatui::buffer::Buffer, inner: Rect, y: u16, col: u16, n: &LineageNode) {
    let mut mx = col + 23;
    for m in &n.mutations {
        let s = format!("{} {} {:+.2}  ", glyphs::MUTATION, TRAIT_NAMES[m.trait_idx], m.delta);
        let w = crate::cast!(s.chars().count() => u16);
        if mx + w >= inner.right() {
            break;
        }
        let st = if n.notable {
            Style::default().fg(theme::INFO).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::INFO).bg(theme::PANEL_BG)
        };
        buf.set_stringn(mx, y, &s, crate::cast!(w => usize), st);
        mx += w;
    }
    if n.notable && mx + 9 < inner.right() {
        buf.set_stringn(mx, y, format!("{} notable", glyphs::DIAMOND), 9, theme::label());
    }
}

/// The by-generation summary table above the legend.
#[allow(clippy::too_many_arguments)]
fn draw_generation_table(f: &mut Frame<'_>, inner: Rect, nodes: &[&LineageNode], g0: u32, g1: u32, species: Option<crate::sim::SpeciesId>, plural: &str) {
    let gen_rows = (crate::cast!((g1 - g0 + 1) => u16)).min(6) + 2;
    let mut row = inner.height.saturating_sub(gen_rows + LEGEND_ROWS + 1);
    panel::section(f, inner, row, "By generation");
    row += 1;
    util::line(f, inner, row, Line::from(sp(format!("  gen   {plural:<8}alive sick mutations  members"), theme::dim_text())));
    row += 1;
    let color = species.map_or(theme::TEXT, |s| s.color());
    let mut shown = 0;
    for g in g0..=g1 {
        if shown >= 6 {
            break;
        }
        let members: Vec<&&LineageNode> = nodes.iter().filter(|n| n.generation == g).collect();
        if members.is_empty() {
            continue;
        }
        let alive_g = members.iter().filter(|n| n.alive()).count();
        let muts_g: usize = members.iter().map(|n| n.mutations.len()).sum();
        let sick_g = members.iter().filter(|n| n.cause == Some(Cause::Disease)).count();
        let names: Vec<String> = members.iter().take(12).map(|n| n.name_str().to_string()).collect();
        let y = inner.y + row;
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x + 1, y, format!(" g{g:<4}"), 7, theme::text());
        let blocks: String = std::iter::repeat_n(glyphs::SQUARE, members.len().min(7)).collect();
        buf.set_stringn(inner.x + 8, y, format!("{blocks:<7}"), 7, Style::default().fg(color).bg(theme::PANEL_BG));
        buf.set_stringn(inner.x + 15, y, format!("{alive_g:>5}"), 5, Style::default().fg(theme::GOOD).bg(theme::PANEL_BG));
        // C7: disease deaths in this generation.
        let sick_text = if sick_g > 0 { format!("{}{:<3}", glyphs::DISEASE, sick_g.min(999)) } else { "   -".to_string() };
        buf.set_stringn(inner.x + 21, y, format!("{sick_text:>4}"), 4, Style::default().fg(if sick_g > 0 { theme::SICK } else { theme::DIM }).bg(theme::PANEL_BG));
        let m: String = std::iter::repeat_n(glyphs::MUTATION, muts_g.min(9)).collect();
        buf.set_stringn(inner.x + 27, y, format!("{m:<9}"), 9, Style::default().fg(theme::INFO).bg(theme::PANEL_BG));
        let mut list = names.join(", ");
        if members.len() > 12 {
            let _ = write!(list, " {} {} more", glyphs::DOT, members.len() - 12);
        }
        buf.set_stringn(inner.x + 38, y, list, (crate::cast!(inner.width => usize)).saturating_sub(39), theme::dim_text());
        row += 1;
        shown += 1;
    }
}

/// The legend and totals pinned to the bottom of the panel.
fn draw_tree_legend(f: &mut Frame<'_>, inner: Rect, nodes: &[&LineageNode], tree: &Tree, lin: &crate::sim::Lineage, g0: u32, g1: u32) {
    let mut row;
    let alive = nodes.iter().filter(|n| n.alive()).count();
    let notable = nodes.iter().filter(|n| n.notable).count();
    let muts: usize = nodes.iter().map(|n| n.mutations.len()).sum();
    row = inner.height - LEGEND_ROWS;
    panel::section(f, inner, row, "Legend");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" ", theme::text()),
        sp(" focus ", theme::selected()),
        sp("   ", theme::text()),
        sp("notable", theme::title()),
        sp("   living", theme::text()),
        sp("   dead", theme::dim_text()),
        sp(format!("   {} mutation at birth", glyphs::MUTATION), Style::default().fg(theme::INFO).bg(theme::PANEL_BG)),
        sp(format!("   {} alive  {} dead", glyphs::BIRTH, glyphs::DEATH), theme::dim_text()),
        sp(format!("   {} died of disease", glyphs::DISEASE), Style::default().fg(theme::SICK).bg(theme::PANEL_BG)),
        sp(format!("  {}N infections survived", glyphs::IMMUNE), Style::default().fg(theme::IMMUNE).bg(theme::PANEL_BG)),
    ]));
    row += 2;
    util::line(f, inner, row, Line::from(sp(
        format!(" {} in tree: {} alive, {} dead   {} notable   {} mutations recorded   generations g{}..g{}", tree.node_count, alive, tree.node_count - alive, notable, muts, g0, g1),
        theme::dim_text(),
    )));
    row += 1;
    let focus = lin.get(tree.focus);
    let mother = focus.and_then(LineageNode::mother).and_then(|m| lin.get(m));
    let father = focus.and_then(LineageNode::father).and_then(|m| lin.get(m));
    util::line(f, inner, row, Line::from(vec![
        sp(" fathers are not drawn; ", theme::dim_text()),
        sp(match (mother, father) {
            (Some(m), Some(fa)) => format!("{} {} is the mother and {} {} the father of the focus.", m.name_str(), m.tag, fa.name_str(), fa.tag),
            _ => "the focus is a founder.".to_string(),
        }, theme::text()),
    ]));
}

fn details(f: &mut Frame<'_>, area: Rect, sim: &Sim, focus: CreatureId) {
    let lin = &sim.lineage;
    let Some(n) = lin.get(focus) else {
        panel::draw(f, area, "Focused creature", panel::Kind::Focus);
        return;
    };
    let live = sim.creatures.get(focus);
    let inner = panel::draw(f, area, &format!("Focused {}", n.species.name().to_lowercase()), panel::Kind::Focus);
    let mut row = 0u16;
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} ", n.species.glyph().to_ascii_uppercase()), Style::default().fg(n.species.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(n.name_str().to_string(), theme::title()),
        sp(format!("  {}", n.tag), theme::label()),
        sp(if n.alive() { "  alive" } else { "  dead" }, Style::default().fg(if n.alive() { theme::GOOD } else { theme::DIM }).bg(theme::PANEL_BG)),
    ]));
    row += 2;
    let season_days = sim.time.season_days;
    let day = crate::cast!(sim.time.day_index() => i64);
    for (k, v) in details_facts(n, live, lin, sim, focus, day, season_days) {
        util::line(f, inner, row, Line::from(vec![sp(format!(" {k:<13}"), theme::dim_text()), sp(v, theme::text())]));
        row += 1;
    }
    row = draw_mutations(f, inner, row + 1, n);
    row = draw_inheritance(f, inner, row + 1, n, lin, mother_of(n, lin), grand_of(n, lin), sim);
    row = draw_children(f, inner, row + 2, n, lin, season_days);
    row = draw_direct_line(f, inner, row + 1, sim, lin, focus);
    draw_siblings(f, inner, row + 1, focus, mother_of(n, lin), lin, season_days);
}

/// The focus's mother, if the lineage still holds her.
fn mother_of<'a>(n: &LineageNode, lin: &'a crate::sim::Lineage) -> Option<&'a LineageNode> {
    n.mother().and_then(|m| lin.get(m))
}

/// The focus's maternal grandmother, if the lineage still holds her.
fn grand_of<'a>(n: &LineageNode, lin: &'a crate::sim::Lineage) -> Option<&'a LineageNode> {
    mother_of(n, lin).and_then(LineageNode::mother).and_then(|g| lin.get(g))
}

/// The key/value fact list for the focused node.
fn details_facts(n: &LineageNode, live: Option<&crate::sim::Creature>, lin: &crate::sim::Lineage, sim: &Sim, focus: CreatureId, day: i64, season_days: u32) -> Vec<(String, String)> {
    let mother = n.mother().and_then(|m| lin.get(m));
    let father = n.father().and_then(|m| lin.get(m));
    let grand = mother.and_then(LineageNode::mother).and_then(|g| lin.get(g));
    let name_of = |x: Option<&LineageNode>| x.map_or_else(|| "unknown".into(), |p| format!("{} {}", p.name_str(), p.tag));
    let living_kids = n.children.iter().filter(|c| lin.get(**c).is_some_and(LineageNode::alive)).count();
    let (living_desc, _) = lin.living_descendants(focus, 5000);
    let desc_total = lin.descendants(focus, u32::MAX, 5000).len();
    let age_days = (n.died_day.map_or(day, i64::from) - i64::from(n.born_day)).max(0);
    let count_label = if n.species.kind() == Kind::Prey { "offspring" } else { "kills" };
    let count_value = match live {
        Some(c) if n.species.kind() == Kind::Prey => c.offspring.to_string(),
        Some(c) => c.kills.to_string(),
        None => n.children.len().to_string(),
    };
    let mut facts: Vec<(String, String)> = vec![
        ("generation".into(), format!("{}", n.generation)),
        ("born".into(), format!("{}  (age {} days)", day_stamp(i64::from(n.born_day), season_days), age_days)),
        ("died".into(), n.died_day.map_or_else(|| "still living".into(), |d| day_stamp(i64::from(d), season_days))),
    ];
    // C7: cause of death and, for disease, the outbreak it belonged to.
    if !n.alive() {
        if let Some(cause) = n.cause {
            facts.push(("died of".into(), cause.label().to_string()));
        }
        if let Some(o) = n.outbreak.and_then(|i| sim.disease.outbreak(i)) {
            let year = o.started_day.div_euclid((4 * season_days).max(1)) + 1;
            // The 38-column panel leaves 23 cells for the value.
            facts.push(("outbreak".into(), format!("{} outbreak, Y{}", sim.disease.name(o.pathogen), year)));
        }
    }
    facts.extend([
        ("mother".into(), name_of(mother)),
        ("father".into(), name_of(father)),
        ("grandmother".into(), name_of(grand)),
        ("children".into(), format!("{}  ({} living)", n.children.len(), living_kids)),
        ("descendants".into(), format!("{desc_total}  ({living_desc} living)")),
        (count_label.into(), count_value),
    ]);
    facts
}

/// The mutations section; returns the row after it.
fn draw_mutations(f: &mut Frame<'_>, inner: Rect, row: u16, n: &LineageNode) -> u16 {
    let mut row = row;
    row += 1;
    if n.mutations.is_empty() {
        util::line(f, inner, row, Line::from(sp(" none", theme::dim_text())));
        row += 1;
    }
    for m in &n.mutations {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", glyphs::MUTATION), Style::default().fg(theme::INFO).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{} {:+.2} (gen {})", TRAIT_NAMES[m.trait_idx], m.delta, m.generation), theme::text()),
        ]));
        row += 1;
    }
    row += 1;
    row
}

/// The trait-inheritance section; returns the row after it.
#[allow(clippy::too_many_arguments)]
fn draw_inheritance(
    f: &mut Frame<'_>,
    inner: Rect,
    row: u16,
    n: &LineageNode,
    lin: &crate::sim::Lineage,
    mother: Option<&LineageNode>,
    grand: Option<&LineageNode>,
    sim: &Sim,
) -> u16 {
    let mut row = row;
    row += 1;
    util::line(f, inner, row, Line::from(sp("            grand parent  self  kids", theme::dim_text())));
    row += 1;
    // The three traits that moved most from the mother.
    let kids_mean: Option<[f32; Genome::LEN]> = {
        let living: Vec<&LineageNode> = n.children.iter().filter_map(|c| lin.get(*c)).filter(|k| k.alive()).collect();
        if living.is_empty() {
            None
        } else {
            let mut m = [0.0f32; Genome::LEN];
            for k in &living {
                for t in 0..Genome::LEN {
                    m[t] += k.genome.0[t];
                }
            }
            for v in &mut m {
                *v /= crate::cast!(living.len() => f32);
            }
            Some(m)
        }
    };
    // C7: Resistance leads when the species has been through an outbreak.
    // C8: the genome grew to eleven traits, so this must name the resistance
    // *index*, not the last slot (which is now Maturity).
    let had_outbreak = sim.disease.outbreaks.iter().any(|o| o.species_cases[n.species.index()] > 0);
    let mut order: Vec<usize> = (0..Genome::LEN).filter(|&t| !(had_outbreak && t == IDX_RESISTANCE)).collect();
    order.sort_by(|&a, &b| {
        let da = mother.map_or(0.0, |m| (n.genome.0[a] - m.genome.0[a]).abs());
        let db = mother.map_or(0.0, |m| (n.genome.0[b] - m.genome.0[b]).abs());
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b))
    });
    if had_outbreak {
        order.insert(0, IDX_RESISTANCE);
    }
    for &t in order.iter().take(3) {
        let a = grand.map(|g| g.genome.0[t]);
        let b = mother.map(|m| m.genome.0[t]);
        let c = n.genome.0[t];
        let d = kids_mean.map(|k| k[t]);
        let y = inner.y + row;
        let buf = f.buffer_mut();
        let fmt = |v: Option<f32>| v.map_or_else(|| "  - ".into(), |x| format!("{x:.2}"));
        buf.set_stringn(inner.x, y, format!(" {:<11}", TRAIT_NAMES[t]), 12, theme::text());
        buf.set_stringn(inner.x + 12, y, fmt(a), 4, theme::dim_text());
        buf.set_stringn(inner.x + 18, y, fmt(b), 4, theme::text());
        buf.set_stringn(inner.x + 24, y, format!("{c:.2}"), 4, theme::selected());
        let dk = Style::default().fg(if d.unwrap_or(c) > c { theme::GOOD } else { theme::BAD }).bg(theme::PANEL_BG);
        buf.set_stringn(inner.x + 30, y, fmt(d), 4, dk);
        let vals: Vec<f32> = [a, b, Some(c), d].iter().flatten().copied().collect();
        let lo = vals.iter().copied().fold(f32::MAX, f32::min);
        let hi = vals.iter().copied().fold(f32::MIN, f32::max);
        bars::range(buf, inner.x + 12, y + 1, 26, lo, c, hi, n.species.color());
        let first = a.or(b).unwrap_or(c);
        let last = d.unwrap_or(c);
        buf.set_stringn(inner.x + 1, y + 1, format!("{:+.2}", last - first), 5, Style::default().fg(if last >= first { theme::GOOD } else { theme::BAD }).bg(theme::PANEL_BG));
        row += 2;
    }
    util::line(f, inner, row, Line::from(sp(" kids = mean of living children", theme::dim_text())));
    row += 2;
    row
}

/// The children section; returns the row after it.
fn draw_children(f: &mut Frame<'_>, inner: Rect, row: u16, n: &LineageNode, lin: &crate::sim::Lineage, season_days: u32) -> u16 {
    let mut row = row;
    row += 1;
    if n.children.is_empty() {
        util::line(f, inner, row, Line::from(sp(" none", theme::dim_text())));
        row += 1;
    }
    for c in n.children.iter().take(6) {
        let Some(k) = lin.get(*c) else { continue };
        let dead = !k.alive();
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", if dead { glyphs::DEATH } else { glyphs::BIRTH }), Style::default().fg(if dead { theme::DIM } else { theme::GOOD }).bg(theme::PANEL_BG)),
            sp(format!("{:<8}{:<7}", k.name_str(), k.tag), if dead { theme::dim_text() } else { theme::text() }),
            sp(format!("g{} {}", k.generation, years(k, season_days)), theme::dim_text()),
            sp(format!("  {}", if k.children.is_empty() { "" } else { "+kids" }), theme::label()),
        ]));
        row += 1;
    }
    if n.children.len() > 6 {
        util::line(f, inner, row, Line::from(sp(format!("   {} and {} more", glyphs::DOT, n.children.len() - 6), theme::dim_text())));
        row += 1;
    }
    row += 1;
    row
}

/// The direct maternal line; returns the row after it.
fn draw_direct_line(f: &mut Frame<'_>, inner: Rect, row: u16, sim: &Sim, lin: &crate::sim::Lineage, focus: CreatureId) -> u16 {
    let mut row = row;
    row += 1;
    let mut chain = vec![focus];
    let mut cur = focus;
    for _ in 0..sim.params.genetics.lineage_up {
        match lin.get(cur).and_then(LineageNode::mother) {
            Some(m) if lin.get(m).is_some() => {
                chain.push(m);
                cur = m;
            }
            _ => break,
        }
    }
    chain.reverse();
    for (i, id) in chain.iter().enumerate() {
        let Some(x) = lin.get(*id) else { continue };
        let st = if *id == focus { theme::selected() } else if !x.alive() { theme::dim_text() } else { theme::text() };
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {}{} ", " ".repeat(i), if i == 0 { ' ' } else { glyphs::RIGHT }), theme::border()),
            sp(format!("{} {}", x.name_str(), x.tag), st),
            sp(format!("  g{}", x.generation), theme::dim_text()),
        ]));
        row += 1;
    }
    row += 1;
    row
}

/// The siblings section.
fn draw_siblings(f: &mut Frame<'_>, inner: Rect, mut row: u16, focus: CreatureId, mother: Option<&LineageNode>, lin: &crate::sim::Lineage, season_days: u32) {
    row += 1;
    let sibs: Vec<&LineageNode> = mother
        .map(|m| m.children.iter().filter(|c| **c != focus).filter_map(|c| lin.get(*c)).collect())
        .unwrap_or_default();
    if sibs.is_empty() {
        util::line(f, inner, row, Line::from(sp(" none", theme::dim_text())));
    }
    for k in sibs.iter().take(5) {
        if row >= inner.height {
            break;
        }
        let dead = !k.alive();
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", if dead { glyphs::DEATH } else { glyphs::BIRTH }), Style::default().fg(if dead { theme::DIM } else { theme::GOOD }).bg(theme::PANEL_BG)),
            sp(format!("{:<8}{:<7}", k.name_str(), k.tag), if dead { theme::dim_text() } else { theme::text() }),
            sp(format!("g{} {}", k.generation, years(k, season_days)), theme::dim_text()),
        ]));
        row += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::disease::{Infection, Outbreak, PathogenId, Stage};
    use crate::sim::Params;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn screen_text(app: &AppState, screen: &dyn Screen) -> String {
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| screen.render(app, f, Rect::new(0, 0, 155, 45))).unwrap();
        let buf = terminal.backend().buffer();
        (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    }

    #[test]
    fn s08_disease_markers() {
        let mut app = AppState::new(Params::default());
        let mut sim = Sim::new(7, Params::default());
        let id = sim.creatures.living_ids()[0];
        sim.creatures.get_mut(id).unwrap().infection =
            Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 0, ends_day: 9, severity: 0.8, source: None, outbreak: 0 });
        // An outbreak touched the species; the lineage node records the disease
        // death in it and two survived infections.
        let species = sim.creatures.get(id).unwrap().species;
        let mut species_cases = [0u32; 6];
        species_cases[species.index()] = 4;
        sim.disease.outbreaks.push(Outbreak {
            pathogen: PathogenId(0),
            started_day: 0,
            ended_day: None,
            origin_region: 0,
            index_case: id,
            cases: 4,
            deaths: 1,
            recovered: 1,
            peak_active: 2,
            peak_day: 0,
            species_cases,
            species_deaths: [0; 6],
            epidemic: false,
            resist_at_start: [0.3; 6],
            resist_at_end: [0.0; 6],
            active: 2,
            cases_today: 0,
        });
        sim.lineage.record_death(id, 3, Cause::Disease, Some(0), 2);
        app.sim = Some(sim);
        let text = screen_text(&app, &LineageScreen::new(id));
        assert!(text.contains(&format!("{} died of disease", glyphs::DISEASE)), "legend missing: {text}");
        assert!(text.contains(&format!("{}2", glyphs::IMMUNE)), "survived marker missing: {text}");
        assert!(text.contains("died of      disease"), "facts missing: {text}");
        assert!(text.contains("outbreak, Y1"), "outbreak fact missing: {text}");
        assert!(text.contains(" Resistance "), "Resistance should lead trait inheritance: {text}");
    }
}
