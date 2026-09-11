//! S04: species browser table and per-species detail.

#[allow(unused_imports)]
use crate::fixtures::{EventKindStyle as _, SeasonStyle as _, SpeciesStyle as _};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::Prototype;
use crate::fixtures::{self, Fixtures, Kind, Species, SpeciesId, TRAIT_NAMES};
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Table,
    Detail,
}

pub struct SpeciesBrowser {
    pub variant: Variant,
}

pub fn all() -> Vec<Box<dyn Prototype>> {
    vec![
        Box::new(SpeciesBrowser { variant: Variant::Table }),
        Box::new(SpeciesBrowser { variant: Variant::Detail }),
    ]
}

const SELECTED: SpeciesId = SpeciesId::Wolf;
const TABLE_H: u16 = 13;

fn sp(s: impl Into<String>, st: Style) -> Span<'static> {
    Span::styled(s.into(), st)
}

fn arrow_color(a: char) -> Color {
    match a {
        glyphs::UP => theme::GOOD,
        glyphs::DOWN => theme::BAD,
        _ => theme::DIM,
    }
}

fn trait_color(t: usize) -> Color {
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

fn delta_style(d: f32, bg: Color) -> Style {
    let c = if d > 0.005 {
        theme::GOOD
    } else if d < -0.005 {
        theme::BAD
    } else {
        theme::DIM
    };
    Style::default().fg(c).bg(bg)
}

/// Two-digit trait value: 0.74 -> "74".
fn two(v: f32) -> String {
    format!("{:>2}", ((v * 100.0).round() as u32).min(99))
}

/// Downsample a f32 series to `cols` u16 buckets (mean of each bucket).
fn downsample(series: &[f32], cols: usize) -> Vec<u16> {
    let n = series.len();
    (0..cols)
        .map(|i| {
            let a = i * n / cols;
            let b = ((i + 1) * n / cols).max(a + 1).min(n);
            let s: f32 = series[a..b].iter().sum::<f32>() / (b - a) as f32;
            s.round() as u16
        })
        .collect()
}

impl Prototype for SpeciesBrowser {
    fn id(&self) -> &'static str {
        match self.variant {
            Variant::Table => "S04a",
            Variant::Detail => "S04b",
        }
    }
    fn name(&self) -> &'static str {
        "Species Browser"
    }
    fn variant(&self) -> &'static str {
        match self.variant {
            Variant::Table => "species table",
            Variant::Detail => "species detail (Wolf)",
        }
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        let fx = fixtures::get();
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        match self.variant {
            Variant::Table => {
                table(f, Rect::new(area.x, area.y, area.width, TABLE_H), fx);
                summary(f, Rect::new(area.x, area.y + TABLE_H, area.width, body_h - TABLE_H), fx);
                status::render(
                    f,
                    Rect::new(area.x, status_row, area.width, 1),
                    &[("↑↓", "select"), ("Enter", "detail"), ("s", "sort"), ("Esc", "back")],
                    &format!("sorted by count {}  {}", glyphs::DOWN, fx.clock.label()),
                );
            }
            Variant::Detail => {
                let left_w = 80u16;
                histograms(f, Rect::new(area.x, area.y, left_w, body_h), fx);
                drift(f, Rect::new(area.x + left_w, area.y, area.width - left_w, body_h), fx);
                status::render(
                    f,
                    Rect::new(area.x, status_row, area.width, 1),
                    &[("↑↓", "select"), ("Enter", "detail"), ("s", "sort"), ("Esc", "back")],
                    &format!("Wolf detail  {}", fx.clock.label()),
                );
            }
        }
    }
}

// ------------------------------------------------------------------ S04a

fn table(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let inner = panel::draw_with_hint(f, area, "Species", "6 species, 3 prey / 3 predator", panel::Kind::Outer);
    let dim = theme::dim_text();
    let header = Line::from(vec![
        sp("   ", dim),
        sp(format!("{:<8}", "Species"), dim),
        sp(format!("{:<6}", "Kind"), dim),
        sp(format!("{:>6}", "Count"), dim),
        sp(format!("{:>7}", "Adults"), dim),
        sp(format!("{:>6}", "Juv"), dim),
        sp(format!("{:>8}", "Birth/d"), dim),
        sp(format!("{:>8}", "Death/d"), dim),
        sp(format!("{:>6}", "Peak"), dim),
        sp(format!("{:>5}", "Gen"), dim),
        sp("  30-day trend         ", dim),
        sp("  ", dim),
        sp("  Spd Siz Sen Met Agg Cam Fer Lon", dim),
        sp("   Diet", dim),
    ]);
    util::line(f, inner, 0, header);
    let mut sorted: Vec<&Species> = fx.species.iter().collect();
    sorted.sort_by(|a, b| b.count.cmp(&a.count));
    let mut row = 2u16;
    for s in &sorted {
        let selected = s.id == SELECTED;
        let base = if selected { theme::selected() } else { theme::text() };
        let bg = if selected { theme::SELECT_BG } else { theme::PANEL_BG };
        let dimmed = if selected { base } else { theme::dim_text() };
        let kind = match s.id.kind() {
            Kind::Prey => "prey",
            Kind::Predator => "pred",
        };
        let marker = if selected { glyphs::PLAY } else { ' ' };
        let arrow = s.trend_arrow();
        let mut spans = vec![
            sp(format!("{}", marker), Style::default().fg(theme::KEY).bg(bg).add_modifier(Modifier::BOLD)),
            sp(format!("{} ", s.id.glyph().to_ascii_uppercase()), Style::default().fg(s.id.color()).bg(bg).add_modifier(Modifier::BOLD)),
            sp(format!("{:<8}", s.id.name()), base),
            sp(format!("{:<6}", kind), dimmed),
            sp(format!("{:>6}", s.count), base),
            sp(format!("{:>7}", s.adults), base),
            sp(format!("{:>6}", s.juveniles), base),
            sp(format!("{:>8}", s.births_today), Style::default().fg(theme::GOOD).bg(bg)),
            sp(format!("{:>8}", s.deaths_today), Style::default().fg(theme::BAD).bg(bg)),
            sp(format!("{:>6}", s.peak), base),
            sp(format!("{:>5}", s.generation), base),
            sp(format!("{:<23}", ""), base), // sparkline slot
            sp(format!("{} ", arrow), Style::default().fg(arrow_color(arrow)).bg(bg).add_modifier(Modifier::BOLD)),
        ];
        spans.push(sp("  ", base));
        for t in 0..8 {
            spans.push(sp(format!("{:>3} ", two(s.mean.0[t])), Style::default().fg(trait_color(t)).bg(bg)));
        }
        spans.push(sp(format!("  {}", s.id.diet()), dimmed));
        let line = Line::from(spans);
        util::line(f, inner, row, line);
        if selected {
            // Extend selection highlight across the full row.
            let w = inner.width as usize;
            let buf = f.buffer_mut();
            for x in 0..w as u16 {
                if let Some(c) = buf.cell_mut((inner.x + x, inner.y + row)) {
                    c.set_bg(bg);
                }
            }
        }
        bars::sparkline(f.buffer_mut(), inner.x + 65, inner.y + row, 20, &s.trend, s.id.color());
        row += 1;
    }
    // Totals row.
    row += 1;
    let prey: u32 = fx.species.iter().filter(|s| s.id.kind() == Kind::Prey).map(|s| s.count).sum();
    let pred: u32 = fx.species.iter().filter(|s| s.id.kind() == Kind::Predator).map(|s| s.count).sum();
    let births: u32 = fx.species.iter().map(|s| s.births_today).sum();
    let deaths: u32 = fx.species.iter().map(|s| s.deaths_today).sum();
    util::line(f, inner, row, Line::from(vec![
        sp(format!("   {:<14}", "totals"), theme::label()),
        sp(format!("{:>6}", prey + pred), theme::text()),
        sp(format!("   prey {}  pred {}  ratio {:.1}:1", prey, pred, prey as f32 / pred.max(1) as f32), theme::dim_text()),
        sp(format!("   births {}", births), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        sp(format!("  deaths {}", deaths), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
        sp(format!("   net {:+} today", births as i32 - deaths as i32), theme::text()),
        sp("      trait columns are species means x100", theme::dim_text()),
    ]));
}

fn summary(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let s = fx.species.iter().find(|s| s.id == SELECTED).unwrap();
    let inner = panel::draw_with_hint(f, area, &format!("Selected: {}", s.id.name()), "Enter for full detail", panel::Kind::Focus);
    let left_w = 64u16;
    let left = Rect::new(inner.x, inner.y, left_w, inner.height);
    let right = Rect::new(inner.x + left_w + 1, inner.y, inner.width - left_w - 1, inner.height);

    // ---- left: identity + trait table
    let mut row = 0u16;
    util::line(f, left, row, Line::from(vec![
        sp(format!(" {} ", s.id.glyph().to_ascii_uppercase()), Style::default().fg(s.id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(s.id.plural(), theme::title()),
        sp(format!("   predator   diet: {}", s.id.diet()), theme::text()),
    ]));
    row += 1;
    util::line(f, left, row, Line::from(vec![
        sp(format!(" {} alive  {} adults  {} juveniles  generation {}  peak {}", s.count, s.adults, s.juveniles, s.generation, s.peak), theme::dim_text()),
    ]));
    row += 2;
    panel::section(f, left, row, "Base genome vs current mean");
    row += 1;
    util::line(f, left, row, Line::from(sp(" trait        base   current            delta   spread", theme::dim_text())));
    row += 1;
    let base = s.id.base_genome();
    for t in 0..8 {
        let b = base.0[t];
        let m = s.mean.0[t];
        let d = m - b;
        let color = trait_color(t);
        let y = left.y + row;
        let buf = f.buffer_mut();
        buf.set_stringn(left.x, y, format!(" {:<12}", TRAIT_NAMES[t]), 13, theme::text());
        buf.set_stringn(left.x + 13, y, format!("{:.2}", b), 4, theme::dim_text());
        buf.set_stringn(left.x + 20, y, format!("{:.2}", m), 4, theme::text());
        bars::bar(buf, left.x + 25, y, 14, m, color);
        // Base marker inside the bar.
        let bx = left.x + 26 + (b * 11.0).round() as u16;
        buf.set_stringn(bx, y, glyphs::V_LINE.to_string(), 1, Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG));
        buf.set_stringn(left.x + 42, y, format!("{:+.2}", d), 5, delta_style(d, theme::PANEL_BG));
        bars::range(buf, left.x + 49, y, 13, s.min.0[t], m, s.max.0[t], color);
        row += 1;
    }
    util::line(f, left, row, Line::from(vec![
        sp(format!(" {} base marker   spread = min/mean/max across living {}", glyphs::V_LINE, s.id.plural()), theme::dim_text()),
    ]));
    row += 2;
    panel::section(f, left, row, "Interactions");
    row += 1;
    for (id, share) in [(SpeciesId::Deer, 0.48f32), (SpeciesId::Hare, 0.39), (SpeciesId::Vole, 0.13)] {
        let prey = fx.species.iter().find(|p| p.id == id).unwrap();
        let buf = f.buffer_mut();
        let y = left.y + row;
        buf.set_stringn(left.x, y, format!(" eats {} {:<5}", id.glyph().to_ascii_uppercase(), id.name()), 13, Style::default().fg(id.color()).bg(theme::PANEL_BG));
        bars::bar(buf, left.x + 13, y, 16, share, id.color());
        buf.set_stringn(left.x + 30, y, format!("{:>3}% of kills   {} alive {}", (share * 100.0).round() as u32, prey.count, prey.trend_arrow()), 33, theme::dim_text());
        row += 1;
    }
    util::line(f, left, row, Line::from(vec![
        sp(" competes with ", theme::dim_text()),
        sp("L Lynx", Style::default().fg(theme::LYNX).bg(theme::PANEL_BG)),
        sp(" for hares;  hunted by nothing", theme::dim_text()),
    ]));
    row += 2;
    panel::section(f, left, row, "Notable individuals");
    row += 1;
    let mut wolves: Vec<&fixtures::Creature> = fx.creatures.iter().filter(|c| c.alive && c.species == s.id).collect();
    wolves.sort_by(|a, b| b.kills.cmp(&a.kills));
    for c in wolves.iter().take(5) {
        if row >= left.height {
            break;
        }
        util::line(f, left, row, Line::from(vec![
            sp(format!(" {} ", c.glyph()), Style::default().fg(s.id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{:<9}{:<7}", c.name, c.tag()), theme::text()),
            sp(format!("{:>3} kills  gen {:<3} {:>4} days  ", c.kills, c.generation, c.age_days), theme::dim_text()),
            sp(fx.world.region_name(c.x, c.y), theme::dim_text()),
        ]));
        row += 1;
    }

    // ---- right: population history
    let mut row = 0u16;
    panel::section(f, right, row, "Population, last 240 days");
    row += 1;
    let series = fx.series.pop(s.id);
    let cols = 100usize;
    let data = downsample(series, cols);
    let max = *data.iter().max().unwrap_or(&1) as f32;
    let min = *data.iter().min().unwrap_or(&0) as f32;
    // A 6-row block chart built from the sparkline data: each row is a band.
    let rows = 6u16;
    {
        let buf = f.buffer_mut();
        for (i, v) in data.iter().enumerate() {
            let t = if max > min { (*v as f32 - min) / (max - min) } else { 0.5 };
            let halves = (t * rows as f32 * 2.0).round() as u16;
            for r in 0..rows {
                let y = right.y + row + rows - 1 - r;
                let level = halves.saturating_sub(r * 2);
                let ch = if level >= 2 {
                    glyphs::FULL_BLOCK
                } else if level == 1 {
                    glyphs::HALF_LOWER
                } else {
                    glyphs::SHADE_1
                };
                let color = if level >= 1 { s.id.color() } else { theme::dim(theme::DIM, 0.6) };
                buf.set_stringn(right.x + 6 + i as u16, y, ch.to_string(), 1, Style::default().fg(color).bg(theme::PANEL_BG));
            }
        }
        buf.set_stringn(right.x, right.y + row, format!("{:>4} ", max as u32), 5, theme::dim_text());
        buf.set_stringn(right.x, right.y + row + rows - 1, format!("{:>4} ", min as u32), 5, theme::dim_text());
    }
    row += rows;
    util::line(f, right, row, Line::from(vec![
        sp("      D-240", theme::dim_text()),
        sp(format!("{:>34}", "drought"), Style::default().fg(theme::WARN).bg(theme::PANEL_BG)),
        sp(format!("{:>52}", "today"), theme::dim_text()),
    ]));
    row += 2;
    util::line(f, right, row, Line::from(vec![
        sp(" 30-day trend ", theme::dim_text()),
    ]));
    bars::sparkline(f.buffer_mut(), right.x + 14, right.y + row, 30, &s.trend, s.id.color());
    let a = s.trend_arrow();
    f.buffer_mut().set_stringn(right.x + 45, right.y + row, format!(" {} {}", a, if a == glyphs::UP { "growing" } else if a == glyphs::DOWN { "declining" } else { "stable" }), 14, Style::default().fg(arrow_color(a)).bg(theme::PANEL_BG));
    row += 2;
    let first = series.first().copied().unwrap_or(0.0);
    let last = series.last().copied().unwrap_or(0.0);
    let lo = series.iter().cloned().fold(f32::MAX, f32::min);
    let hi = series.iter().cloned().fold(0.0f32, f32::max);
    let stats: Vec<(String, String, Style)> = vec![
        ("240 days ago".into(), format!("{:.0}", first), theme::text()),
        ("today".into(), format!("{}", s.count), theme::text()),
        ("change".into(), format!("{:+.0} ({:+.0}%)", last - first, (last - first) / first.max(1.0) * 100.0), delta_style(last - first, theme::PANEL_BG)),
        ("low / high".into(), format!("{:.0} / {:.0}", lo, hi), theme::text()),
        ("births today".into(), format!("{}", s.births_today), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        ("deaths today".into(), format!("{}", s.deaths_today), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
    ];
    for (k, v, st) in stats {
        util::line(f, right, row, Line::from(vec![
            sp(format!(" {:<14}", k), theme::dim_text()),
            sp(v, st),
        ]));
        row += 1;
    }
    row += 1;
    util::line(f, right, row, Line::from(vec![
        sp(format!(" {} ", glyphs::NOTE), theme::label()),
        sp("Wolves recovered after the Day 150 drought as deer herds returned to the Long Meadow.", theme::dim_text()),
    ]));
    row += 2;
    panel::section(f, right, row, "Habitat (living individuals by region)");
    row += 1;
    let mut per_region: Vec<(&str, usize)> = fx.world.regions.iter().map(|r| (r.0.as_str(), 0usize)).collect();
    for c in fx.creatures.iter().filter(|c| c.alive && c.species == s.id) {
        let name = fx.world.region_name(c.x, c.y);
        if let Some(e) = per_region.iter_mut().find(|e| e.0 == name) {
            e.1 += 1;
        }
    }
    per_region.sort_by(|a, b| b.1.cmp(&a.1));
    let max = per_region.first().map(|e| e.1).unwrap_or(1).max(1) as f32;
    let half = right.width / 2;
    for (i, (name, n)) in per_region.iter().enumerate() {
        let col = (i % 2) as u16;
        let r = row + (i / 2) as u16;
        if r >= right.height {
            break;
        }
        let x = right.x + 1 + col * half;
        let y = right.y + r;
        let buf = f.buffer_mut();
        buf.set_stringn(x, y, format!("{:<17}", name), 17, theme::text());
        bars::bar(buf, x + 17, y, 14, *n as f32 / max, s.id.color());
        buf.set_stringn(x + 32, y, format!("{:>2}", n), 2, theme::dim_text());
    }
}

// ------------------------------------------------------------------ S04b

fn histograms(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let s = fx.species.iter().find(|s| s.id == SELECTED).unwrap();
    let inner = panel::draw_with_hint(f, area, "Wolf: trait distributions", "12 buckets, living adults + juveniles", panel::Kind::Outer);
    let col_w = 38u16;
    let block_h = 9u16;
    for t in 0..8 {
        let col = (t / 4) as u16;
        let r = (t % 4) as u16;
        let x = inner.x + 1 + col * (col_w + 1);
        let y = inner.y + 1 + r * block_h;
        let color = trait_color(t);
        let mean = s.mean.0[t];
        let (min, max) = (s.min.0[t], s.max.0[t]);
        let buf = f.buffer_mut();
        buf.set_stringn(x, y, TRAIT_NAMES[t], 12, Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
        buf.set_stringn(x + 12, y, format!("min .{:02} mean .{:02} max .{:02}", (min * 100.0).round() as u32 % 100, (mean * 100.0).round() as u32 % 100, (max * 100.0).round() as u32 % 100), 26, theme::dim_text());
        let hist_area = Rect::new(x, y + 1, 36, 5);
        bars::histogram(buf, hist_area, &s.hist[t], color, 3);
        // Axis with a mean marker.
        let axis: String = std::iter::repeat_n(glyphs::H_LINE, 36).collect();
        buf.set_stringn(x, y + 6, &axis, 36, theme::border());
        let mx = x + ((mean * 35.0).round() as u16).min(35);
        buf.set_stringn(mx, y + 6, glyphs::CROSS.to_string(), 1, Style::default().fg(theme::TEXT_BRIGHT).bg(theme::PANEL_BG));
        buf.set_stringn(x, y + 7, "0.0", 3, theme::dim_text());
        buf.set_stringn(x + 16, y + 7, "0.5", 3, theme::dim_text());
        buf.set_stringn(x + 33, y + 7, "1.0", 3, theme::dim_text());
        let n: u16 = s.hist[t].iter().sum();
        let peak_bucket = s.hist[t].iter().enumerate().max_by_key(|(_, v)| **v).map(|(i, _)| i).unwrap_or(0);
        buf.set_stringn(x + 4, y + 7, format!("n={}", n), 8, theme::dim_text());
        buf.set_stringn(x + 20, y + 7, format!("mode {:.2}", (peak_bucket as f32 + 0.5) / 12.0), 12, theme::dim_text());
    }
    // Footer note.
    let y = inner.y + 1 + 4 * block_h;
    let buf = f.buffer_mut();
    buf.set_stringn(inner.x + 1, y, format!("{} mean   {} full  {} half bucket   each column is 1/12 of the 0..1 range", glyphs::CROSS, glyphs::FULL_BLOCK, glyphs::HALF_LOWER), inner.width as usize - 2, theme::dim_text());
}

fn drift(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let s = fx.species.iter().find(|s| s.id == SELECTED).unwrap();
    let inner = panel::draw_with_hint(f, area, "Drift over generations", "12 sampled generations", panel::Kind::Outer);
    let mut row = 0u16;
    {
        let y = inner.y + row;
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x, y, " trait       gen 1", 18, theme::dim_text());
        buf.set_stringn(inner.x + 21, y, "oldest", 6, theme::dim_text());
        buf.set_stringn(inner.x + 51, y, "newest", 6, theme::dim_text());
        buf.set_stringn(inner.x + 58, y, format!(" g{:<3} change", s.generation), 12, theme::dim_text());
    }
    row += 1;
    let n = s.drift.len();
    for t in 0..8 {
        let vals: Vec<u16> = s.drift.iter().map(|g| (g.0[t] * 100.0).round() as u16).collect();
        let first = s.drift[0].0[t];
        let last = s.drift[n - 1].0[t];
        let color = trait_color(t);
        let y = inner.y + row;
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x, y, format!(" {:<12}", TRAIT_NAMES[t]), 13, Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD));
        buf.set_stringn(inner.x + 13, y, format!("{:.2}", first), 4, theme::dim_text());
        // Widen each generation to 3 cells so the sparkline is readable.
        let wide: Vec<u16> = vals.iter().flat_map(|v| [*v, *v, *v]).collect();
        bars::sparkline(buf, inner.x + 21, y, 36, &wide, color);
        buf.set_stringn(inner.x + 59, y, format!("{:.2}", last), 4, theme::text());
        buf.set_stringn(inner.x + 66, y, format!("{:+.2}", last - first), 5, delta_style(last - first, theme::PANEL_BG));
        row += 1;
    }
    row += 1;
    panel::section(f, inner, row, "Per-generation means (x100)");
    row += 1;
    let mut hdr = String::from("              ");
    for g in 0..n {
        let gen = 1 + g as u32 * (s.generation - 1) / (n as u32 - 1);
        hdr.push_str(&format!("g{:<3}", gen));
    }
    util::line(f, inner, row, Line::from(sp(hdr, theme::dim_text())));
    row += 1;
    for t in 0..8 {
        let mut spans = vec![sp(format!(" {:<12} ", TRAIT_NAMES[t]), theme::text())];
        for g in 0..n {
            let v = s.drift[g].0[t];
            let prev = if g == 0 { v } else { s.drift[g - 1].0[t] };
            let st = if g == 0 { theme::dim_text() } else { delta_style(v - prev, theme::PANEL_BG) };
            spans.push(sp(format!("{:<4}", two(v)), st));
        }
        util::line(f, inner, row, Line::from(spans));
        row += 1;
    }
    util::line(f, inner, row, Line::from(sp(" green = rose vs previous sample, red = fell", theme::dim_text())));
    row += 2;

    panel::section(f, inner, row, "Population");
    row += 1;
    let stats: Vec<(String, String, Style)> = vec![
        ("count".into(), format!("{}  ({} adults, {} juveniles)", s.count, s.adults, s.juveniles), theme::text()),
        ("generation".into(), format!("{}", s.generation), theme::text()),
        ("peak".into(), format!("{}  ({}% of peak now)", s.peak, s.count * 100 / s.peak.max(1)), theme::text()),
        ("births today".into(), format!("{}", s.births_today), Style::default().fg(theme::GOOD).bg(theme::PANEL_BG)),
        ("deaths today".into(), format!("{}", s.deaths_today), Style::default().fg(theme::BAD).bg(theme::PANEL_BG)),
        ("trend".into(), format!("{} over 30 days", s.trend_arrow()), Style::default().fg(arrow_color(s.trend_arrow())).bg(theme::PANEL_BG)),
    ];
    for (k, v, st) in stats {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {:<14}", k), theme::dim_text()),
            sp(v, st),
        ]));
        row += 1;
    }
    row += 1;
    panel::section(f, inner, row, "Selection pressure");
    row += 1;
    let notes = [
        format!(" {} Aggression rising fastest: hungry packs out-breed timid ones", glyphs::MUTATION),
        format!(" {} Camouflage falling: open-meadow hunting favours speed over hiding", glyphs::MUTATION),
        format!(" {} Longevity flat: few wolves reach old age", glyphs::NOTE),
    ];
    for note in notes {
        util::line(f, inner, row, Line::from(sp(note, theme::dim_text())));
        row += 1;
    }
    row += 1;
    panel::section(f, inner, row, "Compared with other predators (mean x100)");
    row += 1;
    util::line(f, inner, row, Line::from(sp("              Spd Siz Sen Met Agg Cam Fer Lon   count  gen", theme::dim_text())));
    row += 1;
    for other in fx.species.iter().filter(|o| o.id.kind() == Kind::Predator) {
        let mut spans = vec![
            sp(format!(" {} ", other.id.glyph().to_ascii_uppercase()), Style::default().fg(other.id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{:<10}", other.id.name()), if other.id == s.id { theme::title() } else { theme::text() }),
        ];
        for t in 0..8 {
            spans.push(sp(format!(" {:>3}", two(other.mean.0[t])), Style::default().fg(trait_color(t)).bg(theme::PANEL_BG)));
        }
        spans.push(sp(format!("   {:>5}  {:>3}", other.count, other.generation), theme::dim_text()));
        util::line(f, inner, row, Line::from(spans));
        row += 1;
    }
}
