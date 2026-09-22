//! The living members of the selected line, the top few by kills.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::widgets::Constraint::Fixed;
use crate::widgets::{Bar, Column, Component, Divider, Table, TableCell, TableRow, Text};
use crate::{glyphs, theme};

use super::rank::top_pct;
use super::{bold, fg, put, Focus, Pin, Stat, View};

const BAR_W: u16 = 22;

const fn columns() -> [Column; 13] {
    [
        Column::new(Fixed(1)),
        Column::titled("name", Fixed(9)),
        Column::titled("tag", Fixed(7)),
        Column::titled("sex", Fixed(4)),
        Column::titled("gen", Fixed(5)).right(),
        Column::titled("kills", Fixed(7)).right(),
        Column::titled("terr", Fixed(6)).right(),
        Column::titled("young", Fixed(7)).right(),
        Column::titled("surv", Fixed(6)).right(),
        Column::titled("age", Fixed(6)).right(),
        Column::titled("muts", Fixed(6)).right(),
        Column::new(Fixed(1)),
        Column::titled("kills vs the living", Fixed(BAR_W)),
    ]
}

/// The divider, in the focus colour when the member list has the cursor.
fn divider(buf: &mut Buffer, inner: Rect, y: u16, text: &str, focused: bool) {
    Divider::new(text).render(buf, Rect::new(inner.x, y, inner.width, 1));
    if focused {
        let rule: String = std::iter::repeat_n(glyphs::H_LINE, usize::from(inner.width)).collect();
        put(buf, inner.x, y, inner.width, &rule, theme::border_focus());
        put(buf, inner.x + 1, y, inner.width - 1, &format!(" {text} "), bold(theme::BORDER_FOCUS));
    }
}

/// Draws the members from row `y`, `rows` of them.
pub(super) fn draw(buf: &mut Buffer, inner: Rect, y: u16, rows: u16, v: &View<'_>) {
    let Some(d) = v.dynasty() else { return };
    let shown = d.members.len().min(usize::from(rows));
    let focused = v.focus == Focus::Members;
    divider(buf, inner, y, &format!("Living members · {}, the top {shown} by kills", d.members_living), focused);
    let ranks = v.ranked.living.get(&d.species);
    let best = ranks.map_or(1.0, |r| r.best_of(Stat::Kills));
    let mean = ranks.map_or(0.0, |r| r.mean_of(Stat::Kills));
    let color = v.color(d.species);
    let table_rows: Vec<TableRow<'_>> = d
        .members
        .iter()
        .take(shown)
        .map(|m| {
            let (name, tag) = m.label.split_once(' ').unwrap_or((m.label.as_str(), ""));
            let kills = Stat::Kills.of(&m.stats);
            let (rank, n) = ranks.map_or((1, 1), |r| r.rank(Stat::Kills, kills));
            TableRow::new([
                TableCell::text(""),
                TableCell::text(name.to_string()),
                TableCell::dim(tag.to_string()),
                TableCell::dim(super::sex_glyph(m.sex).to_string()),
                TableCell::dim(format!("g{}", m.generation)),
                TableCell::text(m.stats.kills.to_string()),
                TableCell::text(m.stats.terr.to_string()),
                TableCell::text(m.stats.young.to_string()),
                TableCell::text(m.stats.surv.to_string()),
                TableCell::text(Stat::Age.fmt(Stat::Age.of(&m.stats), v.year_days)),
                TableCell::text(m.stats.muts.to_string()),
                TableCell::text(""),
                TableCell::widget(Bar::new(kills / best).color(color).marker(mean / best)),
            ])
            .tail(Text::new(format!("top {}% · #{rank} of {n}", top_pct(rank, n))).style(theme::dim_text()))
        })
        .collect();
    let table_sel = if focused { Some(v.member_ix.min(shown.saturating_sub(1))) } else { None };
    let area = Rect::new(inner.x + 1, y + 1, inner.width - 1, 1 + rows);
    Table::new(&columns(), &table_rows).selected(table_sel).render(buf, area);
    for (r, m) in d.members.iter().take(shown).enumerate() {
        let yy = area.y + 1 + crate::cast!(r => u16);
        let bg = if Some(r) == table_sel { theme::SELECT_BG } else { theme::PANEL_BG };
        if v.is_pinned(Pin::Member(m.id)) {
            put(buf, inner.x, yy, 1, &glyphs::DIAMOND.to_string(), bold(theme::ACCENT).bg(bg));
        }
        if !focused && r == v.member_ix.min(shown.saturating_sub(1)) {
            put(buf, area.x, yy, 1, &glyphs::DOT.to_string(), fg(theme::DIM));
        }
    }
}
