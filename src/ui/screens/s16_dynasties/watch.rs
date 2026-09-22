//! The Watch strip: up to four pinned lines and animals, reachable with `1`–`4`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::widgets::Panel;
use crate::{glyphs, theme};

use super::rank::top_pct;
use super::{bold, fg, line_name, put, region_short, Pin, Stat, View, MAX_PINS};

/// Cut `s` to `w` cells at a ` · ` boundary when it does not fit, else at a word.
fn cut_words(s: &str, w: usize) -> String {
    if s.chars().count() <= w {
        return s.to_string();
    }
    let head: String = s.chars().take(w).collect();
    if let Some(i) = head.rfind(" · ") {
        return head.get(..i).unwrap_or(&head).to_string();
    }
    head.rsplit_once(' ').map_or_else(|| head.clone(), |(a, _)| a.to_string())
}

/// The strip's text for a pin, and whether its target is still listed.
fn label(v: &View<'_>, pin: Pin) -> (String, bool) {
    match pin {
        Pin::Dynasty(root) => {
            let Some((i, d)) = v.ranked.dynasties.iter().enumerate().find(|(_, d)| d.root == root) else {
                return (format!("line {} · gone", root.0), false);
            };
            let region = d.region.map_or_else(|| "nowhere".to_string(), |r| region_short(&super::region_name(v.sim, usize::from(r))));
            let rank = v.ranked.standing.get(i).map_or(0, |s| s.region[0].0);
            (format!("{} · {} kills · {region} · #{rank} in region", line_name(d), d.totals.kills), true)
        }
        Pin::Member(id) => {
            let Some((d, m)) = v.ranked.dynasties.iter().find_map(|d| d.members.iter().find(|m| m.id == id).map(|m| (d, m))) else {
                return (format!("animal #{} · gone", id.0), false);
            };
            let region = region_short(&super::region_name(v.sim, usize::from(m.region)));
            let (rank, n) = v.ranked.living.get(&d.species).map_or((1, 1), |r| r.rank(Stat::Kills, Stat::Kills.of(&m.stats)));
            let plural = v.sim.roster().plural(d.species).to_lowercase();
            (format!("{} · {} kills · {region} · top {}% of {plural}", m.label, m.stats.kills, top_pct(rank, n)), true)
        }
    }
}

/// Whether the pin is the current selection.
fn is_current(v: &View<'_>, pin: Pin) -> bool {
    match pin {
        Pin::Dynasty(root) => v.dynasty().is_some_and(|d| d.root == root),
        Pin::Member(id) => v.member().is_some_and(|m| m.id == id),
    }
}

pub(super) fn draw(buf: &mut Buffer, area: Rect, v: &View<'_>) {
    let inner = Panel::new("Watch").info(format!("{} pinned · [p] pin the selection · [1-4] jump", v.pins.len())).render(buf, area);
    if inner.height == 0 {
        return;
    }
    if v.pins.is_empty() {
        put(buf, inner.x + 1, inner.y, inner.width - 1, "nothing pinned yet · [p] pins the selected dynasty or animal, it then stays selected when the ranking shifts", fg(theme::DIM));
        return;
    }
    let n = v.pins.len().min(MAX_PINS);
    let slot = inner.width.div_euclid(crate::cast!(n => u16));
    for (i, &pin) in v.pins.iter().take(n).enumerate() {
        let sx = inner.x + crate::cast!(i => u16) * slot;
        let (text, listed) = label(v, pin);
        let current = listed && is_current(v, pin);
        let bg = if current { theme::SELECT_BG } else { theme::PANEL_BG };
        let color = match (listed, pin) {
            (false, _) => theme::DIM,
            (true, Pin::Dynasty(root)) => v.ranked.dynasties.iter().find(|d| d.root == root).map_or(theme::TEXT, |d| v.color(d.species)),
            (true, Pin::Member(id)) => v.ranked.dynasties.iter().find(|d| d.members.iter().any(|m| m.id == id)).map_or(theme::TEXT, |d| v.color(d.species)),
        };
        let text_w = usize::from(slot.saturating_sub(7));
        crate::widgets::util::fill(buf, Rect::new(sx, inner.y, slot.saturating_sub(2), 1), fg(theme::TEXT).bg(bg));
        put(buf, sx, inner.y, 1, &(i + 1).to_string(), bold(theme::KEY).bg(bg));
        put(buf, sx + 2, inner.y, 1, &glyphs::DIAMOND.to_string(), bold(theme::ACCENT).bg(bg));
        let style = if current { bold(theme::TEXT_BRIGHT).bg(bg) } else { fg(color) };
        put(buf, sx + 4, inner.y, slot.saturating_sub(7), &cut_words(&text, text_w), style);
        if i + 1 < n {
            put(buf, sx + slot - 2, inner.y, 1, &glyphs::V_LINE.to_string(), theme::border());
        }
    }
}
