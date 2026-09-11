//! S08: the live lineage / family tree (C4 FR8), rooted `lineage_up`
//! generations above the focused creature along the mother line.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::Frame;

use crate::sim::creatures::CreatureId;
use crate::sim::lineage::{LineageNode, Tree, TreeItem};
use crate::sim::{Kind, Sim, TRAIT_NAMES};
use crate::ui::app::AppState;
use crate::ui::screens::common::{day_stamp, sp};
use crate::ui::screens::s03_inspector::Inspector;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

const SIDE_W: u16 = 40;
const LEGEND_ROWS: u16 = 6;

pub struct LineageScreen {
    pub focus: CreatureId,
}

impl LineageScreen {
    pub fn new(focus: CreatureId) -> Self {
        LineageScreen { focus }
    }

    fn tree(&self, sim: &Sim) -> Option<Tree> {
        let gp = &sim.params.genetics;
        sim.lineage.tree(self.focus, gp.lineage_up, gp.lineage_rows_max)
    }
}

fn years(n: &LineageNode, season_days: u32) -> String {
    let year_len = (4 * season_days) as i64;
    let born = if n.born_day < 0 { "founder".to_string() } else { format!("Y{}", n.born_day as i64 / year_len + 1) };
    match n.died_day {
        Some(d) => format!("{}-Y{}", born, d as i64 / year_len + 1),
        None => format!("{}-", born),
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
                if let Some(m) = sim.lineage.get(self.focus).and_then(|n| n.mother()) {
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

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
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
        let pos = ids.iter().position(|&id| id == self.focus).map(|p| p + 1).unwrap_or(0);
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

fn draw_tree(f: &mut Frame, area: Rect, sim: &Sim, tree: &Tree) {
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
    let plural = species.map(|s| s.plural().to_lowercase()).unwrap_or_else(|| "creatures".into());
    let inner = panel::draw_with_hint(f, area, &title, &format!("{} {}, {} generations", tree.node_count, plural, g1 - g0 + 1), panel::Kind::Outer);
    let season_days = sim.time.season_days;

    // Column header.
    let mut row = 0u16;
    util::line(f, inner, row, Line::from(vec![
        sp(" ", theme::text()),
        sp(format!("{:<46}", "ancestry (older on the left)"), theme::dim_text()),
        sp(format!("{:<9}", "gen"), theme::dim_text()),
        sp(format!("{:<10}", "years"), theme::dim_text()),
        sp("mutations", theme::dim_text()),
    ]));
    row += 2;

    // Layout budget: legend at the bottom, a by-generation table above it,
    // the tree scrolled so the focus stays visible in what remains.
    let gen_rows = ((g1 - g0 + 1) as u16).min(6) + 2;
    let tree_rows = inner.height.saturating_sub(row + gen_rows + LEGEND_ROWS + 1) as usize;
    let focus_pos = tree.items.iter().position(|it| matches!(it, TreeItem::Node { id, .. } if *id == tree.focus)).unwrap_or(0);
    let start = if tree.items.len() <= tree_rows { 0 } else { focus_pos.saturating_sub(tree_rows / 2).min(tree.items.len() - tree_rows) };
    let end = (start + tree_rows).min(tree.items.len());
    if start > 0 {
        util::line(f, inner, row - 1, Line::from(sp(format!("   {} {} rows above", glyphs::UP, start), theme::dim_text())));
    }
    for it in &tree.items[start..end] {
        let y = inner.y + row;
        let buf = f.buffer_mut();
        match it {
            TreeItem::More { prefix, count, .. } => {
                buf.set_stringn(inner.x + 1, y, prefix, 40, theme::border());
                let x = inner.x + 1 + prefix.chars().count() as u16;
                buf.set_stringn(x, y, format!(" {} and {} more", glyphs::DOT, count), 20, theme::dim_text());
            }
            TreeItem::Node { id, prefix, .. } => {
                let Some(n) = lin.get(*id) else { continue };
                let is_focus = *id == tree.focus;
                let dead = !n.alive();
                buf.set_stringn(inner.x + 1, y, prefix, 40, theme::border());
                let x = inner.x + 1 + prefix.chars().count() as u16;
                let name_style = if is_focus {
                    theme::selected()
                } else if n.notable {
                    theme::title()
                } else if dead {
                    theme::dim_text()
                } else {
                    theme::text()
                };
                let label = format!(" {} {} ", n.name_str(), n.tag);
                let name_w = label.chars().count() as u16;
                buf.set_stringn(x, y, &label, name_w as usize, name_style);
                if is_focus {
                    buf.set_stringn(x + name_w, y, format!("{} you are here", glyphs::REWIND), 14, Style::default().fg(theme::KEY).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
                }
                let col = inner.x + 47;
                let gen_style = if dead { theme::dim_text() } else { theme::text() };
                buf.set_stringn(col, y, format!(" g{:<3}", n.generation), 6, gen_style);
                let yrs = years(n, season_days);
                let yrs_style = if dead { theme::dim_text() } else { Style::default().fg(theme::GOOD).bg(theme::PANEL_BG) };
                buf.set_stringn(col + 7, y, format!("{:<11}", yrs), 11, yrs_style);
                let status = if dead { format!("{} ", glyphs::DEATH) } else { format!("{} ", glyphs::BIRTH) };
                let status_style = if dead { theme::dim_text() } else { Style::default().fg(theme::GOOD).bg(theme::PANEL_BG) };
                buf.set_stringn(col + 18, y, &status, 2, status_style);
                let mut mx = col + 20;
                for m in &n.mutations {
                    let s = format!("{} {} {:+.2}  ", glyphs::MUTATION, TRAIT_NAMES[m.trait_idx], m.delta);
                    let w = s.chars().count() as u16;
                    if mx + w >= inner.right() {
                        break;
                    }
                    let st = if n.notable {
                        Style::default().fg(theme::INFO).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme::INFO).bg(theme::PANEL_BG)
                    };
                    buf.set_stringn(mx, y, &s, w as usize, st);
                    mx += w;
                }
                if n.notable && mx + 9 < inner.right() {
                    buf.set_stringn(mx, y, format!("{} notable", glyphs::DIAMOND), 9, theme::label());
                }
            }
        }
        row += 1;
    }
    if end < tree.items.len() {
        util::line(f, inner, row, Line::from(sp(format!("   {} {} rows below", glyphs::DOWN, tree.items.len() - end), theme::dim_text())));
    }

    // Per-generation summary.
    row = inner.height.saturating_sub(gen_rows + LEGEND_ROWS + 1);
    panel::section(f, inner, row, "By generation");
    row += 1;
    util::line(f, inner, row, Line::from(sp(format!("  gen   {:<8}alive  mutations  members", plural), theme::dim_text())));
    row += 1;
    let color = species.map(|s| s.color()).unwrap_or(theme::TEXT);
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
        let names: Vec<String> = members.iter().take(12).map(|n| n.name_str().to_string()).collect();
        let y = inner.y + row;
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x + 1, y, format!(" g{:<4}", g), 7, theme::text());
        let blocks: String = std::iter::repeat_n(glyphs::SQUARE, members.len().min(7)).collect();
        buf.set_stringn(inner.x + 8, y, format!("{:<7}", blocks), 7, Style::default().fg(color).bg(theme::PANEL_BG));
        buf.set_stringn(inner.x + 15, y, format!("{:>5}", alive_g), 5, Style::default().fg(theme::GOOD).bg(theme::PANEL_BG));
        let m: String = std::iter::repeat_n(glyphs::MUTATION, muts_g.min(9)).collect();
        buf.set_stringn(inner.x + 24, y, format!("{:<9}", m), 9, Style::default().fg(theme::INFO).bg(theme::PANEL_BG));
        let mut list = names.join(", ");
        if members.len() > 12 {
            list.push_str(&format!(" {} {} more", glyphs::DOT, members.len() - 12));
        }
        buf.set_stringn(inner.x + 34, y, list, (inner.width as usize).saturating_sub(35), theme::dim_text());
        row += 1;
        shown += 1;
    }

    // Legend + stats pinned to the bottom.
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
    ]));
    row += 2;
    util::line(f, inner, row, Line::from(sp(
        format!(" {} in tree: {} alive, {} dead   {} notable   {} mutations recorded   generations g{}..g{}", tree.node_count, alive, tree.node_count - alive, notable, muts, g0, g1),
        theme::dim_text(),
    )));
    row += 1;
    let focus = lin.get(tree.focus);
    let mother = focus.and_then(|n| n.mother()).and_then(|m| lin.get(m));
    let father = focus.and_then(|n| n.father()).and_then(|m| lin.get(m));
    util::line(f, inner, row, Line::from(vec![
        sp(" fathers are not drawn; ", theme::dim_text()),
        sp(match (mother, father) {
            (Some(m), Some(fa)) => format!("{} {} is the mother and {} {} the father of the focus.", m.name_str(), m.tag, fa.name_str(), fa.tag),
            _ => "the focus is a founder.".to_string(),
        }, theme::text()),
    ]));
}

fn details(f: &mut Frame, area: Rect, sim: &Sim, focus: CreatureId) {
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
    let day = sim.time.day_index() as i64;
    let mother = n.mother().and_then(|m| lin.get(m));
    let father = n.father().and_then(|m| lin.get(m));
    let grand = mother.and_then(|m| m.mother()).and_then(|g| lin.get(g));
    let name_of = |x: Option<&LineageNode>| x.map(|p| format!("{} {}", p.name_str(), p.tag)).unwrap_or_else(|| "unknown".into());
    let living_kids = n.children.iter().filter(|c| lin.get(**c).is_some_and(|k| k.alive())).count();
    let (living_desc, _) = lin.living_descendants(focus, 5000);
    let desc_total = lin.descendants(focus, u32::MAX, 5000).len();
    let age_days = (n.died_day.map(|d| d as i64).unwrap_or(day) - n.born_day as i64).max(0);
    let count_label = if n.species.kind() == Kind::Prey { "offspring" } else { "kills" };
    let count_value = match live {
        Some(c) if n.species.kind() == Kind::Prey => c.offspring.to_string(),
        Some(c) => c.kills.to_string(),
        None => n.children.len().to_string(),
    };
    let facts: Vec<(String, String)> = vec![
        ("generation".into(), format!("{}", n.generation)),
        ("born".into(), format!("{}  (age {} days)", day_stamp(n.born_day as i64, season_days), age_days)),
        ("died".into(), n.died_day.map(|d| day_stamp(d as i64, season_days)).unwrap_or_else(|| "still living".into())),
        ("mother".into(), name_of(mother)),
        ("father".into(), name_of(father)),
        ("grandmother".into(), name_of(grand)),
        ("children".into(), format!("{}  ({} living)", n.children.len(), living_kids)),
        ("descendants".into(), format!("{}  ({} living)", desc_total, living_desc)),
        (count_label.into(), count_value),
    ];
    for (k, v) in facts {
        util::line(f, inner, row, Line::from(vec![sp(format!(" {:<13}", k), theme::dim_text()), sp(v, theme::text())]));
        row += 1;
    }
    row += 1;
    panel::section(f, inner, row, "Mutations");
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

    panel::section(f, inner, row, "Trait inheritance");
    row += 1;
    util::line(f, inner, row, Line::from(sp("            grand parent  self  kids", theme::dim_text())));
    row += 1;
    // The three traits that moved most from the mother.
    let kids_mean: Option<[f32; 8]> = {
        let living: Vec<&LineageNode> = n.children.iter().filter_map(|c| lin.get(*c)).filter(|k| k.alive()).collect();
        if living.is_empty() {
            None
        } else {
            let mut m = [0.0f32; 8];
            for k in &living {
                for t in 0..8 {
                    m[t] += k.genome.0[t];
                }
            }
            for v in m.iter_mut() {
                *v /= living.len() as f32;
            }
            Some(m)
        }
    };
    let mut order: Vec<usize> = (0..8).collect();
    order.sort_by(|&a, &b| {
        let da = mother.map(|m| (n.genome.0[a] - m.genome.0[a]).abs()).unwrap_or(0.0);
        let db = mother.map(|m| (n.genome.0[b] - m.genome.0[b]).abs()).unwrap_or(0.0);
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b))
    });
    for &t in order.iter().take(3) {
        let a = grand.map(|g| g.genome.0[t]);
        let b = mother.map(|m| m.genome.0[t]);
        let c = n.genome.0[t];
        let d = kids_mean.map(|k| k[t]);
        let y = inner.y + row;
        let buf = f.buffer_mut();
        let fmt = |v: Option<f32>| v.map(|x| format!("{:.2}", x)).unwrap_or_else(|| "  - ".into());
        buf.set_stringn(inner.x, y, format!(" {:<11}", TRAIT_NAMES[t]), 12, theme::text());
        buf.set_stringn(inner.x + 12, y, fmt(a), 4, theme::dim_text());
        buf.set_stringn(inner.x + 18, y, fmt(b), 4, theme::text());
        buf.set_stringn(inner.x + 24, y, format!("{:.2}", c), 4, theme::selected());
        let dk = Style::default().fg(if d.unwrap_or(c) > c { theme::GOOD } else { theme::BAD }).bg(theme::PANEL_BG);
        buf.set_stringn(inner.x + 30, y, fmt(d), 4, dk);
        let vals: Vec<f32> = [a, b, Some(c), d].iter().flatten().copied().collect();
        let lo = vals.iter().cloned().fold(f32::MAX, f32::min);
        let hi = vals.iter().cloned().fold(f32::MIN, f32::max);
        bars::range(buf, inner.x + 12, y + 1, 26, lo, c, hi, n.species.color());
        let first = a.or(b).unwrap_or(c);
        let last = d.unwrap_or(c);
        buf.set_stringn(inner.x + 1, y + 1, format!("{:+.2}", last - first), 5, Style::default().fg(if last >= first { theme::GOOD } else { theme::BAD }).bg(theme::PANEL_BG));
        row += 2;
    }
    util::line(f, inner, row, Line::from(sp(" kids = mean of living children", theme::dim_text())));
    row += 2;

    panel::section(f, inner, row, "Children");
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

    panel::section(f, inner, row, "Direct line");
    row += 1;
    let mut chain = vec![focus];
    let mut cur = focus;
    for _ in 0..sim.params.genetics.lineage_up {
        match lin.get(cur).and_then(|x| x.mother()) {
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
    panel::section(f, inner, row, "Siblings");
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
