//! S08: lineage / family tree for the hero wolf.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::Prototype;
use crate::fixtures::{self, lineage::Lineage};
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

pub struct LineageView;

pub fn all() -> Vec<Box<dyn Prototype>> {
    vec![Box::new(LineageView)]
}

const SIDE_W: u16 = 40;

fn sp(s: impl Into<String>, st: Style) -> Span<'static> {
    Span::styled(s.into(), st)
}

/// One printable row of the tree: prefix box-drawing, node index.
struct Row {
    prefix: String,
    node: usize,
}

fn flatten(l: &Lineage, node: usize, prefix: &str, last: bool, root: bool, out: &mut Vec<Row>) {
    let branch = if root {
        String::new()
    } else if last {
        format!("{}└──", prefix)
    } else {
        format!("{}├──", prefix)
    };
    out.push(Row { prefix: branch, node });
    let child_prefix = if root {
        String::new()
    } else if last {
        format!("{}   ", prefix)
    } else {
        format!("{}│  ", prefix)
    };
    let kids = &l.nodes[node].children;
    for (i, &k) in kids.iter().enumerate() {
        flatten(l, k, &child_prefix, i + 1 == kids.len(), false, out);
    }
}

fn descendants(l: &Lineage, node: usize) -> usize {
    l.nodes[node].children.iter().map(|&k| 1 + descendants(l, k)).sum()
}

fn parent_of(l: &Lineage, node: usize) -> Option<usize> {
    l.nodes.iter().position(|n| n.children.contains(&node))
}

fn years(n: &fixtures::lineage::Node) -> String {
    match n.died_year {
        Some(d) => format!("Y{}-Y{}", n.born_year, d),
        None => format!("Y{}-", n.born_year),
    }
}

impl Prototype for LineageView {
    fn id(&self) -> &'static str {
        "S08a"
    }
    fn name(&self) -> &'static str {
        "Lineage / Family Tree"
    }
    fn variant(&self) -> &'static str {
        "Ashfang w#042"
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        let fx = fixtures::get();
        let l = &fx.lineage;
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let tree_area = Rect::new(area.x, area.y, area.width - SIDE_W, body_h);
        let side_area = Rect::new(area.x + area.width - SIDE_W, area.y, SIDE_W, body_h);

        tree(f, tree_area, l);
        details(f, side_area, fx);

        let focus = &l.nodes[l.focus];
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("↑↓←→", "navigate"), ("Enter", "inspect"), ("Esc", "back")],
            &format!("{} {}  {} of {} wolves", focus.name, focus.tag, l.focus + 1, l.nodes.len()),
        );
    }
}

fn tree(f: &mut Frame, area: Rect, l: &Lineage) {
    let root = &l.nodes[l.root];
    let inner = panel::draw_with_hint(
        f,
        area,
        &format!("Lineage of {} {}", root.name, root.tag),
        &format!("{} wolves, {} generations", l.nodes.len(), l.nodes.iter().map(|n| n.generation).max().unwrap_or(0) - root.generation + 1),
        panel::Kind::Outer,
    );
    let mut rows = Vec::new();
    flatten(l, l.root, "", true, true, &mut rows);

    // Column header: generation axis across the top.
    let mut row = 0u16;
    util::line(f, inner, row, Line::from(vec![
        sp(" ", theme::text()),
        sp(format!("{:<46}", "ancestry (older on the left)"), theme::dim_text()),
        sp(format!("{:<9}", "gen"), theme::dim_text()),
        sp(format!("{:<10}", "years"), theme::dim_text()),
        sp("mutations", theme::dim_text()),
    ]));
    row += 2;

    for r in &rows {
        let n = &l.nodes[r.node];
        let is_focus = r.node == l.focus;
        let dead = n.died_year.is_some();
        let y = inner.y + row;
        let buf = f.buffer_mut();
        // Box-drawing prefix.
        buf.set_stringn(inner.x + 1, y, &r.prefix, 40, theme::border());
        let x = inner.x + 1 + r.prefix.chars().count() as u16;
        let name_style = if is_focus {
            theme::selected()
        } else if n.notable {
            theme::title()
        } else if dead {
            theme::dim_text()
        } else {
            theme::text()
        };
        let label = format!(" {} {} ", n.name, n.tag);
        let name_w = label.chars().count() as u16;
        buf.set_stringn(x, y, &label, name_w as usize, name_style);
        if is_focus {
            buf.set_stringn(x + name_w, y, format!("{} you are here", glyphs::REWIND), 14, Style::default().fg(theme::KEY).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
        }
        // Fixed columns to the right: generation, years, mutations.
        let col = inner.x + 47;
        let gen_style = if dead { theme::dim_text() } else { theme::text() };
        buf.set_stringn(col, y, format!(" g{:<3}", n.generation), 6, gen_style);
        let yrs = years(n);
        let yrs_style = if dead {
            theme::dim_text()
        } else {
            Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)
        };
        buf.set_stringn(col + 9, y, format!("{:<9}", yrs), 9, yrs_style);
        let status = if dead { format!("{} ", glyphs::DEATH) } else { format!("{} ", glyphs::BIRTH) };
        let status_style = if dead { theme::dim_text() } else { Style::default().fg(theme::GOOD).bg(theme::PANEL_BG) };
        buf.set_stringn(col + 18, y, &status, 2, status_style);
        let mut mx = col + 20;
        for m in &n.mutations {
            let s = format!("{} {}  ", glyphs::MUTATION, m);
            let w = s.chars().count() as u16;
            let st = if n.notable {
                Style::default().fg(theme::INFO).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::INFO).bg(theme::PANEL_BG)
            };
            buf.set_stringn(mx, y, &s, w as usize, st);
            mx += w;
        }
        if n.notable && mx + 8 < inner.right() {
            buf.set_stringn(mx, y, format!("{} notable", glyphs::DIAMOND), 9, theme::label());
        }
        row += 1;
    }

    // Per-generation summary under the tree.
    row += 1;
    panel::section(f, inner, row, "By generation");
    row += 1;
    let g0 = root.generation;
    let g1 = l.nodes.iter().map(|n| n.generation).max().unwrap_or(g0);
    util::line(f, inner, row, Line::from(sp("  gen   wolves  alive  mutations  members", theme::dim_text())));
    row += 1;
    for g in g0..=g1 {
        let members: Vec<&fixtures::lineage::Node> = l.nodes.iter().filter(|n| n.generation == g).collect();
        let alive_g = members.iter().filter(|n| n.died_year.is_none()).count();
        let muts_g: usize = members.iter().map(|n| n.mutations.len()).sum();
        let names: Vec<String> = members.iter().map(|n| n.name.clone()).collect();
        let y = inner.y + row;
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x + 1, y, format!(" g{:<4}", g), 7, theme::text());
        let blocks: String = std::iter::repeat_n(glyphs::SQUARE, members.len()).collect();
        buf.set_stringn(inner.x + 8, y, format!("{:<7}", blocks), 7, Style::default().fg(theme::WOLF).bg(theme::PANEL_BG));
        buf.set_stringn(inner.x + 15, y, format!("{:>5}", alive_g), 5, Style::default().fg(theme::GOOD).bg(theme::PANEL_BG));
        let m: String = std::iter::repeat_n(glyphs::MUTATION, muts_g).collect();
        buf.set_stringn(inner.x + 24, y, format!("{:<9}", m), 9, Style::default().fg(theme::INFO).bg(theme::PANEL_BG));
        buf.set_stringn(inner.x + 34, y, names.join(", "), 70, theme::dim_text());
        row += 1;
    }

    // Legend + stats at the bottom of the tree panel.
    let alive = l.nodes.iter().filter(|n| n.died_year.is_none()).count();
    let notable = l.nodes.iter().filter(|n| n.notable).count();
    let muts: usize = l.nodes.iter().map(|n| n.mutations.len()).sum();
    row = inner.height - 6;
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
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} in tree: {} alive, {} dead   {} notable   {} mutations recorded   longest branch: Fenrir {} Talon (8 generations)", l.nodes.len(), alive, l.nodes.len() - alive, notable, muts, glyphs::RIGHT), theme::dim_text()),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" mates are not shown; ", theme::dim_text()),
        sp("Sable w#021", theme::text()),
        sp(" (Howl's daughter) is Ashfang's mother.", theme::dim_text()),
    ]));
}

fn details(f: &mut Frame, area: Rect, fx: &fixtures::Fixtures) {
    let l = &fx.lineage;
    let n = &l.nodes[l.focus];
    let hero = &fx.creatures[fx.hero_pred];
    let inner = panel::draw(f, area, "Focused wolf", panel::Kind::Focus);
    let mut row = 0u16;
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} ", glyphs::WOLF.to_ascii_uppercase()), Style::default().fg(theme::WOLF).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(n.name.clone(), theme::title()),
        sp(format!("  {}", n.tag), theme::label()),
        sp(if n.died_year.is_some() { "  dead" } else { "  alive" }, Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
    ]));
    row += 2;
    let parent = parent_of(l, l.focus);
    let grand = parent.and_then(|p| parent_of(l, p));
    let kids = n.children.len();
    let desc = descendants(l, l.focus);
    let living_desc = count_living(l, l.focus);
    let facts: Vec<(String, String)> = vec![
        ("generation".into(), format!("{}", n.generation)),
        ("born".into(), format!("Year {}  (age {} years)", n.born_year, fx.clock.year - n.born_year)),
        ("died".into(), n.died_year.map(|d| format!("Year {}", d)).unwrap_or_else(|| "still living".into())),
        ("father".into(), parent.map(|p| format!("{} {}", l.nodes[p].name, l.nodes[p].tag)).unwrap_or_else(|| "unknown".into())),
        ("mother".into(), hero.parents.1.clone()),
        ("grandfather".into(), grand.map(|p| format!("{} {}", l.nodes[p].name, l.nodes[p].tag)).unwrap_or_else(|| "unknown".into())),
        ("children".into(), format!("{}  ({} living)", kids, n.children.iter().filter(|&&k| l.nodes[k].died_year.is_none()).count())),
        ("descendants".into(), format!("{}  ({} living)", desc, living_desc)),
        ("kills".into(), format!("{}", hero.kills)),
    ];
    for (k, v) in facts {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {:<13}", k), theme::dim_text()),
            sp(v, theme::text()),
        ]));
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
            sp(format!("{} at birth (gen {})", m, n.generation), theme::text()),
        ]));
        row += 1;
    }
    util::line(f, inner, row, Line::from(sp(" inherited: Aggression +0.09 (Greymaw)", theme::dim_text())));
    row += 1;
    util::line(f, inner, row, Line::from(sp("            Size +0.11 (Rime)", theme::dim_text())));
    row += 2;

    panel::section(f, inner, row, "Trait inheritance");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp("            grand parent  self  kids", theme::dim_text()),
    ]));
    row += 1;
    // Derived from the hero genome: walk back the recorded mutation deltas.
    let g = &hero.genome;
    let traits: [(&str, f32, f32, f32, f32); 3] = [
        ("Speed", g.speed() - 0.07, g.speed() - 0.03, g.speed(), g.speed() + 0.02),
        ("Aggression", g.aggression() - 0.14, g.aggression() - 0.05, g.aggression(), g.aggression() + 0.03),
        ("Sense", g.sense() - 0.08, g.sense() - 0.04, g.sense(), g.sense() + 0.05),
    ];
    for (name, a, b, c, d) in traits {
        let y = inner.y + row;
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x, y, format!(" {:<11}", name), 12, theme::text());
        buf.set_stringn(inner.x + 12, y, format!("{:.2}", a), 4, theme::dim_text());
        buf.set_stringn(inner.x + 18, y, format!("{:.2}", b), 4, theme::text());
        buf.set_stringn(inner.x + 24, y, format!("{:.2}", c), 4, theme::selected());
        let dk = Style::default().fg(if d > c { theme::GOOD } else { theme::BAD }).bg(theme::PANEL_BG);
        buf.set_stringn(inner.x + 30, y, format!("{:.2}", d), 4, dk);
        row += 1;
        // Bar row under the numbers: a range bar from grandparent to kids.
        let lo = a.min(b).min(c).min(d);
        let hi = a.max(b).max(c).max(d);
        bars::range(buf, inner.x + 12, y + 1, 26, lo, c, hi, theme::WOLF);
        buf.set_stringn(inner.x + 1, y + 1, format!("{:+.2}", d - a), 5, Style::default().fg(if d > a { theme::GOOD } else { theme::BAD }).bg(theme::PANEL_BG));
        row += 1;
    }
    util::line(f, inner, row, Line::from(sp(" kids = mean of living children", theme::dim_text())));
    row += 2;

    panel::section(f, inner, row, "Children");
    row += 1;
    for &k in &n.children {
        let c = &l.nodes[k];
        let dead = c.died_year.is_some();
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", if dead { glyphs::DEATH } else { glyphs::BIRTH }), Style::default().fg(if dead { theme::DIM } else { theme::GOOD }).bg(theme::PANEL_BG)),
            sp(format!("{:<8}{:<7}", c.name, c.tag), if dead { theme::dim_text() } else { theme::text() }),
            sp(format!("g{} {}", c.generation, years(c)), theme::dim_text()),
            sp(format!("  {}", if c.children.is_empty() { "" } else { "+kids" }), theme::label()),
        ]));
        row += 1;
    }
    row += 1;
    ancestors_and_siblings(f, inner, row, l);
}

fn ancestors_and_siblings(f: &mut Frame, inner: Rect, mut row: u16, l: &Lineage) {
    panel::section(f, inner, row, "Direct line");
    row += 1;
    let mut chain = vec![l.focus];
    let mut cur = l.focus;
    while let Some(p) = parent_of(l, cur) {
        chain.push(p);
        cur = p;
    }
    chain.reverse();
    for (i, &id) in chain.iter().enumerate() {
        let n = &l.nodes[id];
        let st = if id == l.focus { theme::selected() } else if n.died_year.is_some() { theme::dim_text() } else { theme::text() };
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {}{} ", " ".repeat(i), if i == 0 { ' ' } else { glyphs::RIGHT }), theme::border()),
            sp(format!("{} {}", n.name, n.tag), st),
            sp(format!("  g{}", n.generation), theme::dim_text()),
        ]));
        row += 1;
    }
    row += 1;
    panel::section(f, inner, row, "Siblings");
    row += 1;
    if let Some(p) = parent_of(l, l.focus) {
        let sibs: Vec<usize> = l.nodes[p].children.iter().copied().filter(|&k| k != l.focus).collect();
        if sibs.is_empty() {
            util::line(f, inner, row, Line::from(sp(" none", theme::dim_text())));
        }
        for k in sibs {
            let n = &l.nodes[k];
            let dead = n.died_year.is_some();
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" {} ", if dead { glyphs::DEATH } else { glyphs::BIRTH }), Style::default().fg(if dead { theme::DIM } else { theme::GOOD }).bg(theme::PANEL_BG)),
                sp(format!("{:<8}{:<7}", n.name, n.tag), if dead { theme::dim_text() } else { theme::text() }),
                sp(format!("g{} {}", n.generation, years(n)), theme::dim_text()),
            ]));
            row += 1;
        }
    }
}

fn count_living(l: &Lineage, node: usize) -> usize {
    l.nodes[node]
        .children
        .iter()
        .map(|&k| (l.nodes[k].died_year.is_none()) as usize + count_living(l, k))
        .sum()
}
