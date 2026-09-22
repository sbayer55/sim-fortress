//! The region line with the species chips, and the four-line dynasty table.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::sim::Kind;
use crate::ui::style::SpeciesStyle;
use crate::widgets::Constraint::Fixed;
use crate::widgets::{Column, Component, FilterStrip, Table, TableCell, TableRow};
use crate::{glyphs, theme};

use super::{bold, fg, line_name, put, region_short, Focus, Pin, Stat, View, ALL};

/// Where the species chips start on the region line.
const CHIPS_X: u16 = 76;

/// `6 foxes, 17 wolves, 4 lynx`: living predators in the region (or the valley).
fn living_counts(v: &View<'_>) -> String {
    let roster = v.sim.roster();
    let mut counts = vec![0u32; roster.len()];
    for c in v.sim.creatures.living() {
        if roster.kind(c.species) != Kind::Predator {
            continue;
        }
        if v.region != ALL && v.sim.world.region_index(c.x, c.y).min(7) != v.region {
            continue;
        }
        if let Some(n) = counts.get_mut(c.species.index()) {
            *n += 1;
        }
    }
    let parts: Vec<String> = roster.predator_ids().filter_map(|id| counts.get(id.index()).filter(|&&n| n > 0).map(|n| format!("{n} {}", roster.plural(id).to_lowercase()))).collect();
    parts.join(", ")
}

fn region_line(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>) {
    let name = v.region_name();
    let x = inner.x + 1;
    put(buf, x, y, 1, &glyphs::REWIND.to_string(), bold(theme::KEY));
    put(buf, x + 2, y, crate::cast!(name.chars().count() => u16), &name, bold(theme::TITLE));
    let after = x + 3 + crate::cast!(name.chars().count() => u16);
    put(buf, after, y, 1, &glyphs::PLAY.to_string(), bold(theme::KEY));
    let lines = v.rows.len();
    let counts = living_counts(v);
    let info = if v.region == ALL {
        format!("the whole valley · {} dynasties in {ALL} regions", v.ranked.dynasties.len())
    } else {
        format!("region {}/{ALL} · {lines} dynasties · {counts} living", v.region + 1)
    };
    put(buf, after + 3, y, (inner.x + CHIPS_X).saturating_sub(after + 4), &info, fg(theme::DIM));
    let roster = v.sim.roster();
    let ids: Vec<_> = roster.predator_ids().collect();
    let names: Vec<String> = ids.iter().map(|&id| roster.name(id).to_lowercase()).collect();
    let mut chips: Vec<(char, &str)> = vec![(' ', "all")];
    chips.extend(ids.iter().zip(&names).map(|(&id, n)| (roster.adult_glyph(id), n.as_str())));
    let mut active = vec![v.species.is_none()];
    active.extend(ids.iter().map(|&id| v.species == Some(id)));
    let mut colors: Vec<Color> = vec![theme::TEXT];
    colors.extend(ids.iter().map(|&id| v.color(id)));
    FilterStrip::new(&chips).active(&active).colors(&colors).hint("[s]").render(buf, Rect::new(inner.x + CHIPS_X, y, inner.width.saturating_sub(CHIPS_X), 1));
}

const fn columns() -> [Column; 15] {
    [
        Column::titled("#", Fixed(4)).right(),
        Column::titled("dynasty", Fixed(15)),
        Column::titled("sp", Fixed(3)),
        Column::titled("since", Fixed(6)).right(),
        Column::titled("gens", Fixed(6)).right(),
        Column::titled("living", Fixed(8)).right(),
        Column::titled("kills", Fixed(8)).right(),
        Column::titled("terr", Fixed(7)).right(),
        Column::titled("young", Fixed(7)).right(),
        Column::titled("surv", Fixed(6)).right(),
        Column::titled("age", Fixed(6)).right(),
        Column::titled("muts", Fixed(6)).right(),
        Column::new(Fixed(1)),
        Column::titled("carried by", Fixed(15)),
        Column::titled("region", Fixed(10)),
    ]
}

fn row<'a>(v: &View<'_>, rank: usize, i: usize) -> Option<TableRow<'a>> {
    let d = v.ranked.dynasties.get(i)?;
    let since = format!("Y{}", super::founded_year(d.founded_day, v.year_days));
    let region = d.region.map_or_else(String::new, |r| region_short(&super::region_name(v.sim, usize::from(r))));
    let carrier = d.members.first().map_or_else(String::new, |m| m.label.clone());
    let t = &d.totals;
    Some(TableRow::new([
        TableCell::styled(format!("{rank} "), if rank == 1 { theme::ACCENT } else { theme::DIM }),
        TableCell::text(line_name(d)),
        TableCell::Glyph(v.sim.roster().adult_glyph(d.species), v.color(d.species)),
        TableCell::text(since),
        TableCell::text(d.generations.to_string()),
        TableCell::styled(d.members_living.to_string(), theme::GOOD),
        TableCell::text(t.kills.to_string()),
        TableCell::text(t.terr.to_string()),
        TableCell::text(t.young.to_string()),
        TableCell::text(t.surv.to_string()),
        TableCell::text(Stat::Age.fmt(Stat::Age.of(t), v.year_days)),
        TableCell::text(t.muts.to_string()),
        TableCell::text(""),
        TableCell::styled(carrier, v.color(d.species)),
        TableCell::dim(region),
    ]))
}

/// Draws the region line and the table from `inner.y`; returns the next row.
pub(super) fn draw(buf: &mut Buffer, inner: Rect, v: &View<'_>) -> u16 {
    region_line(buf, inner, inner.y, v);
    let rows: Vec<TableRow<'_>> = v.rows.iter().enumerate().filter_map(|(r, &i)| row(v, r + 1, i)).collect();
    let selected = v.sel.and_then(|s| v.rows.iter().position(|&i| i == s));
    let table_sel = if v.focus == Focus::Dynasties { selected } else { None };
    let area = Rect::new(inner.x + 1, inner.y + 1, inner.width - 1, 5);
    Table::new(&columns(), &rows).selected(table_sel).render(buf, area);
    for (r, &i) in v.rows.iter().enumerate() {
        let y = area.y + 1 + crate::cast!(r => u16);
        let bg = if Some(r) == table_sel { theme::SELECT_BG } else { theme::PANEL_BG };
        if let Some(d) = v.ranked.dynasties.get(i) {
            if v.is_pinned(Pin::Dynasty(d.root)) {
                put(buf, inner.x, y, 1, &glyphs::DIAMOND.to_string(), bold(theme::ACCENT).bg(bg));
            }
        }
        if Some(r) == selected && v.focus != Focus::Dynasties {
            put(buf, area.x, y, 1, &glyphs::DOT.to_string(), fg(theme::DIM));
        }
    }
    if rows.is_empty() {
        put(buf, inner.x + 4, area.y + 1, inner.width - 4, "no dynasties match the filter", fg(theme::DIM));
    }
    area.y + 5
}
