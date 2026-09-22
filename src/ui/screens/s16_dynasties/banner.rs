//! The selected line's banner: portrait, standing, six bars against the valley
//! mean, the carrier and the pin hint.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::widgets::{Bar, Component, Divider};
use crate::{glyphs, theme};

use super::{age_str, bold, fg, line_name, portraits, put, Pin, Stat, View};

/// Column of the text beside the portrait.
const X: u16 = 18;
const BAR_W: u16 = 24;

fn stat_row(buf: &mut Buffer, inner: Rect, y: u16, stat: Stat, v: &View<'_>) {
    let (Some(d), Some(st)) = (v.dynasty(), v.standing()) else { return };
    let k = stat.index();
    let x = inner.x + X;
    let value = stat.of(&d.totals);
    let best = st.valley_best[k];
    put(buf, x, y, 10, stat.label(), fg(theme::DIM));
    put(buf, x + 11, y, 4, &format!("{:>4}", stat.fmt(value, v.year_days)), bold(theme::TEXT_BRIGHT));
    Bar::new(value / best).color(v.color(d.species)).marker(st.valley_mean[k] / best).render(buf, Rect::new(x + 16, y, BAR_W + 2, 1));
    let (rr, rn) = st.region[k];
    let (vr, vn) = st.valley[k];
    let base = format!("region #{rr}/{rn} · valley #{vr}/{vn} · mean {} · best {}", stat.fmt(st.valley_mean[k], v.year_days), stat.fmt(best, v.year_days));
    let who = v.ranked.dynasties.get(st.valley_best_of[k]).map_or_else(String::new, |b| if b.root == d.root { "this line".to_string() } else { line_name(b) });
    let full = format!("{base} ({who})");
    let avail = usize::from(inner.right().saturating_sub(x + 19 + BAR_W));
    let text = if full.chars().count() <= avail { full } else { base };
    put(buf, x + 19 + BAR_W, y, crate::cast!(avail => u16), &text, fg(theme::DIM));
}

/// Draws the banner from row `y`; returns the next row.
pub(super) fn draw(buf: &mut Buffer, inner: Rect, y: u16, v: &View<'_>) -> u16 {
    let (Some(d), Some(st)) = (v.dynasty(), v.standing()) else { return y };
    let year = super::founded_year(d.founded_day, v.year_days);
    let title = format!("{} · founded Y{year} by {} · {} generations · {} members ever, {} living", line_name(d), d.founder, d.generations, d.members_ever, d.members_living);
    Divider::new(title).render(buf, Rect::new(inner.x, y, inner.width, 1));
    portraits::draw(buf, inner.x + 1, y + 1, portraits::for_species(v.sim.roster(), d.species), v.color(d.species));
    let x = inner.x + X;
    let (rr, rn) = st.region[0];
    let (vr, vn) = st.valley[0];
    let region = d.region.map_or_else(|| "the valley".to_string(), |r| format!("the {}", super::region_name(v.sim, usize::from(r))));
    put(buf, x, y + 1, 10, "standing", fg(theme::DIM));
    let standing = format!("#{rr} of {rn} dynasties in {region} · #{vr} of {vn} {} lines in the valley", v.sim.roster().name(d.species).to_lowercase());
    put(buf, x + 11, y + 1, inner.right().saturating_sub(x + 11), &standing, bold(theme::TEXT));
    for (i, stat) in Stat::ALL.into_iter().enumerate() {
        stat_row(buf, inner, y + 2 + crate::cast!(i => u16), stat, v);
    }
    if let Some(top) = d.members.first() {
        put(buf, x, y + 8, 10, "carried by", fg(theme::DIM));
        let name_w = crate::cast!(top.label.chars().count() => u16);
        put(buf, x + 11, y + 8, name_w, &top.label, bold(v.color(d.species)));
        let rest = format!("· {} of {} kills · {} of {} cells · age {} · gen {}", top.stats.kills, d.totals.kills, top.stats.terr, d.totals.terr, age_str(top.stats.age, v.year_days), top.generation);
        put(buf, x + 12 + name_w, y + 8, inner.right().saturating_sub(x + 12 + name_w), &rest, fg(theme::TEXT));
    }
    let pinned = v.is_pinned(Pin::Dynasty(d.root));
    let hint = if pinned { format!("{} pinned · [p] unpins", glyphs::DIAMOND) } else { "[p] pins this line to the Watch strip".to_string() };
    put(buf, x, y + 9, 42, &hint, fg(if pinned { theme::ACCENT } else { theme::DIM }));
    put(buf, x + 43, y + 9, inner.right().saturating_sub(x + 43), &format!("{} valley mean of the species' lines", glyphs::V_LINE), fg(theme::DIM));
    y + 10
}
