//! The Top member sidebar: the selected member (else the carrier), its ranks
//! and percentiles among the living of its species, its kills by prey, its
//! territory, survival and genetics.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::sim::lineage::dynasties::DynastyMember;
use crate::sim::species::TRAIT_NAMES;
use crate::sim::{Kind, Mutation};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{Bar, Component, Divider, Histogram, Panel};
use crate::{glyphs, theme};

use super::rank::top_pct;
use super::{age_str, bold, fg, line_name, put, signed, Pin, Stat, View};

fn section(buf: &mut Buffer, inner: Rect, y: u16, title: &str) -> u16 {
    Divider::new(title).render(buf, Rect::new(inner.x, y, inner.width, 1));
    y + 1
}

fn head(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>, m: &DynastyMember) -> u16 {
    let Some(d) = v.dynasty() else { return y };
    let sp = d.species;
    let x = inner.x + 1;
    put(buf, x, y, 1, &v.sim.roster().adult_glyph(sp).to_string(), bold(v.color(sp)));
    let (name, tag) = m.label.split_once(' ').unwrap_or((m.label.as_str(), ""));
    put(buf, x + 2, y, crate::cast!(name.chars().count() => u16), name, bold(theme::TITLE));
    put(buf, x + 3 + crate::cast!(name.chars().count() => u16), y, 8, tag, fg(theme::DIM));
    let sex = format!("{} {}", super::sex_glyph(m.sex), v.sim.roster().name(sp).to_lowercase());
    let sw = crate::cast!(sex.chars().count() => u16);
    put(buf, inner.right().saturating_sub(sw + 1), y, sw, &sex, fg(v.color(sp)));
    let carries = d.members.first().is_some_and(|c| c.id == m.id);
    let rank = d.members.iter().position(|o| o.id == m.id).map_or(1, |p| p + 1);
    let line = if carries { format!("carries the {}", line_name(d)) } else { format!("of the {}, #{rank} by kills", line_name(d)) };
    put(buf, x, y + 1, inner.width - 1, &line, fg(if carries { theme::ACCENT } else { theme::TEXT }));
    put(buf, x, y + 2, inner.width - 1, &format!("{} living · age {} · gen {}", glyphs::BIRTH, age_str(m.stats.age, v.year_days), m.generation), fg(theme::GOOD));
    y + 3
}

fn ranks(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>, m: &DynastyMember) -> u16 {
    let Some(d) = v.dynasty() else { return y };
    super::portraits::draw(buf, inner.x + 2, y, super::portraits::for_species(v.sim.roster(), d.species), v.color(d.species));
    let x = inner.x + 19;
    let living = v.ranked.living.get(&d.species);
    for (i, stat) in Stat::ALL.into_iter().enumerate() {
        let yy = y + crate::cast!(i => u16);
        let value = stat.of(&m.stats);
        let (rank, n) = living.map_or((1, 1), |r| r.rank(stat, value));
        put(buf, x, yy, 10, stat.label(), fg(theme::DIM));
        put(buf, x + 10, yy, 4, &format!("{:>4}", stat.fmt(value, v.year_days)), bold(theme::TEXT_BRIGHT));
        put(buf, x + 15, yy, 7, &format!("#{rank}/{n}"), fg(theme::DIM));
    }
    let pinned = v.is_pinned(Pin::Member(m.id));
    put(buf, x, y + 6, 12, if pinned { "♦ pinned" } else { "[p] pin" }, fg(if pinned { theme::ACCENT } else { theme::DIM }));
    y + 8
}

fn against(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>, m: &DynastyMember) -> u16 {
    let Some(d) = v.dynasty() else { return y };
    let plural = v.sim.roster().plural(d.species).to_lowercase();
    let mut y = section(buf, inner, y, &format!("Against living {plural}"));
    let living = v.ranked.living.get(&d.species);
    let x = inner.x + 1;
    for stat in Stat::ALL {
        let value = stat.of(&m.stats);
        let best = living.map_or(1.0, |r| r.best_of(stat));
        let mean = living.map_or(0.0, |r| r.mean_of(stat));
        let (rank, n) = living.map_or((1, 1), |r| r.rank(stat, value));
        put(buf, x, y, 9, stat.label(), fg(theme::TEXT));
        Bar::new(value / best).color(v.color(d.species)).marker(mean / best).render(buf, Rect::new(x + 9, y, 20, 1));
        let pct = top_pct(rank, n);
        put(buf, x + 30, y, 9, &format!("{:>9}", format!("top {pct}%")), fg(if pct <= 10 { theme::GOOD } else { theme::TEXT }));
        y += 1;
    }
    put(buf, x, y, inner.width - 1, &format!("{} species mean · bar = share of the best", glyphs::V_LINE), fg(theme::DIM));
    y + 2
}

fn kills_by_prey(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>, m: &DynastyMember) -> u16 {
    let Some(d) = v.dynasty() else { return y };
    let y = section(buf, inner, y, "Kills by prey");
    let roster = v.sim.roster();
    let prey: Vec<_> = roster.ids().filter(|&id| roster.kind(id) == Kind::Prey).collect();
    let buckets: Vec<u16> = prey.iter().map(|id| crate::cast!(m.kills_by_species.get(id.index()).copied().unwrap_or(0).min(u32::from(u16::MAX)) => u16)).collect();
    let x = inner.x + 1;
    Histogram::new(&buckets).rows(2).col_w(4).color(v.color(d.species)).render(buf, Rect::new(x, y, inner.width - 1, 2));
    for (i, id) in prey.iter().enumerate() {
        let cx = x + crate::cast!(i => u16) * 4;
        if buckets.get(i).copied().unwrap_or(0) == 0 {
            put(buf, cx, y + 1, 2, "··", fg(theme::DIM));
        }
        put(buf, cx, y + 2, 3, &roster.adult_glyph(*id).to_string(), bold(roster_color(v, *id)));
        put(buf, cx, y + 3, 3, &buckets.get(i).copied().unwrap_or(0).to_string(), fg(theme::TEXT));
    }
    y + 5
}

fn roster_color(v: &View<'_>, id: crate::sim::SpeciesId) -> ratatui::style::Color {
    v.color(id)
}

fn territory(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>, m: &DynastyMember) -> u16 {
    let Some(d) = v.dynasty() else { return y };
    let y = section(buf, inner, y, "Territory");
    let share = if d.totals.terr == 0 { 0 } else { (m.stats.terr * 100).div_euclid(d.totals.terr) };
    put(buf, inner.x + 1, y, inner.width - 1, &format!("{} cells · {share}% of line · won {} lost {}", m.stats.terr, m.contests.0, m.contests.1), fg(theme::TEXT));
    y + 2
}

fn survival(buf: &mut Buffer, inner: Rect, y: u16, m: &DynastyMember) -> u16 {
    let mut y = section(buf, inner, y, "Survival");
    let s = m.survival;
    let lines: [(char, &str, u32, ratatui::style::Color); 5] = [
        (glyphs::IMMUNE, "gained immunity", u32::from(s.infections), theme::SICK),
        (glyphs::CUE, "escaped a predator", s.escapes, theme::TEXT),
        (glyphs::DROUGHT, "survived a drought", u32::from(s.droughts), theme::WARN),
        (glyphs::SNOW, "survived a hard winter", u32::from(s.winters), theme::INFO),
        (glyphs::PLAY, "won a contest", u32::from(s.contests_won), theme::ACCENT),
    ];
    let mut shown = 0;
    for (g, text, n, color) in lines {
        if n == 0 || shown == 3 {
            continue;
        }
        put(buf, inner.x + 1, y, 1, &g.to_string(), bold(color));
        put(buf, inner.x + 3, y, inner.width - 3, &format!("{text} ×{n}"), fg(theme::TEXT));
        y += 1;
        shown += 1;
    }
    if shown == 0 {
        put(buf, inner.x + 1, y, inner.width - 1, "nothing survived yet, nothing lost", fg(theme::DIM));
        y += 1;
    }
    y + 1
}

fn mutation_text(mu: Mutation) -> String {
    let name = TRAIT_NAMES.get(mu.trait_idx).copied().unwrap_or("trait");
    format!("{name} {}", signed(mu.delta))
}

fn genetics(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>, m: &DynastyMember, limit: u16) -> u16 {
    let mut y = section(buf, inner, y, "Genetics");
    if m.mutations.is_empty() {
        put(buf, inner.x + 1, y, inner.width - 1, "no mutation at birth", fg(theme::DIM));
        y += 1;
    }
    for &mu in m.mutations.iter().take(2) {
        put(buf, inner.x + 1, y, 1, &glyphs::MUTATION.to_string(), bold(theme::INFO));
        put(buf, inner.x + 3, y, inner.width - 3, &format!("{} at birth (g{})", mutation_text(mu), mu.generation), fg(theme::TEXT));
        y += 1;
    }
    let inherited: Vec<String> = v.sim.lineage.ancestors(m.id, 3).iter().filter_map(|id| v.sim.lineage.get(*id)).flat_map(|n| n.mutations.iter().copied().map(mutation_text)).take(3).collect();
    if !inherited.is_empty() && y < limit {
        put(buf, inner.x + 1, y, inner.width - 1, &format!("inherited: {}", inherited.join(", ")), fg(theme::DIM));
        y += 1;
    }
    y
}

pub(super) fn draw(buf: &mut Buffer, area: Rect, v: &View<'_>) {
    let inner = Panel::new("Top member").render(buf, area);
    if inner.width < 30 || inner.height < 30 {
        return;
    }
    let Some(m) = v.member() else {
        put(buf, inner.x + 1, inner.y, inner.width - 1, "nothing selected", fg(theme::DIM));
        return;
    };
    let footer_y = inner.bottom() - 1;
    let mut y = head(buf, inner, inner.y, v, m);
    y = ranks(buf, inner, y + 1, v, m);
    y = against(buf, inner, y, v, m);
    y = kills_by_prey(buf, inner, y, v, m);
    y = territory(buf, inner, y, v, m);
    y = survival(buf, inner, y, m);
    genetics(buf, inner, y, v, m, footer_y);
    put(buf, inner.x + 1, footer_y, inner.width - 1, "[f] follow on map  [l] lineage", fg(theme::DIM));
}
