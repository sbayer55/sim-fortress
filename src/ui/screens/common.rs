//! Small helpers shared by the data screens (S03/S04/S05/S08).

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::{glyphs, theme};

pub fn sp(s: impl Into<String>, st: Style) -> Span<'static> {
    Span::styled(s.into(), st)
}

pub use crate::widgets::trend::{arrow_color, trend_arrow};

/// One colour per genome slot. Deliberately exhaustive (no catch-all): adding a
/// trait must be a compile error here rather than a silently shared colour.
pub const fn trait_color(t: usize) -> Color {
    match t {
        0 => theme::INFO,
        1 => theme::TAN,
        2 => theme::ACCENT,
        3 => theme::WARN,
        4 => theme::BAD,
        5 => theme::VEGETATION,
        6 => theme::MAGENTA,
        7 => theme::ROSE,
        8 => theme::SICK,
        9 => theme::CREAM,
        10 => theme::SEED,
        _ => theme::DIM,
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
    format!("{:>2}", (crate::cast!((v * 100.0).round() => u32)).min(99))
}

/// Downsample a series to `cols` u16 buckets (mean of each bucket).
pub fn downsample(series: &[f32], cols: usize) -> Vec<u16> {
    let n = series.len();
    if n == 0 || cols == 0 {
        return vec![0; cols];
    }
    (0..cols)
        .map(|i| {
            let a = (i * n).div_euclid(cols);
            let b = (((i + 1) * n).div_euclid(cols)).max(a + 1).min(n);
            let s: f32 = series[a..b].iter().sum::<f32>() / crate::cast!((b - a) => f32);
            crate::cast!(s.round() => u16)
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
    let year_len = i64::from(4 * season_days);
    format!("Y{} D{:03}", day.div_euclid(year_len) + 1, day % year_len + 1)
}
