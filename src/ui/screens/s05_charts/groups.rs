//! S05e — the group-size distribution (C8 follow-up): how large the herds and
//! packs the cohesion rule holds together actually get, per species.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::Frame;
use crate::sim::stats::GROUP_HIST;
use crate::sim::{Kind, Sim, SpeciesId};
use crate::ui::screens::common::sp;
use crate::ui::style::SpeciesStyle;
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};

/// Rows per species block: the summary line, a four-row histogram and the axis.
const BLOCK_H: u16 = 6;
/// Columns per group-size bucket (16 buckets, so 64 columns of plot).
const COL_W: u16 = 4;
/// Left gutter for the bucket-count axis.
const GUTTER: u16 = 6;

pub(super) fn group_chart(f: &mut Frame<'_>, area: Rect, sim: &Sim) {
    let inner = panel::draw_with_hint(f, area, "Group sizes — herds and packs", "living groups by size, each row scaled to its own tallest bar", panel::Kind::Outer);
    let living: Vec<SpeciesId> = sim.roster().ids().filter(|id| sim.species[id.index()].count > 0).collect();
    if living.is_empty() {
        util::line(f, inner, 0, Line::from(sp(" no living creatures", theme::dim_text())));
        return;
    }
    for (k, id) in living.iter().enumerate() {
        let y = inner.y + crate::cast!(k => u16) * BLOCK_H;
        if y + BLOCK_H > inner.bottom() {
            break;
        }
        species_block(f, Rect::new(inner.x, y, inner.width, BLOCK_H), sim, *id);
    }
}

/// `(singular, plural)` for a species' groups: prey herd, predators pack.
const fn group_word(kind: Kind) -> (&'static str, &'static str) {
    match kind {
        Kind::Prey => ("herd", "herds"),
        Kind::Predator => ("pack", "packs"),
    }
}

/// One species: the numbers line, its size histogram and the size axis.
fn species_block(f: &mut Frame<'_>, area: Rect, sim: &Sim, id: SpeciesId) {
    let i = id.index();
    let g = &sim.group_stats;
    let (word, words) = group_word(sim.roster().kind(id));
    let noun = if g.groups[i] == 1 { word } else { words };
    let mut spans = vec![
        sp(format!(" {} ", sim.roster().adult_glyph(id)), Style::default().fg(sim.roster().color(id)).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(format!("{:<8}", sim.roster().plural(id)), theme::text()),
    ];
    if g.groups[i] == 0 {
        spans.push(sp(format!("no {words} forming — all {} alone", sim.species[i].count), theme::dim_text()));
    } else {
        spans.push(sp("mean ", theme::dim_text()));
        spans.push(sp(format!("{:.1}", g.mean[i]), Style::default().fg(sim.roster().color(id)).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)));
        spans.push(sp(format!("  max {}   {} {noun}   {} of {} grouped ({:.0}%)   {} alone", g.max[i], g.groups[i], g.members[i], sim.species[i].count, g.grouped_share(i, sim.species[i].count) * 100.0, g.solo(i)), theme::text()));
    }
    util::line(f, area, 0, Line::from(spans));

    let hist = Rect::new(area.x + GUTTER, area.y + 1, COL_W * crate::cast!(GROUP_HIST => u16), area.height - 2);
    let values: Vec<u16> = g.hist[i].iter().map(|&v| crate::cast!(v.min(u32::from(u16::MAX)) => u16)).collect();
    bars::histogram(f.buffer_mut(), hist, &values, sim.roster().color(id), COL_W);
    let peak = values.iter().copied().max().unwrap_or(0);
    f.buffer_mut().set_stringn(area.x, area.y + 1, format!("{peak:>5}"), 5, theme::dim_text());

    // The size axis: a `─` line with a `┼` tick and label at 1, 4, 8, 12 and 16+.
    let axis_y = area.y + area.height - 1;
    let line: String = std::iter::repeat_n(glyphs::H_LINE, crate::cast!(hist.width => usize)).collect();
    let buf = f.buffer_mut();
    buf.set_stringn(hist.x, axis_y, &line, crate::cast!(hist.width => usize), theme::border());
    for (bucket, label) in [(0usize, "1"), (3, "4"), (7, "8"), (11, "12"), (15, "16+")] {
        let x = hist.x + crate::cast!(bucket => u16) * COL_W;
        buf.set_stringn(x, axis_y, glyphs::CROSS.to_string(), 1, theme::border());
        buf.set_stringn(x + 1, axis_y, label, label.len(), theme::dim_text());
    }
}

pub(super) fn group_sidebar(f: &mut Frame<'_>, area: Rect, sim: &Sim) {
    let inner = panel::draw(f, area, "Group sizes", panel::Kind::Outer);
    let mut row = 0u16;
    for id in sim.roster().ids() {
        let i = id.index();
        let g = &sim.group_stats;
        let absent = sim.species[i].count == 0;
        let style = if absent { theme::dim_text() } else { theme::text() };
        let mut spans = vec![
            sp(format!(" {} ", sim.roster().adult_glyph(id)), Style::default().fg(if absent { theme::DIM } else { sim.roster().color(id) }).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{:<8}", sim.roster().display_name(id)), style),
        ];
        if absent {
            spans.push(sp("—", theme::dim_text()));
        } else if g.groups[i] == 0 {
            spans.push(sp(format!("all {} alone", g.solo(i)), theme::dim_text()));
        } else {
            let (word, words) = group_word(sim.roster().kind(id));
            let noun = if g.groups[i] == 1 { word } else { words };
            spans.push(sp(format!("mean {:.1}  max {:<3} {} {}", g.mean[i], g.max[i], g.groups[i], noun), style));
        }
        util::line(f, inner, row, Line::from(spans));
        row += 1;
        if !absent && g.groups[i] > 0 {
            util::line(f, inner, row, Line::from(sp(
                format!("          {} of {} grouped ({:.0}%)", g.members[i], sim.species[i].count, g.grouped_share(i, sim.species[i].count) * 100.0),
                theme::dim_text(),
            )));
            row += 1;
        }
        row += 1;
    }
    row += 1;
    panel::section(f, inner, row, "How to read");
    row += 1;
    for line in [
        "a group is same-species kin inside",
        "sense range, held while it stays under",
        "1.5 x the sociality's preferred size;",
        "below social.cohesion_min an animal",
        "never herds. Prey form herds, predators",
        "packs (C8). Mean and max count the",
        "groups of two or more, as of midnight.",
    ] {
        util::line(f, inner, row, Line::from(sp(format!(" {line}"), theme::dim_text())));
        row += 1;
    }
    row += 1;
    panel::section(f, inner, row, "Legend");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" ░▒▓█", theme::text()),
        sp("  groups per size, in the species colour", theme::text()),
    ]));
    row += 2;
    panel::section(f, inner, row, "Keys");
    row += 1;
    util::line(f, inner, row, Line::from(sp(" [g] next  [1-5] pick  [+/-] zoom", theme::dim_text())));
}
