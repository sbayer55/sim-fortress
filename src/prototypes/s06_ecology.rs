//! S06: resources / ecology overview. Totals with sparklines, the season
//! table, and a per-region table with vegetation, moisture and population.

#[allow(unused_imports)]
use crate::fixtures::{EventKindStyle as _, SeasonStyle as _, SpeciesStyle as _};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::Prototype;
use crate::fixtures::{self, series, Fixtures, Kind, Season, Terrain};
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

pub struct Ecology;

pub fn all() -> Vec<Box<dyn Prototype>> {
    vec![Box::new(Ecology)]
}

const TOP_H: u16 = 24;
const TOTALS_W: u16 = 90;

impl Prototype for Ecology {
    fn id(&self) -> &'static str {
        "S06a"
    }
    fn name(&self) -> &'static str {
        "Resources / Ecology"
    }
    fn variant(&self) -> &'static str {
        "default"
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        let fx = fixtures::get();
        let body_h = area.height - 1;
        totals(f, Rect::new(area.x, area.y, TOTALS_W, TOP_H), fx);
        season(f, Rect::new(area.x + TOTALS_W, area.y, area.width - TOTALS_W, TOP_H), fx);
        regions(f, Rect::new(area.x, area.y + TOP_H, area.width, body_h - TOP_H), fx);
        status::render(
            f,
            Rect::new(area.x, area.y + area.height - 1, area.width, 1),
            &[("r", "sort regions"), ("Tab", "focus panel"), ("Esc", "back")],
            &format!("{}  {} day", fx.clock.label(), glyphs::SUN),
        );
    }
}

fn sp(s: impl Into<String>, fg: Color) -> Span<'static> {
    Span::styled(s.into(), Style::default().fg(fg).bg(theme::PANEL_BG))
}

fn bold(s: impl Into<String>, fg: Color) -> Span<'static> {
    Span::styled(s.into(), Style::default().fg(fg).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
}

/// Downsample a series to `w` points, scaled to u16 for `bars::sparkline`.
fn spark(v: &[f32], w: usize) -> Vec<u16> {
    (0..w)
        .map(|c| {
            let a = c * v.len() / w;
            let b = ((c + 1) * v.len() / w).max(a + 1).min(v.len());
            let m = v[a..b].iter().sum::<f32>() / (b - a) as f32;
            (m * 1000.0).round() as u16
        })
        .collect()
}

// ------------------------------------------------------------------ totals

fn totals(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let inner = panel::draw_with_hint(f, area, "Totals", "sparklines = last 240 days", panel::Kind::Outer);
    let s = &fx.series;
    let n = series::LEN;
    let world = &fx.world;
    let veg_now = s.vegetation[n - 1];
    let water_now = s.water[n - 1];
    let carc_now = world.carcasses.len() as f32;
    let carc_max = s.carcasses.iter().cloned().fold(1.0, f32::max);
    // Dens and regrowth have no series: derive them from what drives them
    // (dens follow predators, regrowth sites appear where vegetation is thin).
    let pred = s.pred_total();
    let dens_series: Vec<f32> = pred.iter().map(|p| p / pred[n - 1].max(1.0) * world.dens.len() as f32 / 20.0).collect();
    let seed_series: Vec<f32> = s.vegetation.iter().map(|v| (1.0 - v) * world.seeds.len() as f32 / (1.0 - veg_now).max(0.05) / 30.0).collect();

    let mut row = 0;
    panel::section(f, inner, row, "Now");
    row += 1;
    util::line(f, inner, row, Line::from(sp(format!("{:<14}{:<26}{:>6}   {:<40}", " resource", "level", "value", "trend"), theme::DIM)));
    row += 1;
    let label_w = 13;
    let bar_w = 20;
    let spark_x = inner.x + label_w + bar_w + 10;
    let spark_w = (inner.right() - spark_x - 1) as usize;
    let rows: [(&str, f32, Color, String, &[f32]); 5] = [
        (" vegetation", veg_now, theme::VEGETATION, format!("{:.0}%", veg_now * 100.0), &s.vegetation),
        (" water", water_now, theme::SHALLOW_FG, format!("{:.0}%", water_now * 100.0), &s.water),
        (" carcasses", carc_now / carc_max, theme::CARCASS, format!("{}", world.carcasses.len()), &s.carcasses),
        (" dens", world.dens.len() as f32 / 20.0, theme::DEN, format!("{}", world.dens.len()), &dens_series),
        (" regrowth", world.seeds.len() as f32 / 30.0, theme::SEED, format!("{}", world.seeds.len()), &seed_series),
    ];
    for (label, v01, color, value, series) in rows {
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x, inner.y + row, format!("{:<w$}", label, w = label_w as usize), label_w as usize, theme::text());
        bars::bar(buf, inner.x + label_w, inner.y + row, bar_w, v01, color);
        buf.set_stringn(inner.x + label_w + bar_w + 2, inner.y + row, format!("{:>6}", value), 6, theme::text());
        bars::sparkline(buf, spark_x, inner.y + row, spark_w as u16, &spark(series, spark_w), color);
        row += 1;
    }
    util::line(f, inner, row, Line::from(vec![
        sp(" 30d: ", theme::DIM),
        delta("veg", &s.vegetation),
        delta("water", &s.water),
        delta("carcasses", &s.carcasses),
        sp("  (dens, regrowth trends derived)", theme::DIM),
    ]));
    row += 2;

    // Terrain composition.
    panel::section(f, inner, row, "Terrain");
    row += 1;
    let kinds = [
        Terrain::DeepWater,
        Terrain::ShallowWater,
        Terrain::Sand,
        Terrain::Dirt,
        Terrain::GrassSparse,
        Terrain::Grass,
        Terrain::GrassDense,
        Terrain::Forest,
        Terrain::Rock,
    ];
    let total = (world.width() * world.height()) as f32;
    let mut veg_by: Vec<(Terrain, usize, f32)> = kinds
        .iter()
        .map(|&t| {
            let cells: Vec<&fixtures::Cell> = world.cells.iter().filter(|c| c.terrain == t).collect();
            let v = if cells.is_empty() { 0.0 } else { cells.iter().map(|c| c.vegetation).sum::<f32>() / cells.len() as f32 };
            (t, cells.len(), v)
        })
        .collect();
    veg_by.sort_by(|a, b| b.1.cmp(&a.1));
    let max_cells = veg_by[0].1 as f32;
    for (t, count, v) in &veg_by {
        let cell = fixtures::Cell { terrain: *t, elevation: 0.5, moisture: 0.5, vegetation: *v, prey_pressure: 0.0, pred_pressure: 0.0 };
        let (g, fg, bg) = crate::widgets::map::terrain_cell(&cell, false);
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x + 1, inner.y + row, g.to_string(), 1, Style::default().fg(fg).bg(bg));
        buf.set_stringn(inner.x + 3, inner.y + row, format!("{:<14}", t.name()), 14, theme::text());
        bars::bar(buf, inner.x + 17, inner.y + row, 24, *count as f32 / max_cells, fg);
        buf.set_stringn(inner.x + 43, inner.y + row, format!("{:>5} cells {:>4.0}%", count, *count as f32 / total * 100.0), 18, theme::dim_text());
        if *v > 0.0 {
            buf.set_stringn(inner.x + 63, inner.y + row, "biomass ", 8, theme::dim_text());
            bars::bar(buf, inner.x + 71, inner.y + row, 12, *v, theme::VEGETATION);
            buf.set_stringn(inner.x + 84, inner.y + row, format!("{:.2}", v), 4, theme::text());
        } else {
            buf.set_stringn(inner.x + 63, inner.y + row, "no forage", 9, theme::dim_text());
        }
        row += 1;
    }
    row += 1;
    let water_cells = world.cells.iter().filter(|c| c.terrain.is_water()).count();
    let walkable = world.cells.iter().filter(|c| c.terrain.walkable()).count();
    let biomass: f32 = world.cells.iter().map(|c| c.vegetation).sum();
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" standing biomass {:.0} units over {} walkable cells; {} water cells ({:.0}%)", biomass, walkable, water_cells, water_cells as f32 / total * 100.0), theme::DIM),
    ]));
    row += 1;
    let prey_p = world.cells.iter().map(|c| c.prey_pressure).sum::<f32>() / total;
    let pred_p = world.cells.iter().map(|c| c.pred_pressure).sum::<f32>() / total;
    let buf = f.buffer_mut();
    buf.set_stringn(inner.x, inner.y + row, " prey pressure", 14, theme::text());
    buf.set_stringn(inner.x + 80, inner.y + row, "(mean)", 6, theme::dim_text());
    bars::bar(buf, inner.x + 15, inner.y + row, 18, prey_p * 4.0, theme::HARE);
    buf.set_stringn(inner.x + 34, inner.y + row, format!("{:.2}", prey_p), 4, theme::text());
    buf.set_stringn(inner.x + 42, inner.y + row, "pred pressure", 13, theme::text());
    bars::bar(buf, inner.x + 56, inner.y + row, 18, pred_p * 4.0, theme::WOLF);
    buf.set_stringn(inner.x + 75, inner.y + row, format!("{:.2}", pred_p), 4, theme::text());
}

fn delta(label: &str, v: &[f32]) -> Span<'static> {
    let n = v.len();
    let (a, b) = (v[n - 31], v[n - 1]);
    let d = (b - a) / a.max(0.01) * 100.0;
    let (arrow, color) = if d > 3.0 { (glyphs::UP, theme::GOOD) } else if d < -3.0 { (glyphs::DOWN, theme::BAD) } else { (glyphs::FLAT, theme::DIM) };
    Span::styled(format!("{} {} {:+.0}%  ", label, arrow, d), Style::default().fg(color).bg(theme::PANEL_BG))
}

// ------------------------------------------------------------------ season

fn season(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let inner = panel::draw(f, area, "Season", panel::Kind::Outer);
    let clock = &fx.clock;
    let cur = clock.season;
    let day_of = clock.day;
    let left = 90 - day_of;
    let next = match cur {
        Season::Spring => Season::Summer,
        Season::Summer => Season::Autumn,
        Season::Autumn => Season::Winter,
        Season::Winter => Season::Spring,
    };
    let mut row = 0;
    util::line(f, inner, row, Line::from(vec![
        Span::styled(format!(" {} {} ", cur.glyph(), cur.name()), Style::default().fg(cur.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(format!("  day {} of 90", day_of), theme::TEXT),
        sp(format!("   year {}", clock.year), theme::DIM),
    ]));
    row += 1;
    let buf = f.buffer_mut();
    buf.set_stringn(inner.x + 1, inner.y + row, "progress", 8, theme::dim_text());
    bars::bar(buf, inner.x + 10, inner.y + row, 30, day_of as f32 / 90.0, cur.color());
    buf.set_stringn(inner.x + 42, inner.y + row, format!("{:>3}%", day_of * 100 / 90), 4, theme::text());
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" next: ", theme::DIM),
        Span::styled(format!("{} {}", next.glyph(), next.name()), Style::default().fg(next.color()).bg(theme::PANEL_BG)),
        sp(format!(" in {} days", left), theme::TEXT),
        sp(format!("   {} sunrise 06:00  sunset 18:00", glyphs::SUN), theme::DIM),
    ]));
    row += 2;

    panel::section(f, inner, row, "Modifiers");
    row += 1;
    util::line(f, inner, row, Line::from(sp("  season    regrowth   evaporation   metabolism   forage", theme::DIM)));
    row += 1;
    let table: [(Season, f32, f32, f32, &str); 4] = [
        (Season::Spring, 1.3, 0.8, 1.0, "lush"),
        (Season::Summer, 1.0, 1.4, 1.0, "drying"),
        (Season::Autumn, 0.8, 0.9, 1.1, "fading"),
        (Season::Winter, 0.5, 0.6, 1.3, "scarce"),
    ];
    for (s, regrow, evap, metab, forage) in table {
        let selected = s == cur;
        let base = if selected { theme::selected() } else { theme::text() };
        let bg = if selected { theme::SELECT_BG } else { theme::PANEL_BG };
        let mods = |v: f32, good_high: bool| -> Style {
            let c = if (v > 1.0) == good_high { theme::GOOD } else if v == 1.0 { theme::TEXT } else { theme::BAD };
            Style::default().fg(if selected { c } else { theme::dim(c, 0.2) }).bg(bg)
        };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!("{} ", if selected { glyphs::PLAY } else { ' ' }), base),
            Span::styled(format!("{} ", s.glyph()), Style::default().fg(s.color()).bg(bg)),
            Span::styled(format!("{:<8}", s.name()), base),
            Span::styled(format!("{:>6.1}x  ", regrow), mods(regrow, true)),
            Span::styled(format!("{:>10.1}x  ", evap), mods(evap, false)),
            Span::styled(format!("{:>10.1}x  ", metab), mods(metab, false)),
            Span::styled(format!(" {:<8}", forage), base),
        ]));
        row += 1;
    }
    row += 1;

    panel::section(f, inner, row, "Forecast");
    row += 1;
    let water = &fx.series.water;
    let n = water.len();
    let w30 = (water[n - 1] - water[n - 31]) / water[n - 31] * 100.0;
    util::line(f, inner, row, Line::from(vec![
        sp(" drought risk   ", theme::TEXT),
        bold("moderate", theme::WARN),
        sp(format!("   water {:.0}%, {} {:+.0}% in 30 days", water[n - 1] * 100.0, if w30 < 0.0 { glyphs::DOWN } else { glyphs::UP }, w30), theme::DIM),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" frost          ", theme::TEXT),
        bold(format!("in {} days", left), theme::INFO),
        sp("   regrowth halves, shallows freeze", theme::DIM),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" forage line    ", theme::TEXT),
        bold("0.25", theme::ACCENT),
        sp("   regions below it start migrations", theme::DIM),
    ]));
    row += 1;
    let strained = fx.world.regions.iter().filter(|r| status_of(&region_stats(fx, r)).0 != "Plenty" && status_of(&region_stats(fx, r)).0 != "Stable").count();
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} ", glyphs::ALERT), theme::WARN),
        sp(format!("{} regions strained or scarce (see table below)", strained), theme::WARN),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} ", glyphs::DROUGHT), theme::WARN),
        sp("Ashen Ridge: 3 water cells dried up this season", theme::WARN),
    ]));
    row += 2;
    panel::section(f, inner, row, "Day");
    row += 1;
    // Hour strip: 24 cells, night dim, current hour marked.
    let mut spans = vec![sp(" 00 ", theme::DIM)];
    for h in 0..24u32 {
        let night = !(6..18).contains(&h);
        let color = if night { theme::DEEP_WATER_FG } else { theme::ACCENT };
        let g = if h == clock.hour { glyphs::FULL_BLOCK } else if night { glyphs::SHADE_1 } else { glyphs::SHADE_2 };
        spans.push(Span::styled(g.to_string(), Style::default().fg(if h == clock.hour { theme::TEXT_BRIGHT } else { color }).bg(theme::PANEL_BG)));
    }
    spans.push(sp(format!(" 24   now {:02}:00 {} day", clock.hour, glyphs::SUN), theme::DIM));
    util::line(f, inner, row, Line::from(spans));
    row += 1;
    util::line(f, inner, row, Line::from(sp(" 12h daylight; night halves sense range and doubles rest", theme::DIM)));
}

// ------------------------------------------------------------------ regions

struct RegionStats {
    cells: usize,
    water_pct: f32,
    veg: f32,
    moist: f32,
    prey: usize,
    pred: usize,
    pressure: f32,
}

fn region_stats(fx: &Fixtures, r: &(String, usize, usize, usize, usize)) -> RegionStats {
    let (_, x0, y0, x1, y1) = (&r.0, r.1, r.2, r.3, r.4);
    let world = &fx.world;
    let mut cells = 0usize;
    let mut water = 0usize;
    let (mut veg, mut moist, mut pressure) = (0.0f32, 0.0f32, 0.0f32);
    for y in y0..y1 {
        for x in x0..x1 {
            let c = world.cell(x, y);
            cells += 1;
            if c.terrain.is_water() {
                water += 1;
            }
            veg += c.vegetation;
            moist += c.moisture;
            pressure += c.pred_pressure;
        }
    }
    let n = cells.max(1) as f32;
    let (mut prey, mut pred) = (0, 0);
    for c in fx.creatures.iter().filter(|c| c.alive && c.x >= x0 && c.x < x1 && c.y >= y0 && c.y < y1) {
        match c.species.kind() {
            Kind::Prey => prey += 1,
            Kind::Predator => pred += 1,
        }
    }
    RegionStats { cells, water_pct: water as f32 / n, veg: veg / n, moist: moist / n, prey, pred, pressure: pressure / n }
}

fn status_of(s: &RegionStats) -> (&'static str, Color) {
    let crowded = s.prey as f32 > s.veg * 30.0;
    if s.veg < 0.365 {
        ("Scarce", theme::BAD)
    } else if s.veg < 0.40 || (crowded && s.veg < 0.45) {
        ("Strained", theme::WARN)
    } else if s.veg >= 0.45 && s.prey >= 8 {
        ("Plenty", theme::GOOD)
    } else {
        ("Stable", theme::TEXT)
    }
}

fn regions(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let inner = panel::draw_with_hint(f, area, "Regions", "sorted by name   [r] cycle sort", panel::Kind::Outer);
    let mut row = 0;
    util::line(f, inner, row, Line::from(sp(
        format!("{:<18}{:>6}{:>8}   {:<26}{:<26}{:>5}{:>5}{:>10}   {}", " region", "cells", "water", "  vegetation", "  moisture", "prey", "pred", "pressure", "status"),
        theme::DIM,
    )));
    row += 1;
    let mut totals = RegionStats { cells: 0, water_pct: 0.0, veg: 0.0, moist: 0.0, prey: 0, pred: 0, pressure: 0.0 };
    for (i, r) in fx.world.regions.iter().enumerate() {
        let s = region_stats(fx, r);
        let (label, color) = status_of(&s);
        let selected = i == 1; // Ashen Ridge, the strained one
        let bg = if selected { theme::SELECT_BG } else { theme::PANEL_BG };
        let text = Style::default().fg(if selected { theme::TEXT_BRIGHT } else { theme::TEXT }).bg(bg);
        let dim = Style::default().fg(theme::DIM).bg(bg);
        let y = inner.y + row;
        if selected {
            util::fill(f.buffer_mut(), Rect::new(inner.x, y, inner.width, 1), Style::default().bg(bg));
        }
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x, y, format!("{}{:<17}", if selected { glyphs::PLAY } else { ' ' }, r.0), 18, text);
        buf.set_stringn(inner.x + 18, y, format!("{:>6}", s.cells), 6, text);
        buf.set_stringn(inner.x + 24, y, format!("{:>7.0}%", s.water_pct * 100.0), 8, Style::default().fg(theme::SHALLOW_FG).bg(bg));
        let mut x = inner.x + 35;
        for (v, color, ramp) in [(s.veg, theme::VEGETATION, theme::veg(s.veg)), (s.moist, theme::SHALLOW_FG, theme::water(s.moist))] {
            bars::bar(buf, x, y, 20, v, ramp);
            buf.set_stringn(x + 21, y, format!("{:.2}", v), 4, Style::default().fg(color).bg(bg));
            x += 26;
        }
        buf.set_stringn(inner.x + 87, y, format!("{:>5}", s.prey), 5, Style::default().fg(theme::HARE).bg(bg));
        buf.set_stringn(inner.x + 92, y, format!("{:>5}", s.pred), 5, Style::default().fg(theme::WOLF).bg(bg));
        buf.set_stringn(inner.x + 99, y, format!("{:>4.2} ", s.pressure), 5, text);
        bars::bar(buf, inner.x + 105, y, 8, s.pressure * 1.5, theme::heat(s.pressure * 1.5));
        buf.set_stringn(inner.x + 116, y, format!("{:<9}", label), 9, Style::default().fg(color).bg(bg).add_modifier(Modifier::BOLD));
        if label == "Scarce" || label == "Strained" {
            let note = if label == "Scarce" { "forage line, crowded" } else if s.veg < 0.40 { "vegetation thinning" } else { "prey outpacing forage" };
            buf.set_stringn(inner.x + 125, y, format!("{} {}", glyphs::ALERT, note), 26, Style::default().fg(color).bg(bg));
        } else {
            let note = if s.pred == 0 { "no predators seen" } else { "" };
            buf.set_stringn(inner.x + 125, y, note, 26, dim);
        }
        totals.cells += s.cells;
        totals.water_pct += s.water_pct * s.cells as f32;
        totals.veg += s.veg * s.cells as f32;
        totals.moist += s.moist * s.cells as f32;
        totals.prey += s.prey;
        totals.pred += s.pred;
        totals.pressure += s.pressure * s.cells as f32;
        row += 1;
    }
    let n = totals.cells.max(1) as f32;
    row += 1;
    panel::section(f, inner, row, "Whole valley");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {:<17}{:>6}{:>7.0}%   ", "all regions", totals.cells, totals.water_pct / n * 100.0), theme::TEXT),
        sp(format!("vegetation {:.2}   moisture {:.2}   ", totals.veg / n, totals.moist / n), theme::TEXT),
        bold(format!("{} prey", totals.prey), theme::HARE),
        sp("   ", theme::TEXT),
        bold(format!("{} predators", totals.pred), theme::WOLF),
        sp(format!("   mean pressure {:.2}   ratio {:.1}:1   (map sample)", totals.pressure / n, totals.prey as f32 / totals.pred.max(1) as f32), theme::DIM),
    ]));
    row += 1;
    let scarce: Vec<&str> = fx.world.regions.iter().filter(|r| status_of(&region_stats(fx, r)).0 == "Scarce").map(|r| r.0.as_str()).collect();
    let best = fx.world.regions.iter().max_by(|a, b| region_stats(fx, a).veg.partial_cmp(&region_stats(fx, b).veg).unwrap()).map(|r| r.0.as_str()).unwrap_or("-");
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} ", glyphs::MIGRATION), theme::ACCENT),
        sp("migration pressure: ", theme::TEXT),
        bold(if scarce.is_empty() { "none".to_string() } else { scarce.join(", ") }, theme::BAD),
        sp(format!("  {}  richest forage: ", glyphs::MIGRATION), theme::DIM),
        bold(best, theme::GOOD),
    ]));
    row += 2;
    util::line(f, inner, row, Line::from(vec![
        sp(" status: ", theme::DIM),
        bold("Plenty", theme::GOOD),
        sp(" veg >= .45 & prey >= 8   ", theme::DIM),
        bold("Stable", theme::TEXT),
        sp(" veg >= .40   ", theme::DIM),
        bold("Strained", theme::WARN),
        sp(" veg < .40 or prey > 30 x veg   ", theme::DIM),
        bold("Scarce", theme::BAD),
        sp(" veg < .37", theme::DIM),
    ]));
}
