//! Labeled horizontal bars for vitals and traits.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::{glyphs, theme};

/// `label [████░░░░]  82%` on one row. `area.width` bounds the whole thing.
pub fn labeled(buf: &mut Buffer, area: Rect, row: u16, label: &str, value: f32, color: Color, label_w: u16, bar_w: u16) {
    if row >= area.height {
        return;
    }
    let y = area.y + row;
    let x = area.x;
    buf.set_stringn(x, y, format!("{:<w$}", label, w = label_w as usize), label_w as usize, theme::text());
    let bx = x + label_w;
    bar(buf, bx, y, bar_w, value, color);
    let px = bx + bar_w + 2;
    let pct = format!("{:>3}%", (value.clamp(0.0, 1.0) * 100.0).round() as u32);
    buf.set_stringn(px + 1, y, &pct, 4, theme::text());
}

/// Just the `[████░░░░]` part, `w` cells wide including brackets.
pub fn bar(buf: &mut Buffer, x: u16, y: u16, w: u16, value: f32, color: Color) {
    let inner = w.saturating_sub(2) as usize;
    let filled = (value.clamp(0.0, 1.0) * inner as f32).round() as usize;
    let mut s = String::with_capacity(w as usize);
    s.push(glyphs::BAR_L);
    for i in 0..inner {
        s.push(if i < filled { glyphs::BAR_FILL } else { glyphs::BAR_EMPTY });
    }
    s.push(glyphs::BAR_R);
    buf.set_stringn(x, y, &s, w as usize, Style::default().fg(theme::DIM).bg(theme::PANEL_BG));
    // Recolor the filled part.
    let fill: String = std::iter::repeat_n(glyphs::BAR_FILL, filled).collect();
    buf.set_stringn(x + 1, y, &fill, filled, Style::default().fg(color).bg(theme::PANEL_BG));
}

/// A range bar showing min..max as a shaded span with a marker at `mean`.
/// Used for trait distributions: `[░░░▒▒▒█▒▒░░░░]`.
pub fn range(buf: &mut Buffer, x: u16, y: u16, w: u16, min: f32, mean: f32, max: f32, color: Color) {
    let inner = w.saturating_sub(2) as usize;
    let cell = |v: f32| ((v.clamp(0.0, 1.0) * (inner as f32 - 1.0)).round() as usize).min(inner.saturating_sub(1));
    let (a, m, b) = (cell(min), cell(mean), cell(max));
    let mut s = String::new();
    s.push(glyphs::BAR_L);
    for i in 0..inner {
        s.push(if i == m {
            glyphs::SHADE_4
        } else if i >= a && i <= b {
            glyphs::SHADE_2
        } else {
            glyphs::SHADE_1
        });
    }
    s.push(glyphs::BAR_R);
    buf.set_stringn(x, y, &s, w as usize, Style::default().fg(theme::DIM).bg(theme::PANEL_BG));
    let span: String = (a..=b).map(|i| if i == m { glyphs::SHADE_4 } else { glyphs::SHADE_2 }).collect();
    buf.set_stringn(x + 1 + a as u16, y, &span, b - a + 1, Style::default().fg(color).bg(theme::PANEL_BG));
}

/// Color for a vital: green when healthy, red when critical.
/// `inverted` = true for hunger/thirst where high is bad.
pub fn vital_color(value: f32, inverted: bool) -> Color {
    let t = if inverted { 1.0 - value } else { value };
    if t > 0.6 {
        theme::GOOD
    } else if t > 0.3 {
        theme::WARN
    } else {
        theme::BAD
    }
}

/// Vertical histogram in an area: `values` become columns of `▄`/`█` stacks.
pub fn histogram(buf: &mut Buffer, area: Rect, values: &[u16], color: Color, col_w: u16) {
    let max = values.iter().copied().max().unwrap_or(1).max(1) as f32;
    let h = area.height;
    for (i, v) in values.iter().enumerate() {
        let x = area.x + i as u16 * col_w;
        if x + col_w > area.right() {
            break;
        }
        // Two half-cells per row.
        let halves = ((*v as f32 / max) * (h as f32 * 2.0)).round() as u16;
        for r in 0..h {
            let y = area.bottom() - 1 - r;
            let level = halves.saturating_sub(r * 2);
            let ch = if level >= 2 {
                glyphs::FULL_BLOCK
            } else if level == 1 {
                glyphs::HALF_LOWER
            } else {
                continue;
            };
            let s: String = std::iter::repeat_n(ch, col_w.saturating_sub(1).max(1) as usize).collect();
            buf.set_stringn(x, y, &s, col_w as usize, Style::default().fg(color).bg(theme::PANEL_BG));
        }
    }
}

/// One-row sparkline using `░▒▓█` intensity (CP437-safe).
pub fn sparkline(buf: &mut Buffer, x: u16, y: u16, w: u16, values: &[u16], color: Color) {
    let n = values.len().min(w as usize);
    let slice = &values[values.len() - n..];
    let max = slice.iter().copied().max().unwrap_or(1).max(1) as f32;
    let min = slice.iter().copied().min().unwrap_or(0) as f32;
    let s: String = slice
        .iter()
        .map(|&v| {
            let t = if max > min { (v as f32 - min) / (max - min) } else { 0.5 };
            let i = (t * 3.0).round() as usize + 1;
            glyphs::SHADES[i.min(4)]
        })
        .collect();
    buf.set_stringn(x, y, &s, n, Style::default().fg(color).bg(theme::PANEL_BG));
}
