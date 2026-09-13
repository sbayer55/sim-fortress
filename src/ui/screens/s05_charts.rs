//! S05: the live charts screen — S05a prey/predator lines, S05b phase plot,
//! S05c stacked species over vegetation (C4 FR9) and S05d infections (C7 FR13).

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::Frame;

use crate::sim::{Outbreak, PathogenId};
use crate::sim::{Kind, Sample, Sim, SpeciesId};
use crate::ui::app::AppState;
use crate::ui::screens::common::{arrow_color, day_stamp, sp, trend_arrow};
use crate::ui::screens::{Action, Screen};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

const CHART_W: u16 = 112;

#[derive(Clone, Copy, PartialEq, Eq)]
#[derive(Debug)]
pub enum Variant {
    Time,
    Phase,
    Stacked,
    Infections,
}

#[derive(Debug)]
pub struct Charts {
    variant: Variant,
    zoom: usize,
}

impl Default for Charts {
    fn default() -> Self {
        Self::new()
    }
}

impl Charts {
    pub const fn new() -> Self {
        Self { variant: Variant::Time, zoom: 240 }
    }
}

impl Screen for Charts {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, _app: &mut AppState) -> Action {
        match key.code {
            KeyCode::Char('g') => {
                self.variant = match self.variant {
                    Variant::Time => Variant::Phase,
                    Variant::Phase => Variant::Stacked,
                    Variant::Stacked => Variant::Infections,
                    Variant::Infections => Variant::Time,
                };
                Action::None
            }
            KeyCode::Char('1') => {
                self.variant = Variant::Time;
                Action::None
            }
            KeyCode::Char('2') => {
                self.variant = Variant::Phase;
                Action::None
            }
            KeyCode::Char('3') => {
                self.variant = Variant::Stacked;
                Action::None
            }
            KeyCode::Char('4') => {
                self.variant = Variant::Infections;
                Action::None
            }
            KeyCode::Char('+') => {
                self.zoom = match self.zoom {
                    60 => 240,
                    _ => 720,
                };
                Action::None
            }
            KeyCode::Char('-') => {
                self.zoom = match self.zoom {
                    720 => 240,
                    _ => 60,
                };
                Action::None
            }
            KeyCode::Esc => Action::Pop,
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else {
            return;
        };
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let chart_w = CHART_W.min(area.width.saturating_sub(20));
        let chart_area = Rect::new(area.x, area.y, chart_w, body_h);
        let side_area = Rect::new(area.x + chart_w, area.y, area.width - chart_w, body_h);
        let window = Window::new(sim, self.zoom);

        match self.variant {
            Variant::Time => {
                time_chart(f, chart_area, sim, &window);
                time_sidebar(f, side_area, sim, &window);
            }
            Variant::Phase => {
                phase_chart(f, chart_area, sim, &window);
                phase_sidebar(f, side_area, sim, &window);
            }
            Variant::Stacked => {
                stacked_chart(f, chart_area, sim, &window);
                stacked_sidebar(f, side_area, sim, &window);
            }
            Variant::Infections => {
                infection_chart(f, chart_area, sim, &window);
                infection_sidebar(f, side_area, sim, &window);
            }
        }

        let view = match self.variant {
            Variant::Time => "chart 1/4  populations",
            Variant::Phase => "chart 2/4  phase plot",
            Variant::Stacked => "chart 3/4  stacked species",
            Variant::Infections => "chart 4/4  infections",
        };
        status::render(f, Rect::new(area.x, status_row, area.width, 1), &[("g", "next chart"), ("1-4", "pick"), ("+/-", "zoom"), ("Esc", "back")], view);
    }
}

/// The visible slice of the daily series.
struct Window<'a> {
    samples: &'a [Sample],
    season_days: u32,
    prey: Vec<f32>,
    pred: Vec<f32>,
    veg: Vec<f32>,
    drought: Vec<bool>,
    /// Days inside an epidemic window (C7): any outbreak flagged `epidemic`
    /// whose `started_day..=ended_day` (or today) covers the sample day.
    epidemic: Vec<bool>,
}

impl<'a> Window<'a> {
    fn new(sim: &'a Sim, zoom: usize) -> Self {
        let all = sim.series.samples();
        let start = all.len().saturating_sub(zoom);
        let samples = &all[start..];
        let today = crate::cast!(sim.time.day_index() => u32);
        let epidemics: Vec<(u32, u32)> = sim.disease.outbreaks.iter().filter(|o| o.epidemic).map(|o| (o.started_day, o.ended_day.unwrap_or(today))).collect();
        Window {
            samples,
            season_days: sim.time.season_days,
            prey: samples.iter().map(|s| crate::cast!((s.population[0] + s.population[1] + s.population[2]) => f32)).collect(),
            pred: samples.iter().map(|s| crate::cast!((s.population[3] + s.population[4] + s.population[5]) => f32)).collect(),
            veg: samples.iter().map(|s| s.veg_mean).collect(),
            drought: samples.iter().map(|s| s.drought_flags.iter().any(|&d| d)).collect(),
            epidemic: samples.iter().map(|s| epidemics.iter().any(|&(a, b)| s.day >= a && s.day <= b)).collect(),
        }
    }

    const fn len(&self) -> usize {
        self.samples.len()
    }

    fn day_label(&self, i: usize) -> String {
        match self.samples.get(i) {
            Some(s) => day_stamp(i64::from(s.day), self.season_days),
            None => String::new(),
        }
    }

    /// Column → sample index range for `cols` columns.
    fn bin(&self, c: usize, cols: usize) -> (usize, usize) {
        let n = self.len();
        let a = (c * n).div_euclid(cols);
        let b = (((c + 1) * n).div_euclid(cols)).max(a + 1).min(n);
        (a, b)
    }

    fn mean_over(&self, v: &[f32], c: usize, cols: usize) -> f32 {
        if self.len() == 0 {
            return 0.0;
        }
        let (a, b) = self.bin(c, cols);
        v[a..b].iter().sum::<f32>() / crate::cast!((b - a) => f32)
    }

    fn drought_in(&self, c: usize, cols: usize) -> bool {
        if self.len() == 0 {
            return false;
        }
        let (a, b) = self.bin(c, cols);
        self.drought[a..b].iter().any(|&d| d)
    }

    fn epidemic_in(&self, c: usize, cols: usize) -> bool {
        if self.len() == 0 {
            return false;
        }
        let (a, b) = self.bin(c, cols);
        self.epidemic[a..b].iter().any(|&d| d)
    }
}

fn round_up(v: f32, step: f32) -> f32 {
    ((v / step).ceil() * step).max(step)
}

// ------------------------------------------------------------------ S05a

fn time_chart(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw_with_hint(f, area, "Population — prey vs predators", &format!("last {} days, 1 point per day", w.len()), panel::Kind::Outer);
    let half = (inner.height.saturating_sub(1)).div_euclid(2);
    let upper = Rect::new(inner.x, inner.y, inner.width, half);
    let lower = Rect::new(inner.x, inner.y + half + 1, inner.width, inner.height - half - 1);
    util::line(f, upper, 0, Line::from(sp(" Prey (voles + hares + deer)", Style::default().fg(theme::HARE).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))));
    line_chart(f, Rect::new(upper.x, upper.y + 1, upper.width, upper.height - 1), w, &w.prey, theme::HARE, 100.0);
    panel::section(f, inner, half, "Predators (foxes + wolves + lynx)");
    let label = if w.pred.iter().all(|&v| v <= 0.0) { " no predators yet" } else { " " };
    util::line(f, lower, 0, Line::from(sp(label, theme::dim_text())));
    line_chart(f, Rect::new(lower.x, lower.y + 1, lower.width, lower.height - 1), w, &w.pred, theme::WOLF, 20.0);
    let _ = sim;
}

/// One series as a half-block line with a y axis on the left, x labels below.
fn line_chart(f: &mut Frame<'_>, area: Rect, w: &Window<'_>, series: &[f32], color: Color, step: f32) {
    if area.height < 4 || area.width < 12 {
        return;
    }
    let label_w = 6u16;
    let plot = Rect::new(area.x + label_w, area.y, area.width - label_w - 1, area.height - 2);
    let axis_y = plot.bottom();
    let cols = crate::cast!(plot.width => usize);
    let rows = crate::cast!(plot.height => usize);
    let max = series.iter().copied().fold(0.0f32, f32::max);
    let y_max = round_up(max, step);
    let buf = f.buffer_mut();
    let band = theme::lerp(theme::PANEL_BG, theme::WARN, 0.22);
    plot_columns(buf, plot, w, series, color, y_max, cols, rows, band);
    draw_axes(buf, area, plot, axis_y, w, y_max, cols, rows);
}/// Draw the series columns, including the drought band and its label.
#[allow(clippy::too_many_arguments)]
fn plot_columns(buf: &mut Buffer, plot: Rect, w: &Window<'_>, series: &[f32], color: Color, y_max: f32, cols: usize, rows: usize, band: Color) {
    let mut band_started: Option<u16> = None;
    for c in 0..cols {
        let x = plot.x + crate::cast!(c => u16);
        let dry = w.len() > 0 && w.drought_in(c, cols);
        if dry {
            for r in 0..rows {
                if let Some(cell) = buf.cell_mut((x, plot.y + crate::cast!(r => u16))) {
                    cell.set_char(' ');
                    cell.set_style(Style::default().bg(band));
                }
            }
            if band_started.is_none() {
                band_started = Some(x);
            }
        }
        if w.len() == 0 {
            continue;
        }
        let v = w.mean_over(series, c, cols);
        let halves = crate::cast!(((v / y_max) * (crate::cast!(rows => f32) * 2.0)).round() => usize);
        if halves == 0 {
            // Flat zero: a lower half-block on the bottom row.
            if let Some(cell) = buf.cell_mut((x, plot.y + crate::cast!(rows => u16) - 1)) {
                cell.set_char(glyphs::HALF_LOWER);
                cell.set_style(Style::default().fg(color).bg(cell.bg));
            }
            continue;
        }
        let r = rows - 1 - ((halves - 1).div_euclid(2)).min(rows - 1);
        let ch = if halves % 2 == 1 { glyphs::HALF_LOWER } else { glyphs::HALF_UPPER };
        if let Some(cell) = buf.cell_mut((x, plot.y + crate::cast!(r => u16))) {
            cell.set_char(ch);
            cell.set_style(Style::default().fg(color).bg(cell.bg).add_modifier(Modifier::BOLD));
        }
    }
    if let Some(bx) = band_started {
        buf.set_stringn(bx + 1, plot.y, format!("{} drought", glyphs::DROUGHT), 10, Style::default().fg(theme::WARN).bg(band));
    }
}

/// Draw the Y labels, the box lines and the X axis with seven labels.
#[allow(clippy::too_many_arguments)]
fn draw_axes(buf: &mut Buffer, area: Rect, plot: Rect, axis_y: u16, w: &Window<'_>, y_max: f32, cols: usize, rows: usize) {
    // Y labels: 0, ¼, ½, ¾, max.
    for k in 0..=4 {
        let y = axis_y - 1 - crate::cast!((crate::cast!((rows - 1) => f32) * crate::cast!(k => f32) / 4.0).round() => u16);
        let v = crate::cast!((y_max * crate::cast!(k => f32) / 4.0).round() => u32);
        buf.set_stringn(area.x, y, format!("{v:>5}"), 5, theme::dim_text());
        buf.set_stringn(plot.x - 1, y, glyphs::CROSS.to_string(), 1, theme::border());
    }
    for r in 0..rows {
        let y = plot.y + crate::cast!(r => u16);
        if buf.cell((plot.x - 1, y)).is_some_and(|c| c.symbol() != "┼") {
            buf.set_stringn(plot.x - 1, y, glyphs::V_LINE.to_string(), 1, theme::border());
        }
    }
    // X axis with seven labels.
    let axis: String = std::iter::repeat_n(glyphs::H_LINE, cols + 1).collect();
    buf.set_stringn(plot.x - 1, axis_y, &axis, cols + 1, theme::border());
    let n = w.len().max(1);
    for k in 0..=6 {
        let i = ((k * (n - 1)).div_euclid(6)).min(n - 1);
        let x = if k == 6 { plot.x + crate::cast!(cols => u16) - 1 } else { plot.x + crate::cast!(((i * cols).div_euclid(n)) => u16) };
        buf.set_stringn(x, axis_y, glyphs::CROSS.to_string(), 1, theme::border());
        let label = w.day_label(i);
        if !label.is_empty() {
            let lx = crate::cast!((i32::from(x) - crate::cast!(label.len() => i32).div_euclid(2)).max(i32::from(area.x)) => u16);
            let lx = lx.min(area.right().saturating_sub(crate::cast!(label.len() => u16) + 1));
            buf.set_stringn(lx, axis_y + 1, &label, label.len(), theme::dim_text());
        }
    }
    buf.set_stringn(plot.right() - 3, axis_y, "now", 3, Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG));
}

fn stats_of(series: &[f32]) -> Option<(f32, usize, f32, usize, f32)> {
    if series.is_empty() {
        return None;
    }
    let (mut min, mut imin, mut max, mut imax) = (f32::MAX, 0, f32::MIN, 0);
    for (i, &v) in series.iter().enumerate() {
        if v < min {
            min = v;
            imin = i;
        }
        if v > max {
            max = v;
            imax = i;
        }
    }
    let mean = series.iter().sum::<f32>() / crate::cast!(series.len() => f32);
    Some((min, imin, max, imax, mean))
}

fn pct_change(series: &[f32], days: usize) -> Option<f32> {
    if series.len() < 2 {
        return None;
    }
    let a = series[series.len().saturating_sub(days).min(series.len() - 1)];
    let b = series.last().copied().unwrap_or(0.0);
    if a <= 0.0 {
        return None;
    }
    Some((b - a) / a * 100.0)
}

fn time_sidebar(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw(f, area, "Statistics", panel::Kind::Outer);
    let mut row = 0u16;
    panel::section(f, inner, row, "Current");
    row += 1;
    let prey_now = sim.species.iter().filter(|s| s.species.kind() == Kind::Prey).map(|s| s.count).sum::<u32>();
    let counts: Vec<u16> = w.prey.iter().map(|v| crate::cast!(*v => u16)).collect();
    let arrow = trend_arrow(&counts);
    let pct = pct_change(&w.prey, 30).map_or_else(|| "–".into(), |p| format!("{p:+.0}%"));
    util::line(f, inner, row, Line::from(vec![
        sp(" prey       ", theme::dim_text()),
        sp(format!("{prey_now:>5}"), Style::default().fg(theme::HARE).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(format!("  {arrow} {pct} / 30d"), Style::default().fg(arrow_color(arrow)).bg(theme::PANEL_BG)),
    ]));
    row += 1;
    let pred_now = sim.species.iter().filter(|s| s.species.kind() == Kind::Predator).map(|s| s.count).sum::<u32>();
    let pred_counts: Vec<u16> = w.pred.iter().map(|v| crate::cast!(*v => u16)).collect();
    let parrow = trend_arrow(&pred_counts);
    let ppct = pct_change(&w.pred, 30).map_or_else(|| "–".into(), |p| format!("{p:+.0}%"));
    util::line(f, inner, row, Line::from(vec![
        sp(" predators  ", theme::dim_text()),
        sp(format!("{pred_now:>5}"), Style::default().fg(theme::WOLF).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(format!("  {parrow} {ppct} / 30d"), Style::default().fg(arrow_color(parrow)).bg(theme::PANEL_BG)),
    ]));
    row += 1;
    let ratio = if pred_now > 0 { format!("{:.1} prey per predator", crate::cast!(prey_now => f32) / crate::cast!(pred_now => f32)) } else { "–  (no predators)".to_string() };
    util::line(f, inner, row, Line::from(sp(format!(" ratio {ratio}"), theme::dim_text())));
    row += 2;

    row = side_stats(f, inner, row, "Prey", &w.prey, w, "  (max − min) ÷ mean", " no history yet");
    row = side_stats(f, inner, row, "Predators", &w.pred, w, "", " no predators yet");

    row = drought_section(f, inner, row, w);

    row = map_census(f, inner, row, sim);

    panel::section(f, inner, row, "Legend");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {}{} ", glyphs::HALF_UPPER, glyphs::HALF_LOWER), Style::default().fg(theme::HARE).bg(theme::PANEL_BG)),
        sp("prey (voles + hares + deer)", theme::text()),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {}{} ", glyphs::HALF_UPPER, glyphs::HALF_LOWER), Style::default().fg(theme::WOLF).bg(theme::PANEL_BG)),
        sp("predators (none yet)", theme::text()),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" ░ ", Style::default().fg(theme::WARN).bg(theme::PANEL_BG)),
        sp("drought band (any region in drought)", theme::text()),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(sp(" separate y scales: prey per 100, predators per 20", theme::dim_text())));
    row += 2;
    panel::section(f, inner, row, "Keys");
    row += 1;
    util::line(f, inner, row, Line::from(sp(" [+/-] zoom 60/240/720d", theme::dim_text())));
}

// ------------------------------------------------------------------ S05b

/// One min/max/mean/swing block for a series.
#[allow(clippy::too_many_arguments)]
fn side_stats(f: &mut Frame<'_>, inner: Rect, mut row: u16, title: &str, series: &[f32], w: &Window<'_>, swing_note: &str, empty: &str) -> u16 {
    panel::section(f, inner, row, title);
    row += 1;
    if let Some((min, imin, max, imax, mean)) = stats_of(series) {
        let swing = if mean > 0.0 { (max - min) / mean * 100.0 } else { 0.0 };
        for (k, v) in [
            ("min", format!("{:.0}  on {}", min, w.day_label(imin))),
            ("max", format!("{:.0}  on {}", max, w.day_label(imax))),
            ("mean", format!("{mean:.0}")),
            ("swing", format!("{swing:.0}%{swing_note}")),
        ] {
            util::line(f, inner, row, Line::from(vec![sp(format!(" {k:<7}"), theme::dim_text()), sp(v, theme::text())]));
            row += 1;
        }
    } else {
        util::line(f, inner, row, Line::from(sp(empty, theme::dim_text())));
        row += 1;
    }
    row += 1;
    row
}

/// The drought summary, shown only when the window contains drought days.
fn drought_section(f: &mut Frame<'_>, inner: Rect, mut row: u16, w: &Window<'_>) -> u16 {
    let dry_days = w.drought.iter().filter(|&&d| d).count();
    if dry_days > 0 {
        panel::section(f, inner, row, "Drought");
        row += 1;
        let first = w.drought.iter().position(|&d| d).unwrap_or(0);
        let last = w.drought.iter().rposition(|&d| d).unwrap_or(0);
        let veg_before = w.veg.get(first.saturating_sub(1)).copied().unwrap_or(0.0);
        let veg_min = w.veg[first..=last].iter().copied().fold(1.0f32, f32::min);
        let water = w.samples.get(last).map_or(1.0, |s| s.water_level);
        util::line(f, inner, row, Line::from(sp(format!(" {} {} drought days: {} to {}", glyphs::DROUGHT, dry_days, w.day_label(first), w.day_label(last)), Style::default().fg(theme::WARN).bg(theme::PANEL_BG))));
        row += 1;
        util::line(f, inner, row, Line::from(sp(format!(" vegetation {:.0}% {} {:.0}%, water x{:.2}", veg_before * 100.0, glyphs::DOWN, veg_min * 100.0, water), theme::text())));
        row += 1;
        let p0 = w.prey.get(first).copied().unwrap_or(0.0);
        let p1 = w.prey.get(last).copied().unwrap_or(0.0);
        util::line(f, inner, row, Line::from(sp(format!(" prey {p0:.0} at the start, {p1:.0} at the end"), theme::text())));
        row += 2;
    }
    row
}

/// The per-species census rows.
fn map_census(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim) -> u16 {
    panel::section(f, inner, row, "Map census");
    row += 1;
    for chunk in SpeciesId::ALL.chunks(3) {
        let mut spans = Vec::new();
        for id in chunk {
            let s = &sim.species[id.index()];
            spans.push(sp(format!(" {} ", id.glyph().to_ascii_uppercase()), Style::default().fg(if s.count == 0 { theme::DIM } else { id.color() }).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)));
            spans.push(sp(format!("{:<5}{:>4}  ", id.name(), s.count), if s.count == 0 { theme::dim_text() } else { theme::text() }));
        }
        util::line(f, inner, row, Line::from(spans));
        row += 1;
    }
    row += 1;
    row
}

fn phase_chart(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
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
    buf.set_stringn(inner.x, py, "pred", 4, Style::default().fg(theme::WOLF).bg(theme::PANEL_BG));
    buf.set_stringn(px, py + ph + 1, "prey total", 10, Style::default().fg(theme::HARE).bg(theme::PANEL_BG));

    // Quadrant captions.
    buf.set_stringn(px + 1, py + 1, "II few prey, many predators", 27, theme::dim_text());
    buf.set_stringn(px + pw.saturating_sub(26), py + 1, "I many prey, many predators", 26, theme::dim_text());
    buf.set_stringn(px + 1, py + ph.saturating_sub(2), "III few of both", 15, theme::dim_text());
    buf.set_stringn(px + pw.saturating_sub(24), py + ph.saturating_sub(2), "IV many prey, few predators", 26, theme::dim_text());
}

fn phase_sidebar(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw(f, area, "Phase", panel::Kind::Outer);
    let mut row = 0u16;
    let prey_now = sim.species.iter().filter(|s| s.species.kind() == Kind::Prey).map(|s| s.count).sum::<u32>();
    let pred_now = sim.species.iter().filter(|s| s.species.kind() == Kind::Predator).map(|s| s.count).sum::<u32>();
    panel::section(f, inner, row, "Now");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" prey      ", theme::dim_text()),
        sp(format!("{prey_now:>5}"), Style::default().fg(theme::HARE).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp("  predators  ", theme::dim_text()),
        sp(format!("{pred_now:>4}"), Style::default().fg(theme::WOLF).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
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
        ("I", "many prey, many predators", theme::WOLF),
        ("II", "few prey, many predators", theme::WARN),
        ("III", "few of both", theme::HARE),
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

// ------------------------------------------------------------------ S05c

const Y_LABEL_W: u16 = 6;
const R_AXIS_W: u16 = 6;

fn shade_empty(buf: &mut Buffer, x: u16, y0: u16, y1: u16, band: Color) {
    for y in y0..y1 {
        if let Some(cell) = buf.cell_mut((x, y)) {
            if cell.symbol() == " " {
                cell.set_style(Style::default().bg(band));
            }
        }
    }
}

fn stacked_chart(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw_with_hint(f, area, "Stacked populations + vegetation", &format!("{} days, three prey species", w.len()), panel::Kind::Outer);
    let _ = sim;
    // Legend row.
    let mut spans = vec![sp(" ", theme::text())];
    for id in SpeciesId::ALL.iter().take(3) {
        spans.push(sp(glyphs::FULL_BLOCK.to_string(), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)));
        spans.push(sp(format!(" {}   ", id.plural()), theme::text()));
    }
    spans.push(sp(glyphs::DOT.to_string(), Style::default().fg(theme::VEGETATION).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)));
    spans.push(sp(" vegetation biomass (right axis, % of max)", theme::text()));
    util::line(f, inner, 0, Line::from(spans));
    if inner.height < 12 {
        return;
    }

    let plot_x = inner.x + Y_LABEL_W;
    let plot_w = inner.width - Y_LABEL_W - R_AXIS_W;
    let plot_y = inner.y + 2;
    let plot_h = inner.height - 7;
    let axis_y = plot_y + plot_h;
    let cols = crate::cast!(plot_w => usize);
    let n = w.len();

    let (stack, veg) = stacked_bins(w, cols);
    let max_total = stack.iter().map(|v| v.iter().sum::<f32>()).fold(0.0f32, f32::max);
    let y_max = round_up(max_total, 50.0);
    let halves = crate::cast!(plot_h => usize) * 2;
    let band = theme::lerp(theme::PANEL_BG, theme::WARN, 0.22);
    let buf = f.buffer_mut();
    let band_x = stacked_columns(buf, w, plot_x, plot_y, plot_h, cols, y_max, halves, band, n, &stack, &veg);
    if let Some(bx) = band_x {
        shade_empty(buf, bx, plot_y, axis_y, band);
        buf.set_stringn(bx + 1, plot_y, format!("{} drought", glyphs::DROUGHT), 12, Style::default().fg(theme::WARN).bg(band));
    }
    for r in 0..plot_h {
        let y = plot_y + r;
        buf.set_stringn(plot_x - 1, y, glyphs::V_LINE.to_string(), 1, theme::border());
        buf.set_stringn(plot_x + plot_w, y, glyphs::V_LINE.to_string(), 1, theme::border());
    }
    for k in 0..=5 {
        let y = axis_y - 1 - crate::cast!((f32::from(plot_h - 1) * crate::cast!(k => f32) / 5.0).round() => u16);
        let count = crate::cast!((y_max * crate::cast!(k => f32) / 5.0).round() => u32);
        buf.set_stringn(inner.x, y, format!("{count:>5}"), 5, theme::dim_text());
        buf.set_stringn(plot_x - 1, y, glyphs::CROSS.to_string(), 1, theme::border());
        buf.set_stringn(plot_x + plot_w, y, format!("{}{:>4}%", glyphs::CROSS, k * 20), crate::cast!(R_AXIS_W => usize), Style::default().fg(theme::VEGETATION).bg(theme::PANEL_BG));
    }
    let axis: String = std::iter::repeat_n(glyphs::H_LINE, crate::cast!(plot_w => usize) + 1).collect();
    buf.set_stringn(plot_x - 1, axis_y, &axis, crate::cast!(plot_w => usize) + 1, theme::border());
    if n > 0 {
        for k in 0..=6 {
            let i = ((k * (n - 1)).div_euclid(6)).min(n - 1);
            let x = if k == 6 { plot_x + plot_w - 1 } else { plot_x + crate::cast!(((i * cols).div_euclid(n)) => u16) };
            buf.set_stringn(x, axis_y, glyphs::CROSS.to_string(), 1, theme::border());
            let label = w.day_label(i);
            let lx = crate::cast!((i32::from(x) - crate::cast!(label.len() => i32).div_euclid(2)).max(i32::from(inner.x)) => u16);
            let lx = lx.min(inner.right().saturating_sub(crate::cast!(label.len() => u16)));
            buf.set_stringn(lx, axis_y + 1, &label, label.len(), theme::dim_text());
        }
    }
    let note_y = axis_y + 3;
    let note = theme::dim_text();
    let per_col = crate::cast!(n => f32) / crate::cast!(cols.max(1) => f32);
    buf.set_stringn(inner.x + 1, note_y, format!("Species are stacked bottom-up in table order (voles, hares, deer); each column averages ~{per_col:.1} days."), crate::cast!(inner.width => usize) - 2, note);
    buf.set_stringn(inner.x + 1, note_y + 1, "The vegetation dot uses the right-hand scale; the prey stack follows the grass with a lag.", crate::cast!(inner.width => usize) - 2, note);
}

/// Per-column stacked prey populations and vegetation means.
fn stacked_bins(w: &Window<'_>, cols: usize) -> (Vec<[f32; 3]>, Vec<f32>) {
    let n = w.len();
    let mut stack: Vec<[f32; 3]> = Vec::with_capacity(cols);
    let mut veg: Vec<f32> = Vec::with_capacity(cols);
    for c in 0..cols {
        let mut v = [0.0f32; 3];
        if n > 0 {
            let (a, b) = w.bin(c, cols);
            for (k, item) in v.iter_mut().enumerate() {
                *item = w.samples[a..b].iter().map(|s| crate::cast!(s.population[k] => f32)).sum::<f32>() / crate::cast!((b - a) => f32);
            }
            veg.push(w.mean_over(&w.veg, c, cols));
        } else {
            veg.push(0.0);
        }
        stack.push(v);
    }
    (stack, veg)
}

/// Draw the stacked columns, the vegetation dots and the drought banding.
/// Returns the x of the first drought column, if any.
#[allow(clippy::too_many_arguments)]
fn stacked_columns(buf: &mut Buffer, w: &Window<'_>, plot_x: u16, plot_y: u16, plot_h: u16, cols: usize, y_max: f32, halves: usize, band: Color, n: usize, stack: &[[f32; 3]], veg: &[f32]) -> Option<u16> {
    let mut band_x: Option<u16> = None;
    for (c, v) in stack.iter().enumerate() {
        let x = plot_x + crate::cast!(c => u16);
        let mut lower: Vec<Option<Color>> = vec![None; crate::cast!(plot_h => usize)];
        let mut upper: Vec<Option<Color>> = vec![None; crate::cast!(plot_h => usize)];
        let mut cum = 0.0f32;
        for (k, id) in SpeciesId::ALL.iter().take(3).enumerate() {
            let h0 = crate::cast!((cum / y_max * crate::cast!(halves => f32)).round() => usize);
            cum += v[k];
            let h1 = crate::cast!((cum / y_max * crate::cast!(halves => f32)).round() => usize);
            for h in h0..h1.min(halves) {
                let row = crate::cast!(plot_h => usize) - 1 - h.div_euclid(2);
                if h % 2 == 0 {
                    lower[row] = Some(id.color());
                } else {
                    upper[row] = Some(id.color());
                }
            }
        }
        let dry = n > 0 && w.drought_in(c, cols);
        if dry && band_x.is_none() {
            band_x = Some(x);
        }
        let empty_bg = if dry { band } else { theme::PANEL_BG };
        for r in 0..crate::cast!(plot_h => usize) {
            let y = plot_y + crate::cast!(r => u16);
            let Some(cell) = buf.cell_mut((x, y)) else { continue };
            match (lower[r], upper[r]) {
                (Some(lo), Some(up)) if lo == up => {
                    cell.set_char(glyphs::FULL_BLOCK);
                    cell.set_style(Style::default().fg(lo).bg(lo));
                }
                (Some(lo), Some(up)) => {
                    cell.set_char(glyphs::HALF_LOWER);
                    cell.set_style(Style::default().fg(lo).bg(up));
                }
                (Some(lo), None) => {
                    cell.set_char(glyphs::HALF_LOWER);
                    cell.set_style(Style::default().fg(lo).bg(empty_bg));
                }
                (None, Some(up)) => {
                    cell.set_char(glyphs::HALF_UPPER);
                    cell.set_style(Style::default().fg(up).bg(empty_bg));
                }
                (None, None) => {
                    cell.set_char(' ');
                    cell.set_style(Style::default().bg(empty_bg));
                }
            }
        }
        if n > 0 {
            let vh = crate::cast!((veg[c].clamp(0.0, 1.0) * (f32::from(plot_h) - 1.0)).round() => usize);
            let r = crate::cast!(plot_h => usize) - 1 - vh;
            if let Some(cell) = buf.cell_mut((x, plot_y + crate::cast!(r => u16))) {
                let bg = match (lower[r], upper[r]) {
                    (Some(lo), _) => lo,
                    (None, Some(up)) => up,
                    _ => empty_bg,
                };
                cell.set_char(glyphs::DOT);
                cell.set_style(Style::default().fg(theme::VEGETATION).bg(bg).add_modifier(Modifier::BOLD));
            }
        }
    }
    band_x
}

fn stacked_sidebar(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw(f, area, "Composition", panel::Kind::Outer);
    let mut row = 0u16;
    row = stacked_today(f, inner, row, sim, w);
    row = stacked_peaks(f, inner, row, w);
    row = stacked_trend(f, inner, row, sim);

    panel::section(f, inner, row, "How to read");
    row += 1;
    for line in [
        " the top edge is the whole prey population",
        " each band is one species, voles at the bottom",
        " the green dot is mean vegetation (right axis)",
        " a shaded band marks days with a drought",
        " [+/-] widens or narrows the time window",
    ] {
        util::line(f, inner, row, Line::from(sp(line, theme::dim_text())));
        row += 1;
    }
}

/// Today's per-species counts and the share bar.
fn stacked_today(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim, w: &Window<'_>) -> u16 {
    panel::section(f, inner, row, "Today");
    row += 1;
    util::line(f, inner, row, Line::from(sp("   species   count  share   240d range", theme::dim_text())));
    row += 1;
    let total: u32 = sim.species.iter().take(3).map(|s| s.count).sum();
    for (k, id) in SpeciesId::ALL.iter().take(3).enumerate() {
        let s = &sim.species[k];
        let share = if total > 0 { crate::cast!(s.count => f32) / crate::cast!(total => f32) * 100.0 } else { 0.0 };
        let series: Vec<u32> = w.samples.iter().map(|x| x.population[k]).collect();
        let (lo, hi) = (series.iter().min().copied().unwrap_or(0), series.iter().max().copied().unwrap_or(0));
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", glyphs::FULL_BLOCK), Style::default().fg(id.color()).bg(theme::PANEL_BG)),
            sp(format!("{:<8}{:>6}  {:>4.0}%   {}-{}", id.plural(), s.count, share, lo, hi), theme::text()),
        ]));
        row += 1;
    }
    let veg = w.veg.last().copied().unwrap_or(0.0);
    util::line(f, inner, row, Line::from(sp(format!("   total   {:>6}          veg {:.0}%", total, veg * 100.0), theme::dim_text())));
    row += 2;

    panel::section(f, inner, row, "Share today");
    row += 1;
    {
        let width = crate::cast!(inner.width.saturating_sub(2) => usize);
        let mut used = 0usize;
        let buf = f.buffer_mut();
        let y = inner.y + row;
        for (k, id) in SpeciesId::ALL.iter().take(3).enumerate() {
            let s = &sim.species[k];
            let cells = if k == 2 { width.saturating_sub(used) } else if total > 0 { (crate::cast!(s.count => usize) * width).div_euclid(crate::cast!(total => usize)) } else { 0 };
            let seg: String = std::iter::repeat_n(glyphs::FULL_BLOCK, cells).collect();
            buf.set_stringn(inner.x + 1 + crate::cast!(used => u16), y, &seg, cells, Style::default().fg(id.color()).bg(theme::PANEL_BG));
            used += cells;
        }
    }
    row += 1;
    util::line(f, inner, row, Line::from(sp(" prey 100%   predators 0%", theme::dim_text())));
    row += 2;
    row
}

/// Peak/smallest stacks, vegetation range and the drought summary.
fn stacked_peaks(f: &mut Frame<'_>, inner: Rect, mut row: u16, w: &Window<'_>) -> u16 {
    panel::section(f, inner, row, "Peaks");
    row += 1;
    if let Some((min, imin, max, imax, _)) = stats_of(&w.prey) {
        util::line(f, inner, row, Line::from(sp(format!(" largest stack  {:>5.0}  {}", max, w.day_label(imax)), theme::text())));
        row += 1;
        util::line(f, inner, row, Line::from(sp(format!(" smallest stack {:>5.0}  {}", min, w.day_label(imin)), theme::text())));
        row += 1;
    }
    if let Some((vmin, _, vmax, _, _)) = stats_of(&w.veg) {
        util::line(f, inner, row, Line::from(sp(format!(" vegetation     {:>4.0}% .. {:.0}%", vmin * 100.0, vmax * 100.0), Style::default().fg(theme::VEGETATION).bg(theme::PANEL_BG))));
        row += 1;
    }
    row += 1;

    let dry_days = w.drought.iter().filter(|&&d| d).count();
    if dry_days > 0 {
        panel::section(f, inner, row, "Drought");
        row += 1;
        let first = w.drought.iter().position(|&d| d).unwrap_or(0);
        let last = w.drought.iter().rposition(|&d| d).unwrap_or(0);
        util::line(f, inner, row, Line::from(sp(format!(" {} {} to {} ({} days)", glyphs::DROUGHT, w.day_label(first), w.day_label(last), dry_days), Style::default().fg(theme::WARN).bg(theme::PANEL_BG))));
        row += 1;
        let before = w.prey.get(first.saturating_sub(1)).copied().unwrap_or(0.0);
        let after = w.prey.get((last + 20).min(w.len().saturating_sub(1))).copied().unwrap_or(0.0);
        util::line(f, inner, row, Line::from(sp(format!(" stack {before:.0} before, {after:.0} twenty days after"), theme::text())));
        row += 2;
    }
    row
}

/// The 30-day trend rows.
fn stacked_trend(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim) -> u16 {
    panel::section(f, inner, row, "30-day trend");
    row += 1;
    for (k, id) in SpeciesId::ALL.iter().take(3).enumerate() {
        let s = &sim.species[k];
        let a = s.trend.first().copied().unwrap_or(0);
        let b = s.trend.last().copied().unwrap_or(0);
        let arrow = trend_arrow(&s.trend);
        let pct = s.change_pct().map_or_else(|| "–".into(), |p| format!("{p:+.0}%"));
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", id.glyph().to_ascii_uppercase()), Style::default().fg(id.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{:<7}{:>5} {} {:<5}", id.plural(), a, glyphs::RIGHT, b), theme::text()),
            sp(format!(" {arrow} {pct}"), Style::default().fg(arrow_color(arrow)).bg(theme::PANEL_BG)),
        ]));
        row += 1;
    }
    row += 1;
    row
}

// ------------------------------------------------------------------ S05d

/// Line colours for the eight pathogen slots (roster first, strains appended).
/// Lower-case delta (CP437 0xEB); the capital Δ is not in CP437.
const DELTA: char = 'δ';

const PATHOGEN_COLORS: [Color; 8] = [theme::SICK, theme::WARN, theme::MAGENTA, theme::INFO, theme::ACCENT, theme::LYNX, theme::DEER, theme::HARE];

const fn slot_color(slot: usize) -> Color {
    PATHOGEN_COLORS[slot % PATHOGEN_COLORS.len()]
}

/// Pathogen slots with any active case inside the window, with their series.
fn case_series(w: &Window<'_>) -> Vec<(usize, Vec<f32>)> {
    (0..8)
        .filter_map(|slot| {
            let v: Vec<f32> = w.samples.iter().map(|s| crate::cast!(s.active_by_pathogen[slot] => f32)).collect();
            if v.iter().any(|&x| x > 0.0) { Some((slot, v)) } else { None }
        })
        .collect()
}

/// Species with a nonzero population inside the window, with their mean
/// Resistance series.
fn resistance_series(w: &Window<'_>) -> Vec<(SpeciesId, Vec<f32>)> {
    SpeciesId::ALL
        .iter()
        .filter(|id| w.samples.iter().any(|s| s.population[id.index()] > 0))
        .map(|id| (*id, w.samples.iter().map(|s| s.genome_mean[id.index()].resistance()).collect()))
        .collect()
}

/// The species an outbreak hit hardest (most cases), or `None` before any case.
fn outbreak_host(o: &Outbreak) -> Option<SpeciesId> {
    let (i, n) = o.species_cases.iter().enumerate().max_by_key(|(i, n)| (**n, std::cmp::Reverse(*i)))?;
    if *n == 0 { None } else { Some(SpeciesId::ALL[i]) }
}

/// Several half-block series over one y scale, with epidemic windows shaded;
/// `reference` rows are drawn as dim dotted lines behind the series.
#[allow(clippy::too_many_arguments)]
fn multi_line_chart(f: &mut Frame<'_>, area: Rect, w: &Window<'_>, series: &[(Vec<f32>, Color)], reference: &[(f32, Color)], y_max: f32, y_label: impl Fn(f32) -> String) {
    if area.height < 4 || area.width < 12 {
        return;
    }
    let label_w = 6u16;
    let plot = Rect::new(area.x + label_w, area.y, area.width - label_w - 1, area.height - 2);
    let axis_y = plot.bottom();
    let cols = crate::cast!(plot.width => usize);
    let rows = crate::cast!(plot.height => usize);
    let buf = f.buffer_mut();
    let band = theme::lerp(theme::PANEL_BG, theme::SICK, 0.22);
    multi_line_columns(buf, w, plot, series, reference, y_max, cols, rows, band);
    multi_line_axes(buf, area, plot, axis_y, w, y_max, cols, rows, y_label);
}
/// The multi-line columns: epidemic band, dotted reference rows and the series.
#[allow(clippy::too_many_arguments)]
fn multi_line_columns(buf: &mut Buffer, w: &Window<'_>, plot: Rect, series: &[(Vec<f32>, Color)], reference: &[(f32, Color)], y_max: f32, cols: usize, rows: usize, band: Color) {
    let mut band_started: Option<u16> = None;
    let y_of = |v: f32| -> usize { rows - 1 - (crate::cast!(((v / y_max).clamp(0.0, 1.0) * (crate::cast!(rows => f32) - 1.0)).round() => usize)).min(rows - 1) };
    for c in 0..cols {
        let x = plot.x + crate::cast!(c => u16);
        if w.len() > 0 && w.epidemic_in(c, cols) {
            paint_epidemic_band(buf, plot, x, rows, band);
            if band_started.is_none() {
                band_started = Some(x);
            }
        }
        paint_reference_dots(buf, plot, x, reference, c, &y_of);
        if w.len() == 0 {
            continue;
        }
        paint_series_column(buf, plot, x, w, series, c, (cols, rows), y_max);
    }
    if let Some(bx) = band_started {
        buf.set_stringn(bx + 1, plot.y, format!("{} epidemic", glyphs::DISEASE), 11, Style::default().fg(theme::SICK).bg(band));
    }
}

/// Shade one whole column as an epidemic band.
fn paint_epidemic_band(buf: &mut Buffer, plot: Rect, x: u16, rows: usize, band: Color) {
    for r in 0..rows {
        if let Some(cell) = buf.cell_mut((x, plot.y + crate::cast!(r => u16))) {
            cell.set_char(' ');
            cell.set_style(Style::default().bg(band));
        }
    }
}

/// Dotted reference markers on every other column.
fn paint_reference_dots<F: Fn(f32) -> usize>(buf: &mut Buffer, plot: Rect, x: u16, reference: &[(f32, Color)], c: usize, y_of: &F) {
    for &(v, color) in reference {
        let r = y_of(v);
        if let Some(cell) = buf.cell_mut((x, plot.y + crate::cast!(r => u16))) {
            if c % 2 == 0 {
                cell.set_char(glyphs::DOT);
                cell.set_style(Style::default().fg(theme::dim(color, 0.55)).bg(cell.bg));
            }
        }
    }
}

/// One column of half-block bars for every series.
fn paint_series_column(buf: &mut Buffer, plot: Rect, x: u16, w: &Window<'_>, series: &[(Vec<f32>, Color)], c: usize, dims: (usize, usize), y_max: f32) {
    let (cols, rows) = dims;
    for (values, color) in series {
        let v = w.mean_over(values, c, cols);
        let halves = crate::cast!(((v / y_max).clamp(0.0, 1.0) * (crate::cast!(rows => f32) * 2.0)).round() => usize);
        if halves == 0 {
            if let Some(cell) = buf.cell_mut((x, plot.y + crate::cast!(rows => u16) - 1)) {
                cell.set_char(glyphs::HALF_LOWER);
                cell.set_style(Style::default().fg(*color).bg(cell.bg));
            }
            continue;
        }
        let r = rows - 1 - ((halves - 1).div_euclid(2)).min(rows - 1);
        let ch = if halves % 2 == 1 { glyphs::HALF_LOWER } else { glyphs::HALF_UPPER };
        if let Some(cell) = buf.cell_mut((x, plot.y + crate::cast!(r => u16))) {
            cell.set_char(ch);
            cell.set_style(Style::default().fg(*color).bg(cell.bg).add_modifier(Modifier::BOLD));
        }
    }
}

/// The multi-line chart's Y labels, box lines and X axis.
#[allow(clippy::too_many_arguments)]
fn multi_line_axes<F: Fn(f32) -> String>(buf: &mut Buffer, area: Rect, plot: Rect, axis_y: u16, w: &Window<'_>, y_max: f32, cols: usize, rows: usize, y_label: F) {
    for k in 0..=4 {
        let y = axis_y - 1 - crate::cast!((crate::cast!((rows - 1) => f32) * crate::cast!(k => f32) / 4.0).round() => u16);
        buf.set_stringn(area.x, y, format!("{:>5}", y_label(y_max * crate::cast!(k => f32) / 4.0)), 5, theme::dim_text());
        buf.set_stringn(plot.x - 1, y, glyphs::CROSS.to_string(), 1, theme::border());
    }
    for r in 0..rows {
        let y = plot.y + crate::cast!(r => u16);
        if buf.cell((plot.x - 1, y)).is_some_and(|c| c.symbol() != "┼") {
            buf.set_stringn(plot.x - 1, y, glyphs::V_LINE.to_string(), 1, theme::border());
        }
    }
    let axis: String = std::iter::repeat_n(glyphs::H_LINE, cols + 1).collect();
    buf.set_stringn(plot.x - 1, axis_y, &axis, cols + 1, theme::border());
    let n = w.len().max(1);
    for k in 0..=6 {
        let i = ((k * (n - 1)).div_euclid(6)).min(n - 1);
        let x = if k == 6 { plot.x + crate::cast!(cols => u16) - 1 } else { plot.x + crate::cast!(((i * cols).div_euclid(n)) => u16) };
        buf.set_stringn(x, axis_y, glyphs::CROSS.to_string(), 1, theme::border());
        let label = w.day_label(i);
        if !label.is_empty() {
            let lx = crate::cast!((i32::from(x) - crate::cast!(label.len() => i32).div_euclid(2)).max(i32::from(area.x)) => u16);
            let lx = lx.min(area.right().saturating_sub(crate::cast!(label.len() => u16) + 1));
            buf.set_stringn(lx, axis_y + 1, &label, label.len(), theme::dim_text());
        }
    }
    buf.set_stringn(plot.right() - 3, axis_y, "now", 3, Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG));
}

fn infection_chart(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw_with_hint(f, area, "Infections — cases and resistance", &format!("last {} days", w.len()), panel::Kind::Outer);
    let half = (inner.height.saturating_sub(1)).div_euclid(2);
    let upper = Rect::new(inner.x, inner.y, inner.width, half);
    let lower = Rect::new(inner.x, inner.y + half + 1, inner.width, inner.height - half - 1);

    // Top: active cases per pathogen slot.
    let cases = case_series(w);
    let mut spans = vec![sp(" Active cases", Style::default().fg(theme::SICK).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))];
    if cases.is_empty() {
        spans.push(sp("   no infections in the window", theme::dim_text()));
    }
    for (slot, _) in &cases {
        spans.push(sp(format!("   {}{} ", glyphs::HALF_UPPER, glyphs::HALF_LOWER), Style::default().fg(slot_color(*slot)).bg(theme::PANEL_BG)));
        spans.push(sp(sim.disease.name(PathogenId(crate::cast!(*slot => u8))).to_string(), theme::text()));
    }
    util::line(f, upper, 0, Line::from(spans));
    let max_cases = cases.iter().flat_map(|(_, v)| v.iter().copied()).fold(0.0f32, f32::max);
    let case_lines: Vec<(Vec<f32>, Color)> = cases.iter().map(|(slot, v)| (v.clone(), slot_color(*slot))).collect();
    multi_line_chart(f, Rect::new(upper.x, upper.y + 1, upper.width, upper.height - 1), w, &case_lines, &[], round_up(max_cases, 10.0), |v| format!("{v:.0}"));

    // Bottom: mean Resistance per species with the base value as a reference.
    panel::section(f, inner, half, "Mean Resistance (host species)");
    let resist = resistance_series(w);
    let mut spans = vec![sp(" 0..1", theme::dim_text())];
    for (id, _) in &resist {
        spans.push(sp(format!("   {}{} ", glyphs::HALF_UPPER, glyphs::HALF_LOWER), Style::default().fg(id.color()).bg(theme::PANEL_BG)));
        spans.push(sp(id.plural().to_string(), theme::text()));
    }
    spans.push(sp(format!("   {} base", glyphs::DOT), theme::dim_text()));
    util::line(f, lower, 0, Line::from(spans));
    let resist_lines: Vec<(Vec<f32>, Color)> = resist.iter().map(|(id, v)| (v.clone(), id.color())).collect();
    let reference: Vec<(f32, Color)> = resist.iter().map(|(id, _)| (id.base_genome().resistance(), id.color())).collect();
    multi_line_chart(f, Rect::new(lower.x, lower.y + 1, lower.width, lower.height - 1), w, &resist_lines, &reference, 1.0, |v| format!("{v:.2}"));
}

fn infection_sidebar(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>) {
    let inner = panel::draw(f, area, "Outbreaks", panel::Kind::Outer);
    let mut row = 0u16;
    let today = crate::cast!(sim.time.day_index() => u32);
    let recent: Vec<&Outbreak> = sim.disease.outbreaks.iter().rev().take(8).collect();
    if recent.is_empty() {
        util::line(f, inner, row, Line::from(sp(" no outbreaks yet", theme::dim_text())));
        row += 1;
    }
    for o in recent {
        let strain = sim.disease.pathogen(o.pathogen).is_some_and(crate::sim::disease::Pathogen::is_strain);
        let color = if strain { theme::MAGENTA } else { theme::SICK };
        let name = sim.disease.name(o.pathogen).to_string();
        let stamp = day_stamp(i64::from(o.started_day), w.season_days);
        let tag = if o.epidemic { " EPIDEMIC" } else { "" };
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", glyphs::DISEASE), Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{name:<14}"), Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("{stamp:<9}"), theme::dim_text()),
            sp(tag, Style::default().fg(theme::SICK).bg(theme::PANEL_BG)),
        ]));
        row += 1;
        let (host_glyph, delta) = match outbreak_host(o) {
            Some(id) => {
                let i = id.index();
                let delta = if o.ended_day.is_none() { "open".to_string() } else { format!("{:+.2}", o.resist_at_end[i] - o.resist_at_start[i]).replace("0.", ".") };
                (format!("{} ", id.glyph().to_ascii_uppercase()), delta)
            }
            None => ("- ".to_string(), "open".to_string()),
        };
        let _ = today;
        util::line(f, inner, row, Line::from(vec![
            sp(format!("   {host_glyph}"), Style::default().fg(outbreak_host(o).map_or(theme::DIM, |id| id.color())).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            sp(format!("cases {:<5} dead {:<5} {}resist {}", o.cases, o.deaths, DELTA, delta), theme::text()),
        ]));
        row += 1;
    }
    row += 1;

    panel::section(f, inner, row, "Pathogens");
    row += 1;
    let cases = case_series(w);
    if cases.is_empty() {
        util::line(f, inner, row, Line::from(sp(" none active in the window", theme::dim_text())));
        row += 1;
    }
    for (slot, v) in &cases {
        let id = PathogenId(crate::cast!(*slot => u8));
        let strain = sim.disease.pathogen(id).is_some_and(crate::sim::disease::Pathogen::is_strain);
        let peak = crate::cast!(v.iter().copied().fold(0.0f32, f32::max) => u32);
        let now = crate::cast!(v.last().copied().unwrap_or(0.0) => u32);
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {}{} ", glyphs::HALF_UPPER, glyphs::HALF_LOWER), Style::default().fg(slot_color(*slot)).bg(theme::PANEL_BG)),
            sp(format!("{:<14}", sim.disease.name(id)), theme::text()),
            sp(format!("now {now:<4} peak {peak:<4}"), theme::dim_text()),
            sp(if strain { "strain" } else { "" }, Style::default().fg(theme::MAGENTA).bg(theme::PANEL_BG)),
        ]));
        row += 1;
    }
    row += 1;

    panel::section(f, inner, row, "Legend");
    row += 1;
    for (glyph, desc, color) in [
        ("▀▄", "active cases per pathogen (top)", theme::SICK),
        ("▀▄", "mean Resistance per species (bottom)", theme::HARE),
        ("·", "base Resistance of the species", theme::DIM),
        ("░", "epidemic window", theme::SICK),
    ] {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {glyph:<3}"), Style::default().fg(color).bg(theme::PANEL_BG)),
            sp(desc, theme::text()),
        ]));
        row += 1;
    }
    row += 1;
    panel::section(f, inner, row, "Keys");
    row += 1;
    util::line(f, inner, row, Line::from(sp(" [+/-] zoom 60/240/720d", theme::dim_text())));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{CreatureId, Params};
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyModifiers;
    use ratatui::Terminal;

    fn fake_outbreak(sim: &mut Sim) {
        sim.disease.first_index = 0;
        sim.disease.outbreaks.push(Outbreak {
            pathogen: PathogenId(0),
            started_day: 0,
            ended_day: Some(12),
            origin_region: 1,
            index_case: CreatureId(0),
            cases: 14,
            deaths: 5,
            recovered: 9,
            peak_active: 7,
            peak_day: 6,
            species_cases: [14, 0, 0, 0, 0, 0],
            species_deaths: [5, 0, 0, 0, 0, 0],
            epidemic: true,
            resist_at_start: [0.30; 6],
            resist_at_end: [0.33; 6],
            active: 0,
            cases_today: 0,
        });
    }

    fn screen_text(app: &AppState, screen: &Charts) -> String {
        let backend = TestBackend::new(155, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| screen.render(app, f, Rect::new(0, 0, 155, 45))).unwrap();
        let buf = terminal.backend().buffer();
        (0..45).map(|y| (0..155).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    }

    #[test]
    fn s05d_variant_cycling() {
        let mut app = AppState::new(Params::default());
        let mut sim = Sim::new(7, Params::default());
        fake_outbreak(&mut sim);
        app.sim = Some(sim);

        let mut s = Charts::new();
        // g cycles a → b → c → d → a.
        for _ in 0..3 {
            s.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE), &mut app);
        }
        assert!(s.variant == Variant::Infections);
        s.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE), &mut app);
        assert!(s.variant == Variant::Time);
        // 4 picks it directly; 1–3 keep their meaning.
        s.handle_key(KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE), &mut app);
        assert!(s.variant == Variant::Infections);
        s.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE), &mut app);
        assert!(s.variant == Variant::Phase);
        assert!(screen_text(&app, &s).contains("chart 2/4"));
    }

    #[test]
    fn s05d_infections_render() {
        let mut app = AppState::new(Params::default());
        let mut sim = Sim::new(7, Params::default());
        fake_outbreak(&mut sim);
        let name = sim.disease.name(PathogenId(0)).to_string();
        assert!(!name.is_empty() && name != "?", "default roster should have a pathogen in slot 0");
        app.sim = Some(sim);

        let mut s = Charts::new();
        for _ in 0..3 {
            s.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE), &mut app);
        }
        assert!(s.variant == Variant::Infections);
        let text = screen_text(&app, &s);
        assert!(text.contains("Infections — cases and resistance"), "{text}");
        assert!(text.contains("Mean Resistance (host species)"), "{text}");
        assert!(text.contains("Outbreaks"), "{text}");
        assert!(text.contains(&format!("{} {}", glyphs::DISEASE, name)), "{text}");
        assert!(text.contains("cases 14"), "{text}");
        assert!(text.contains("dead 5"), "{text}");
        assert!(text.contains("resist +.03"), "{text}");
        assert!(text.contains("chart 4/4  infections"), "{text}");
    }
}
