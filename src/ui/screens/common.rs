//! Small helpers shared by the data screens (S03/S04/S05/S08).

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::{glyphs, theme};

pub fn sp(s: impl Into<String>, st: Style) -> Span<'static> {
    Span::styled(s.into(), st)
}

/// The C3 trend rule over a daily count series: > +3 % ↑, < −3 % ↓, else ↔,
/// comparing the last value with the one up to 30 days earlier.
pub fn trend_arrow(counts: &[u16]) -> char {
    if counts.len() < 2 {
        return glyphs::FLAT;
    }
    let a = counts[counts.len().saturating_sub(30).min(counts.len() - 1)] as f32;
    let b = *counts.last().unwrap() as f32;
    let pct = if a > 0.0 { (b - a) / a * 100.0 } else if b > 0.0 { f32::INFINITY } else { 0.0 };
    if pct > 3.0 {
        glyphs::UP
    } else if pct < -3.0 {
        glyphs::DOWN
    } else {
        glyphs::FLAT
    }
}

pub fn arrow_color(a: char) -> Color {
    match a {
        glyphs::UP => theme::GOOD,
        glyphs::DOWN => theme::BAD,
        _ => theme::DIM,
    }
}

pub fn trait_color(t: usize) -> Color {
    match t {
        0 => theme::INFO,
        1 => theme::DEER,
        2 => theme::ACCENT,
        3 => theme::WARN,
        4 => theme::BAD,
        5 => theme::VEGETATION,
        6 => theme::MAGENTA,
        _ => theme::LYNX,
    }
}

pub fn delta_style(d: f32) -> Style {
    let c = if d > 0.005 {
        theme::GOOD
    } else if d < -0.005 {
        theme::BAD
    } else {
        theme::DIM
    };
    Style::default().fg(c).bg(theme::PANEL_BG)
}

/// Two-digit trait value: 0.74 -> "74".
pub fn two(v: f32) -> String {
    format!("{:>2}", ((v * 100.0).round() as u32).min(99))
}

/// Downsample a series to `cols` u16 buckets (mean of each bucket).
pub fn downsample(series: &[f32], cols: usize) -> Vec<u16> {
    let n = series.len();
    if n == 0 || cols == 0 {
        return vec![0; cols];
    }
    (0..cols)
        .map(|i| {
            let a = i * n / cols;
            let b = ((i + 1) * n / cols).max(a + 1).min(n);
            let s: f32 = series[a..b].iter().sum::<f32>() / (b - a) as f32;
            s.round() as u16
        })
        .collect()
}

/// Truncate to `max` cells with a trailing dot.
pub fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut t: String = text.chars().take(max.saturating_sub(1)).collect();
        t.push(glyphs::DOT);
        t
    }
}

/// `Y<year> D<day>` for an absolute day index (negative = before the run).
pub fn day_stamp(day: i64, season_days: u32) -> String {
    if day < 0 {
        return "founder".to_string();
    }
    let year_len = (4 * season_days) as i64;
    format!("Y{} D{:03}", day / year_len + 1, day % year_len + 1)
}
