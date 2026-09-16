//! S05b — the prey/predator phase plot.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::Frame;
use crate::sim::{Kind, Sim};
use crate::ui::screens::common::sp;
use crate::widgets::{panel, util};
use crate::{glyphs, theme};
use super::{Window, round_up};

pub(super) fn phase_chart(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw_with_hint(f, area, "Phase plot — predators against prey", &format!("one point per day, {} days", w.len()), panel::Kind::Outer);
    if w.len() == 0 || inner.height < 12 {
        return;
    }
    let _ = sim;
    let note_h = 3u16;
    let plot = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1 - note_h);
    let label_w = 6u16;
    let px = plot.x + label_w;
    let pw = plot.width - label_w - 1;
    let py = plot.y;
    let ph = plot.height - 1; // last row carries the x labels
    if pw < 4 || ph < 4 {
        return;
    }
    let max_prey = round_up(w.prey.iter().copied().fold(0.0f32, f32::max), 50.0);
    let max_pred = round_up(w.pred.iter().copied().fold(0.0f32, f32::max), 20.0).max(1.0);
    let buf = f.buffer_mut();

    phase_scatter(buf, w, px, pw, py, ph, max_prey, max_pred);
    phase_axes(buf, inner, px, pw, py, ph, max_prey, max_pred);

    // Note rows.
    let note_y = inner.y + inner.height - 3;
    panel::section(f, inner, note_y - inner.y, "Reading the orbit");
    util::line(f, inner, note_y - inner.y + 1, Line::from(sp(" the orbit runs counter-clockwise: prey boom, predators follow, prey crash, predators starve", theme::dim_text())));
    util::line(f, inner, note_y - inner.y + 2, Line::from(vec![
        sp(format!("{}{} ", glyphs::HALF_UPPER, glyphs::HALF_LOWER), Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG)),
        sp("last 40 days   ", theme::text()),
        sp(format!("{} ", glyphs::FULL_BLOCK), Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG)),
        sp("today   ", theme::text()),
        sp(format!("{} ", glyphs::BULLET), Style::default().fg(theme::DIM).bg(theme::PANEL_BG)),
        sp("older days", theme::text()),
    ]));
}

/// The phase-plot scatter: older days dim, the last 40 days accent, today bright,
/// plus the dim equilibrium crosshair.
#[allow(clippy::too_many_arguments)]
fn phase_scatter(buf: &mut Buffer, w: &Window<'_>, px: u16, pw: u16, py: u16, ph: u16, max_prey: f32, max_pred: f32) {
    let n = w.len();
    let recent = n.saturating_sub(40);
    let mean_prey = w.prey.iter().sum::<f32>() / crate::cast!(w.len() => f32);
    let mean_pred = w.pred.iter().sum::<f32>() / crate::cast!(w.len() => f32);
    let x_of = |v: f32| px + crate::cast!(((v / max_prey) * f32::from(pw - 1)).round() => u16);
    let y_of = |v: f32| py + ph - 1 - crate::cast!(((v / max_pred) * f32::from(ph - 1)).round() => u16);

    // Scatter: older days as dim dots, the last 40 days as an accent half-block.
    for i in 0..n {
        let x = x_of(w.prey[i]);
        let y = y_of(w.pred[i]);
        if i < recent {
            if let Some(c) = buf.cell_mut((x, y)) {
                c.set_char(glyphs::BULLET);
                c.set_style(Style::default().fg(theme::DIM).bg(theme::PANEL_BG));
            }
        } else if let Some(c) = buf.cell_mut((x, y)) {
            c.set_char(glyphs::HALF_LOWER);
            c.set_style(Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
        }
    }
    // Today marker.
    if let Some(&lp) = w.prey.last() {
        if let Some(&lq) = w.pred.last() {
            let x = x_of(lp);
            let y = y_of(lq);
            if let Some(c) = buf.cell_mut((x, y)) {
                c.set_char(glyphs::FULL_BLOCK);
                c.set_style(Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
            }
        }
    }
    // Equilibrium crosshair (very dim dotted lines through the means).
    let eqx = x_of(mean_prey);
    let eqy = y_of(mean_pred);
    for r in 0..ph {
        let y = py + r;
        if let Some(c) = buf.cell_mut((eqx, y)) {
            if c.symbol() == " " {
                c.set_char(glyphs::BULLET);
                c.set_style(Style::default().fg(theme::dim(theme::TEXT, 0.5)).bg(theme::PANEL_BG));
            }
        }
    }
    for x in px..px + pw {
        if let Some(c) = buf.cell_mut((x, eqy)) {
            if c.symbol() == " " {
                c.set_char(glyphs::BULLET);
                c.set_style(Style::default().fg(theme::dim(theme::TEXT, 0.5)).bg(theme::PANEL_BG));
            }
        }
    }
    buf.set_stringn(eqx + 1, eqy, format!("{} equilibrium ({:.0}, {:.0})", glyphs::DIAMOND, mean_prey, mean_pred), 26, Style::default().fg(theme::INFO).bg(theme::PANEL_BG));
}

/// The phase-plot axes, tick labels and quadrant captions.
#[allow(clippy::too_many_arguments)]
fn phase_axes(buf: &mut Buffer, inner: Rect, px: u16, pw: u16, py: u16, ph: u16, max_prey: f32, max_pred: f32) {
    // Axes.
    for x in px..px + pw {
        if let Some(c) = buf.cell_mut((x, py + ph)) {
            c.set_char(glyphs::H_LINE);
            c.set_style(Style::default().fg(theme::DIM).bg(theme::PANEL_BG));
        }
    }
    for r in 0..ph {
        if let Some(c) = buf.cell_mut((px - 1, py + r)) {
            c.set_char(glyphs::V_LINE);
            c.set_style(Style::default().fg(theme::DIM).bg(theme::PANEL_BG));
        }
    }
    for k in 0..=4u16 {
        let y = py + ph - 1 - crate::cast!((f32::from(ph - 1) * f32::from(k) / 4.0).round() => u16);
        let v = crate::cast!((max_pred * f32::from(k) / 4.0).round() => u32);
        buf.set_stringn(inner.x, y, format!("{v:>5}"), 5, theme::dim_text());
    }
    for k in 0..=4u16 {
        let x = px + crate::cast!((f32::from(pw - 1) * f32::from(k) / 4.0).round() => u16);
        let v = crate::cast!((max_prey * f32::from(k) / 4.0).round() => u32);
        buf.set_stringn(x, py + ph + 1, format!("{v:>4}"), 4, theme::dim_text());
    }
    buf.set_stringn(inner.x, py, "pred", 4, Style::default().fg(theme::PRED).bg(theme::PANEL_BG));
    buf.set_stringn(px, py + ph + 1, "prey total", 10, Style::default().fg(theme::PREY).bg(theme::PANEL_BG));

    // Quadrant captions.
    buf.set_stringn(px + 1, py + 1, "II few prey, many predators", 27, theme::dim_text());
    buf.set_stringn(px + pw.saturating_sub(26), py + 1, "I many prey, many predators", 26, theme::dim_text());
    buf.set_stringn(px + 1, py + ph.saturating_sub(2), "III few of both", 15, theme::dim_text());
    buf.set_stringn(px + pw.saturating_sub(24), py + ph.saturating_sub(2), "IV many prey, few predators", 26, theme::dim_text());
}

pub(super) fn phase_sidebar(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw(f, area, "Phase", panel::Kind::Outer);
    let mut row = 0u16;
    let prey_now = sim.species.iter().filter(|s| sim.roster().kind(s.species) == Kind::Prey).map(|s| s.count).sum::<u32>();
    let pred_now = sim.species.iter().filter(|s| sim.roster().kind(s.species) == Kind::Predator).map(|s| s.count).sum::<u32>();
    panel::section(f, inner, row, "Now");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" prey      ", theme::dim_text()),
        sp(format!("{prey_now:>5}"), Style::default().fg(theme::PREY).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp("  predators  ", theme::dim_text()),
        sp(format!("{pred_now:>4}"), Style::default().fg(theme::PRED).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
    ]));
    row += 2;

    panel::section(f, inner, row, "Equilibrium estimate");
    row += 1;
    let mean_prey = w.prey.iter().sum::<f32>() / crate::cast!(w.len().max(1) => f32);
    let mean_pred = w.pred.iter().sum::<f32>() / crate::cast!(w.len().max(1) => f32);
    util::line(f, inner, row, Line::from(vec![
        sp(" prey*  ", theme::dim_text()), sp(format!("{mean_prey:.0}"), theme::text()),
        sp("   pred*  ", theme::dim_text()), sp(format!("{mean_pred:.0}"), theme::text()),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(sp(" the time means mark the orbit centre", theme::dim_text())));
    row += 2;

    panel::section(f, inner, row, "Quadrants");
    row += 1;
    for (name, desc, color) in [
        ("I", "many prey, many predators", theme::PRED),
        ("II", "few prey, many predators", theme::WARN),
        ("III", "few of both", theme::PREY),
        ("IV", "many prey, few predators", theme::GOOD),
    ] {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {name} "), Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(desc, theme::text()),
        ]));
        row += 1;
    }
    row += 2;

    panel::section(f, inner, row, "Recent path");
    row += 1;
    let n = w.len();
    if n > 0 {
        for back in [40usize, 30, 20, 10, 0] {
            if back >= n {
                continue;
            }
            let i = n - 1 - back;
            let day = w.day_label(i);
            util::line(f, inner, row, Line::from(vec![
                sp(format!(" {day:<9}"), theme::dim_text()),
                sp(format!("prey {:>4}", crate::cast!(w.prey[i] => u32)), theme::text()),
                sp(format!("  pred {:>4}", crate::cast!(w.pred[i] => u32)), theme::text()),
            ]));
            row += 1;
        }
    }
    row += 1;

    panel::section(f, inner, row, "Legend");
    row += 1;
    for (glyph, desc, color) in [
        ("•", "older days", theme::DIM),
        ("▀▄", "last 40 days", theme::ACCENT),
        ("█", "today", theme::TEXT_BRIGHT),
        ("·", "equilibrium axes (time means)", theme::DIM),
    ] {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {glyph:<3}"), Style::default().fg(color).bg(theme::PANEL_BG)),
            sp(desc, theme::text()),
        ]));
        row += 1;
    }
    let _ = sim;
}
