//! S05: population charts. Three variants: stacked prey/predator line charts,
//! a predator-prey phase plot, and a hand-drawn stacked area of all species.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Chart, Dataset, GraphType, Widget};
use ratatui::Frame;

use super::Prototype;
use crate::fixtures::{self, series, Fixtures, SpeciesId};
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Lines,
    Phase,
    Stacked,
}

pub struct Charts {
    pub variant: Variant,
}

const CHART_PANEL_W: u16 = 112; // inner 110
const SIDEBAR_W: u16 = 43; // inner 41
const DROUGHT: (usize, usize) = (150, 190);

pub fn all() -> Vec<Box<dyn Prototype>> {
    vec![
        Box::new(Charts { variant: Variant::Lines }),
        Box::new(Charts { variant: Variant::Phase }),
        Box::new(Charts { variant: Variant::Stacked }),
    ]
}

impl Prototype for Charts {
    fn id(&self) -> &'static str {
        match self.variant {
            Variant::Lines => "S05a",
            Variant::Phase => "S05b",
            Variant::Stacked => "S05c",
        }
    }
    fn name(&self) -> &'static str {
        "Population Charts"
    }
    fn variant(&self) -> &'static str {
        match self.variant {
            Variant::Lines => "prey vs predator over time",
            Variant::Phase => "predator-prey phase plot",
            Variant::Stacked => "stacked species + vegetation",
        }
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        let fx = fixtures::get();
        let body_h = area.height - 1;
        let chart_area = Rect::new(area.x, area.y, CHART_PANEL_W, body_h);
        let side_area = Rect::new(area.x + CHART_PANEL_W, area.y, SIDEBAR_W, body_h);
        match self.variant {
            Variant::Lines => {
                lines_chart(f, chart_area, fx);
                lines_sidebar(f, side_area, fx);
            }
            Variant::Phase => {
                phase_chart(f, chart_area, fx);
                phase_sidebar(f, side_area, fx);
            }
            Variant::Stacked => {
                stacked_chart(f, chart_area, fx);
                stacked_sidebar(f, side_area, fx);
            }
        }
        let which = match self.variant {
            Variant::Lines => "chart 1/3  populations",
            Variant::Phase => "chart 2/3  phase plot",
            Variant::Stacked => "chart 3/3  stacked species",
        };
        status::render(
            f,
            Rect::new(area.x, area.y + area.height - 1, area.width, 1),
            &[("g", "next chart"), ("1-3", "pick"), ("+/-", "zoom"), ("Esc", "back")],
            which,
        );
    }
}

// ------------------------------------------------------------------ helpers

fn sp(s: impl Into<String>, fg: Color) -> Span<'static> {
    Span::styled(s.into(), Style::default().fg(fg).bg(theme::PANEL_BG))
}

fn bold(s: impl Into<String>, fg: Color) -> Span<'static> {
    Span::styled(s.into(), Style::default().fg(fg).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD))
}

/// Absolute day index -> "Y11 D124".
fn day_label(abs: u32) -> String {
    format!("Y{} D{:03}", abs / 360, abs % 360)
}

struct Stats {
    min: f32,
    min_at: usize,
    max: f32,
    max_at: usize,
    mean: f32,
}

fn stats(v: &[f32]) -> Stats {
    let mut s = Stats { min: f32::MAX, min_at: 0, max: f32::MIN, max_at: 0, mean: 0.0 };
    for (i, &x) in v.iter().enumerate() {
        if x < s.min {
            s.min = x;
            s.min_at = i;
        }
        if x > s.max {
            s.max = x;
            s.max_at = i;
        }
        s.mean += x;
    }
    s.mean /= v.len().max(1) as f32;
    s
}

fn pearson(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let (ma, mb) = (a[..n].iter().sum::<f32>() / n as f32, b[..n].iter().sum::<f32>() / n as f32);
    let (mut num, mut da, mut db) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (x, y) = (a[i] - ma, b[i] - mb);
        num += x * y;
        da += x * x;
        db += y * y;
    }
    if da == 0.0 || db == 0.0 {
        0.0
    } else {
        num / (da * db).sqrt()
    }
}

/// Lag (in days) at which predators best follow prey, and the correlation there.
fn best_lag(prey: &[f32], pred: &[f32]) -> (usize, f32) {
    let mut best = (0usize, -1.0f32);
    for lag in 0..=60 {
        let r = pearson(&prey[..prey.len() - lag], &pred[lag..]);
        if r > best.1 {
            best = (lag, r);
        }
    }
    best
}

fn round_up(v: f32, step: f32) -> f64 {
    ((v / step).ceil() * step) as f64
}

/// Left offset of the graph area inside a `Chart` given our label widths
/// (mirrors ratatui's layout: max(y label width, first x label width - 1) + axis column).
fn graph_x0(chart: Rect, y_label_w: u16, first_x_label_w: u16) -> u16 {
    chart.x + y_label_w.max(first_x_label_w.saturating_sub(1)) + 1
}

/// Tint the background of empty cells in a vertical band (drought window).
fn shade_band(buf: &mut Buffer, x0: u16, x1: u16, y0: u16, y1: u16, bg: Color) {
    for y in y0..y1 {
        for x in x0..x1 {
            if let Some(c) = buf.cell_mut((x, y)) {
                if c.symbol() == " " {
                    c.set_bg(bg);
                }
            }
        }
    }
}

fn x_axis<'a>(day0: u32, len: usize) -> Axis<'a> {
    let labels: Vec<Line<'a>> = (0..=6).map(|k| Line::from(sp(day_label(day0 + (k * len as u32) / 6), theme::DIM))).collect();
    Axis::default().bounds([day0 as f64, (day0 + len as u32) as f64]).labels(labels).style(theme::border())
}

fn y_axis<'a>(max: f64, title: &'a str) -> Axis<'a> {
    let labels: Vec<Line<'a>> = (0..=4).map(|k| Line::from(sp(format!("{:>6}", (max * k as f64 / 4.0).round() as u32), theme::DIM))).collect();
    Axis::default().bounds([0.0, max]).labels(labels).style(theme::border()).title(sp(title, theme::ACCENT))
}

// ------------------------------------------------------------------ S05a

fn lines_chart(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let inner = panel::draw_with_hint(f, area, "Population — prey vs predators", "last 240 days, 1 point per day", panel::Kind::Outer);
    let prey = fx.series.prey_total();
    let pred = fx.series.pred_total();
    let day0 = fx.series.day0;
    let prey_pts: Vec<(f64, f64)> = prey.iter().enumerate().map(|(i, &v)| ((day0 + i as u32) as f64, v as f64)).collect();
    let pred_pts: Vec<(f64, f64)> = pred.iter().enumerate().map(|(i, &v)| ((day0 + i as u32) as f64, v as f64)).collect();
    let prey_max = round_up(stats(&prey).max, 100.0);
    let pred_max = round_up(stats(&pred).max, 20.0);

    let half = (inner.height - 1) / 2; // 20 rows each, one divider row between
    let top = Rect::new(inner.x, inner.y, inner.width, half);
    let bottom = Rect::new(inner.x, inner.y + half + 1, inner.width, inner.height - half - 1);
    panel::section(f, inner, half, "Predators (foxes + wolves + lynx)");
    {
        let buf = f.buffer_mut();
        buf.set_stringn(inner.x + 1, inner.y, " Prey (voles + hares + deer) ", inner.width as usize, theme::label());
    }

    for (rect, pts, color, max, title) in [
        (top, &prey_pts, theme::HARE, prey_max, "prey"),
        (bottom, &pred_pts, theme::WOLF, pred_max, "predators"),
    ] {
        // Leave the first row of the top chart for the section label.
        let rect = if rect.y == inner.y { Rect::new(rect.x, rect.y + 1, rect.width, rect.height - 1) } else { rect };
        let ds = Dataset::default()
            .name(title)
            .marker(Marker::HalfBlock)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(color))
            .data(pts);
        Chart::new(vec![ds])
            .x_axis(x_axis(day0, series::LEN))
            .y_axis(y_axis(max, ""))
            .style(theme::text())
            .legend_position(None)
            .render(rect, f.buffer_mut());
        // Drought band over the empty cells of the graph area.
        let gx0 = graph_x0(rect, 6, 8);
        let gw = (rect.right() - gx0) as f32;
        let bx0 = gx0 + (DROUGHT.0 as f32 / series::LEN as f32 * gw) as u16;
        let bx1 = gx0 + (DROUGHT.1 as f32 / series::LEN as f32 * gw) as u16;
        let band = theme::lerp(theme::PANEL_BG, theme::WARN, 0.22);
        shade_band(f.buffer_mut(), bx0, bx1, rect.y, rect.bottom() - 2, band);
        let buf = f.buffer_mut();
        let label = format!("{} drought", glyphs::DROUGHT);
        buf.set_stringn(bx0 + 1, rect.y, &label, 12, Style::default().fg(theme::WARN).bg(band));
        // "now" marker at the right edge.
        buf.set_stringn(rect.right() - 3, rect.bottom() - 2, "now", 3, Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG));
    }
}

fn lines_sidebar(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let inner = panel::draw(f, area, "Statistics", panel::Kind::Outer);
    let prey = fx.series.prey_total();
    let pred = fx.series.pred_total();
    let (ps, qs) = (stats(&prey), stats(&pred));
    let day0 = fx.series.day0;
    let now_p = *prey.last().unwrap();
    let now_q = *pred.last().unwrap();
    let mut row = 0;

    panel::section(f, inner, row, "Current");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" prey       ", theme::TEXT),
        bold(format!("{:>5}", now_p.round() as u32), theme::HARE),
        sp(format!("   {} vs 30d ago", trend(&prey)), theme::DIM),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" predators  ", theme::TEXT),
        bold(format!("{:>5}", now_q.round() as u32), theme::WOLF),
        sp(format!("   {} vs 30d ago", trend(&pred)), theme::DIM),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(sp(format!(" ratio {:.1} prey per predator", now_p / now_q.max(1.0)), theme::DIM)));
    row += 2;

    for (title, s, color) in [("Prey", &ps, theme::HARE), ("Predators", &qs, theme::WOLF)] {
        panel::section(f, inner, row, title);
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            sp(" min  ", theme::DIM),
            bold(format!("{:>5}", s.min.round() as u32), color),
            sp(format!("   {}", day_label(day0 + s.min_at as u32)), theme::DIM),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            sp(" max  ", theme::DIM),
            bold(format!("{:>5}", s.max.round() as u32), color),
            sp(format!("   {}", day_label(day0 + s.max_at as u32)), theme::DIM),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            sp(" mean ", theme::DIM),
            bold(format!("{:>5}", s.mean.round() as u32), color),
            sp(format!("   swing {:.0}%", (s.max - s.min) / s.mean * 100.0), theme::DIM),
        ]));
        row += 2;
    }

    panel::section(f, inner, row, "Coupling");
    row += 1;
    let lag = qs.max_at as i32 - ps.max_at as i32;
    let (best, r_best) = best_lag(&prey, &pred);
    let r0 = pearson(&prey, &pred);
    util::line(f, inner, row, Line::from(sp(" predator peak lags prey peak", theme::TEXT)));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" by ", theme::TEXT),
        bold(format!("~{} days", lag), theme::ACCENT),
        sp(format!("   ({} vs {})", day_label(day0 + ps.max_at as u32), day_label(day0 + qs.max_at as u32)), theme::DIM),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(sp(format!(" r = {:+.2} same day, {:+.2} at lag {}", r0, r_best, best), theme::DIM)));
    row += 1;
    util::line(f, inner, row, Line::from(sp(" prey booms feed predators; predators", theme::DIM)));
    row += 1;
    util::line(f, inner, row, Line::from(sp(" overshoot, prey crash, predators starve", theme::DIM)));
    row += 2;

    panel::section(f, inner, row, "Drought");
    row += 1;
    let veg_before = fx.series.vegetation[DROUGHT.0 - 1];
    let veg_low = stats(&fx.series.vegetation[DROUGHT.0..DROUGHT.1]).min;
    let q_before = pred[DROUGHT.0 - 1];
    let q_after = pred[DROUGHT.1 - 1];
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} ", glyphs::DROUGHT), theme::WARN),
        sp(format!("{} to {}", day_label(day0 + DROUGHT.0 as u32), day_label(day0 + DROUGHT.1 as u32)), theme::TEXT),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(sp(format!(" vegetation {:.0}% {} {:.0}%, water x0.60", veg_before * 100.0, glyphs::DOWN, veg_low * 100.0), theme::DIM)));
    row += 1;
    util::line(f, inner, row, Line::from(sp(format!(" prey held at floor; predators {} {} -> {}", glyphs::DOWN, q_before.round() as u32, q_after.round() as u32), theme::DIM)));
    row += 2;

    panel::section(f, inner, row, "Map census (sampled)");
    row += 1;
    for chunk in SpeciesId::ALL.chunks(3) {
        let mut spans = vec![sp(" ", theme::TEXT)];
        for id in chunk {
            let sp_ = &fx.species[SpeciesId::ALL.iter().position(|s| s == id).unwrap()];
            spans.push(bold(format!("{} ", id.glyph().to_ascii_uppercase()), id.color()));
            spans.push(sp(format!("{:<5}{:>4}   ", id.name(), sp_.count), theme::TEXT));
        }
        util::line(f, inner, row, Line::from(spans));
        row += 1;
    }
    row += 1;

    panel::section(f, inner, row, "Legend");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        bold(format!(" {}{} ", glyphs::HALF_UPPER, glyphs::HALF_LOWER), theme::HARE),
        sp("prey total   ", theme::TEXT),
        sp("voles+hares+deer", theme::DIM),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        bold(format!(" {}{} ", glyphs::HALF_UPPER, glyphs::HALF_LOWER), theme::WOLF),
        sp("predators    ", theme::TEXT),
        sp("foxes+wolves+lynx", theme::DIM),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        Span::styled("    ", Style::default().bg(theme::lerp(theme::PANEL_BG, theme::WARN, 0.22))),
        sp(" drought window", theme::TEXT),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(sp(" separate y scales: prey per 100,", theme::DIM)));
    row += 1;
    util::line(f, inner, row, Line::from(sp(" predators per 20", theme::DIM)));
    row += 2;
    panel::section(f, inner, row, "Keys");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        Span::styled(" [+/-]", theme::key()),
        sp(" zoom 60/240/720d  ", theme::DIM),
        Span::styled("[l]", theme::key()),
        sp(" log scale", theme::DIM),
    ]));
}

fn trend(v: &[f32]) -> String {
    let now = v[v.len() - 1];
    let ago = v[v.len() - 31];
    let d = (now - ago) / ago.max(1.0) * 100.0;
    let arrow = if d > 3.0 { glyphs::UP } else if d < -3.0 { glyphs::DOWN } else { glyphs::FLAT };
    format!("{} {:+.0}%", arrow, d)
}

// ------------------------------------------------------------------ S05b

const RECENT: usize = 40;

fn phase_chart(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let inner = panel::draw_with_hint(f, area, "Phase plot — predators against prey", "one point per day, 240 days", panel::Kind::Outer);
    let prey = fx.series.prey_total();
    let pred = fx.series.pred_total();
    let n = prey.len();
    let pts: Vec<(f64, f64)> = prey.iter().zip(pred.iter()).map(|(&p, &q)| (p as f64, q as f64)).collect();
    let old = &pts[..n - RECENT];
    let recent = &pts[n - RECENT..];
    let last = &pts[n - 1..];
    let (ps, qs) = (stats(&prey), stats(&pred));
    let x_max = round_up(ps.max, 50.0);
    let y_max = round_up(qs.max, 20.0);
    let (eq_x, eq_y) = (ps.mean as f64, qs.mean as f64);
    let vline = [(eq_x, 0.0), (eq_x, y_max)];
    let hline = [(0.0, eq_y), (x_max, eq_y)];

    let x_labels: Vec<Line> = (0..=5).map(|k| Line::from(sp(format!("{}", (x_max * k as f64 / 5.0).round() as u32), theme::DIM))).collect();
    let y_labels: Vec<Line> = (0..=4).map(|k| Line::from(sp(format!("{:>4}", (y_max * k as f64 / 4.0).round() as u32), theme::DIM))).collect();
    let datasets = vec![
        Dataset::default().marker(Marker::Dot).graph_type(GraphType::Line).style(Style::default().fg(theme::lerp(theme::DIM, theme::PANEL_BG, 0.35))).data(&vline),
        Dataset::default().marker(Marker::Dot).graph_type(GraphType::Line).style(Style::default().fg(theme::lerp(theme::DIM, theme::PANEL_BG, 0.35))).data(&hline),
        Dataset::default().marker(Marker::Dot).graph_type(GraphType::Scatter).style(Style::default().fg(theme::dim(theme::HARE, 0.45))).data(old),
        Dataset::default().marker(Marker::HalfBlock).graph_type(GraphType::Line).style(Style::default().fg(theme::ACCENT)).data(recent),
        Dataset::default().marker(Marker::Block).graph_type(GraphType::Scatter).style(Style::default().fg(theme::TEXT_BRIGHT)).data(last),
    ];
    let chart_rect = Rect::new(inner.x, inner.y, inner.width, inner.height - 3);
    Chart::new(datasets)
        .x_axis(Axis::default().bounds([0.0, x_max]).labels(x_labels).style(theme::border()).title(sp("prey total", theme::HARE)))
        .y_axis(Axis::default().bounds([0.0, y_max]).labels(y_labels).style(theme::border()).title(sp("predators", theme::WOLF)))
        .style(theme::text())
        .legend_position(None)
        .render(chart_rect, f.buffer_mut());

    // Quadrant captions inside the plot corners.
    let gx0 = graph_x0(chart_rect, 4, 1);
    let gy1 = chart_rect.bottom() - 2;
    let buf = f.buffer_mut();
    let cap = Style::default().fg(theme::DIM).bg(theme::PANEL_BG);
    buf.set_stringn(gx0 + 1, chart_rect.y + 1, "II  few prey, many predators: hunters starve", 50, cap);
    buf.set_stringn(chart_rect.right() - 44, chart_rect.y + 1, "I  many prey, many predators: prey crash", 44, cap);
    buf.set_stringn(gx0 + 1, gy1 - 2, "III  few of both: prey recover", 40, cap);
    buf.set_stringn(chart_rect.right() - 45, gy1 - 2, "IV  many prey, few predators: hunters boom", 45, cap);
    // Equilibrium label.
    let ex = gx0 + ((eq_x / x_max) * (chart_rect.right() - gx0) as f64) as u16;
    buf.set_stringn(ex + 1, chart_rect.y + 3, format!("{} equilibrium ({}, {})", glyphs::DIAMOND, eq_x.round() as u32, eq_y.round() as u32), 30, Style::default().fg(theme::INFO).bg(theme::PANEL_BG));

    // Note rows under the chart.
    let r = inner.height - 3;
    panel::section(f, inner, r, "Reading the orbit");
    util::line(f, inner, r + 1, Line::from(vec![
        sp(" The system circles counter-clockwise around the equilibrium: prey grow (right), predators follow (up),", theme::TEXT),
    ]));
    util::line(f, inner, r + 2, Line::from(vec![
        sp(" prey collapse (left), predators starve (down). ", theme::TEXT),
        bold(format!("{}{}", glyphs::HALF_UPPER, glyphs::HALF_LOWER), theme::ACCENT),
        sp(" last 40 days   ", theme::TEXT),
        bold(glyphs::FULL_BLOCK.to_string(), theme::TEXT_BRIGHT),
        sp(" today   ", theme::TEXT),
        sp(glyphs::BULLET.to_string(), theme::dim(theme::HARE, 0.45)),
        sp(" older days", theme::TEXT),
    ]));
}

fn phase_sidebar(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let inner = panel::draw(f, area, "Phase", panel::Kind::Outer);
    let prey = fx.series.prey_total();
    let pred = fx.series.pred_total();
    let (ps, qs) = (stats(&prey), stats(&pred));
    let n = prey.len();
    let (p, q) = (prey[n - 1], pred[n - 1]);
    let (dp, dq) = (p - prey[n - 8], q - pred[n - 8]);
    let mut row = 0;

    panel::section(f, inner, row, "Now");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" prey ", theme::TEXT),
        bold(format!("{:>4}", p.round() as u32), theme::HARE),
        sp(format!("  {} {:+.0} / 7d", arrow(dp), dp), theme::DIM),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" pred ", theme::TEXT),
        bold(format!("{:>4}", q.round() as u32), theme::WOLF),
        sp(format!("  {} {:+.0} / 7d", arrow(dq), dq), theme::DIM),
    ]));
    row += 1;
    let quad = match (p >= ps.mean, q >= qs.mean) {
        (true, true) => ("I", "prey crash ahead"),
        (false, true) => ("II", "predators starving"),
        (false, false) => ("III", "prey recovering"),
        (true, false) => ("IV", "predators booming"),
    };
    util::line(f, inner, row, Line::from(vec![
        sp(" quadrant ", theme::TEXT),
        bold(quad.0, theme::ACCENT),
        sp(format!("  {}", quad.1), theme::TEXT),
    ]));
    row += 1;
    let heading = match (dp >= 0.0, dq >= 0.0) {
        (true, true) => "moving up-right (both growing)",
        (false, true) => "moving up-left (prey falling)",
        (false, false) => "moving down-left (both falling)",
        (true, false) => "moving down-right (prey recovering)",
    };
    util::line(f, inner, row, Line::from(sp(format!(" {}", heading), theme::DIM)));
    row += 2;

    panel::section(f, inner, row, "Equilibrium estimate");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" prey*  ", theme::TEXT),
        bold(format!("{:>4}", ps.mean.round() as u32), theme::HARE),
        sp("   (time-mean of prey)", theme::DIM),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" pred*  ", theme::TEXT),
        bold(format!("{:>4}", qs.mean.round() as u32), theme::WOLF),
        sp("   (time-mean of predators)", theme::DIM),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(sp(" For Lotka-Volterra dynamics the time", theme::DIM)));
    row += 1;
    util::line(f, inner, row, Line::from(sp(" average of an orbit equals its centre.", theme::DIM)));
    row += 1;
    let dist = (((p - ps.mean) / ps.mean).powi(2) + ((q - qs.mean) / qs.mean).powi(2)).sqrt();
    util::line(f, inner, row, Line::from(vec![
        sp(" orbit radius now ", theme::TEXT),
        bold(format!("{:.0}%", dist * 100.0), theme::ACCENT),
        sp("  of centre", theme::DIM),
    ]));
    row += 1;
    let per = match period(&prey) {
        Some(p) => format!("~{} days", p),
        None => "> 240 days".to_string(),
    };
    util::line(f, inner, row, Line::from(vec![
        sp(" period ", theme::TEXT),
        bold(per, theme::ACCENT),
        sp("  (peak to peak)", theme::DIM),
    ]));
    row += 2;

    panel::section(f, inner, row, "Quadrants");
    row += 1;
    let quads: [(&str, &str, &str, Color); 4] = [
        ("I  ", "upper-right", "many prey, many predators. Hunting is easy; prey numbers start to fall.", theme::WOLF),
        ("II ", "upper-left", "few prey, many predators. Hunters starve; predator numbers fall.", theme::WARN),
        ("III", "lower-left", "few of both. With little pressure the prey recover first.", theme::HARE),
        ("IV ", "lower-right", "many prey, few predators. Litters grow; predators boom.", theme::GOOD),
    ];
    for (id, pos, text, color) in quads {
        util::line(f, inner, row, Line::from(vec![
            bold(format!(" {} ", id), color),
            sp(pos, theme::TEXT),
        ]));
        row += 1;
        for l in wrap(text, 35) {
            util::line(f, inner, row, Line::from(sp(format!("     {}", l), theme::DIM)));
            row += 1;
        }
    }
    row += 1;

    panel::section(f, inner, row, "Recent path (every 10 days)");
    row += 1;
    util::line(f, inner, row, Line::from(sp("   day        prey   pred   quadrant", theme::DIM)));
    row += 1;
    for k in (0..=4).rev() {
        let i = n - 1 - k * 10;
        let (pp, qq) = (prey[i], pred[i]);
        let q = match (pp >= ps.mean, qq >= qs.mean) {
            (true, true) => "I",
            (false, true) => "II",
            (false, false) => "III",
            (true, false) => "IV",
        };
        let style = if k == 0 { bold("", theme::TEXT_BRIGHT).style } else { sp("", theme::TEXT).style };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!("   {}  {:>5}  {:>5}   {}", day_label(fx.series.day0 + i as u32), pp.round() as u32, qq.round() as u32, q), style),
        ]));
        row += 1;
    }
    row += 1;

    panel::section(f, inner, row, "Legend");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} ", glyphs::BULLET), theme::dim(theme::HARE, 0.45)),
        sp("days 1-200        ", theme::TEXT),
        bold(format!("{}{} ", glyphs::HALF_UPPER, glyphs::HALF_LOWER), theme::ACCENT),
        sp("last 40 days", theme::TEXT),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        bold(format!(" {} ", glyphs::FULL_BLOCK), theme::TEXT_BRIGHT),
        sp("today             ", theme::TEXT),
        sp(format!("{} ", glyphs::BULLET), theme::DIM),
        sp("equilibrium axes", theme::TEXT),
    ]));
}

fn arrow(d: f32) -> char {
    if d > 1.0 {
        glyphs::UP
    } else if d < -1.0 {
        glyphs::DOWN
    } else {
        glyphs::FLAT
    }
}

/// Rough oscillation period: mean distance between local maxima of a smoothed series.
fn period(v: &[f32]) -> Option<usize> {
    let sm: Vec<f32> = (0..v.len()).map(|i| {
        let a = i.saturating_sub(5);
        let b = (i + 6).min(v.len());
        v[a..b].iter().sum::<f32>() / (b - a) as f32
    }).collect();
    let mut peaks = Vec::new();
    for i in 8..sm.len() - 8 {
        if (i - 8..i + 8).all(|j| sm[j] <= sm[i]) {
            peaks.push(i);
        }
    }
    if peaks.len() < 2 {
        return None;
    }
    Some((peaks.last().unwrap() - peaks[0]) / (peaks.len() - 1))
}

/// Greedy word wrap.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in text.split_whitespace() {
        if !cur.is_empty() && cur.len() + 1 + w.len() > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

// ------------------------------------------------------------------ S05c

const Y_LABEL_W: u16 = 6; // "  123 " + axis
const R_AXIS_W: u16 = 6; // "│ 100%"

fn stacked_chart(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let inner = panel::draw_with_hint(f, area, "Stacked populations + vegetation", "240 days, all six species", panel::Kind::Outer);
    let s = &fx.series;
    let day0 = s.day0;

    // Legend row.
    let mut spans = vec![sp(" ", theme::TEXT)];
    for id in SpeciesId::ALL {
        spans.push(bold(glyphs::FULL_BLOCK.to_string(), id.color()));
        spans.push(sp(format!(" {}   ", id.plural()), theme::TEXT));
    }
    spans.push(bold(glyphs::DOT.to_string(), theme::VEGETATION));
    spans.push(sp(" vegetation biomass (right axis, % of max)", theme::TEXT));
    util::line(f, inner, 0, Line::from(spans));

    // Plot geometry.
    let plot_x = inner.x + Y_LABEL_W;
    let plot_w = inner.width - Y_LABEL_W - R_AXIS_W;
    let plot_y = inner.y + 2;
    let plot_h = inner.height - 7; // legend, gap, axis, labels, gap, 2 note rows
    let axis_y = plot_y + plot_h;

    // Downsample: each column averages the days that fall into it.
    let cols = plot_w as usize;
    let n = series::LEN;
    let bin = |c: usize| -> (usize, usize) {
        let a = c * n / cols;
        let b = ((c + 1) * n / cols).max(a + 1).min(n);
        (a, b)
    };
    let mut stack: Vec<[f32; 6]> = Vec::with_capacity(cols);
    let mut veg: Vec<f32> = Vec::with_capacity(cols);
    for c in 0..cols {
        let (a, b) = bin(c);
        let mut v = [0.0f32; 6];
        for (k, item) in v.iter_mut().enumerate() {
            *item = s.population[k][a..b].iter().sum::<f32>() / (b - a) as f32;
        }
        stack.push(v);
        veg.push(s.vegetation[a..b].iter().sum::<f32>() / (b - a) as f32);
    }
    let max_total = stack.iter().map(|v| v.iter().sum::<f32>()).fold(0.0f32, f32::max);
    let y_max = round_up(max_total, 50.0) as f32;
    let halves = plot_h as usize * 2;

    let bx0 = plot_x + (DROUGHT.0 * cols / n) as u16;
    let bx1 = plot_x + (DROUGHT.1 * cols / n) as u16;
    let band = theme::lerp(theme::PANEL_BG, theme::WARN, 0.22);
    let buf = f.buffer_mut();
    // Column by column: colors of the lower/upper half of every row.
    for (c, v) in stack.iter().enumerate() {
        let x = plot_x + c as u16;
        let mut lower: Vec<Option<Color>> = vec![None; plot_h as usize];
        let mut upper: Vec<Option<Color>> = vec![None; plot_h as usize];
        let mut cum = 0.0f32;
        for (k, id) in SpeciesId::ALL.iter().enumerate() {
            let h0 = (cum / y_max * halves as f32).round() as usize;
            cum += v[k];
            let h1 = (cum / y_max * halves as f32).round() as usize;
            for h in h0..h1.min(halves) {
                let row = plot_h as usize - 1 - h / 2;
                if h % 2 == 0 {
                    lower[row] = Some(id.color());
                } else {
                    upper[row] = Some(id.color());
                }
            }
        }
        let empty_bg = if (bx0..bx1).contains(&x) { band } else { theme::PANEL_BG };
        for r in 0..plot_h as usize {
            let y = plot_y + r as u16;
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
        // Vegetation line: a dot at the vegetation height, keeping the stack color under it.
        let vh = (veg[c].clamp(0.0, 1.0) * (plot_h as f32 - 1.0)).round() as usize;
        let r = plot_h as usize - 1 - vh;
        if let Some(cell) = buf.cell_mut((x, plot_y + r as u16)) {
            let bg = match (lower[r], upper[r]) {
                (Some(lo), _) => lo,
                (None, Some(up)) => up,
                _ => empty_bg,
            };
            cell.set_char(glyphs::DOT);
            cell.set_style(Style::default().fg(theme::VEGETATION).bg(bg).add_modifier(Modifier::BOLD));
        }
    }

    // Drought band on empty cells.
    shade_band(buf, bx0, bx1, plot_y, axis_y, band);
    buf.set_stringn(bx0 + 1, plot_y, format!("{} drought", glyphs::DROUGHT), 12, Style::default().fg(theme::WARN).bg(band));

    // Left y axis (counts) and right axis (vegetation %).
    for r in 0..plot_h {
        let y = plot_y + r;
        buf.set_stringn(plot_x - 1, y, glyphs::V_LINE.to_string(), 1, theme::border());
        buf.set_stringn(plot_x + plot_w, y, glyphs::V_LINE.to_string(), 1, theme::border());
    }
    for k in 0..=5 {
        let y = axis_y - 1 - ((plot_h - 1) as f32 * k as f32 / 5.0).round() as u16;
        let count = (y_max * k as f32 / 5.0).round() as u32;
        buf.set_stringn(inner.x, y, format!("{:>5}", count), 5, theme::dim_text());
        buf.set_stringn(plot_x - 1, y, glyphs::CROSS.to_string(), 1, theme::border());
        buf.set_stringn(plot_x + plot_w, y, format!("{}{:>4}%", glyphs::CROSS, k * 20), R_AXIS_W as usize, Style::default().fg(theme::VEGETATION).bg(theme::PANEL_BG));
    }
    // X axis with ticks every 40 days.
    let axis: String = std::iter::repeat_n(glyphs::H_LINE, plot_w as usize + 1).collect();
    buf.set_stringn(plot_x - 1, axis_y, &axis, plot_w as usize + 1, theme::border());
    for k in 0..=6 {
        let day = k * n / 6;
        let x = plot_x + (day.min(n - 1) * cols / n) as u16;
        let x = if k == 6 { plot_x + plot_w - 1 } else { x };
        buf.set_stringn(x, axis_y, glyphs::CROSS.to_string(), 1, theme::border());
        let label = day_label(day0 + day as u32);
        let lx = (x as i32 - label.len() as i32 / 2).max(inner.x as i32) as u16;
        let lx = lx.min(inner.right() - label.len() as u16);
        buf.set_stringn(lx, axis_y + 1, &label, label.len(), theme::dim_text());
    }
    // Notes.
    let note_y = axis_y + 3;
    let note = Style::default().fg(theme::DIM).bg(theme::PANEL_BG);
    buf.set_stringn(inner.x + 1, note_y, "Species are stacked bottom-up in table order (prey below, predators on top); each column averages ~2.3 days.", inner.width as usize - 2, note);
    buf.set_stringn(inner.x + 1, note_y + 1, "The vegetation dot uses the right-hand scale, so the drought dip lines up with the prey decline after it.", inner.width as usize - 2, note);
}

fn stacked_sidebar(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let inner = panel::draw(f, area, "Composition", panel::Kind::Outer);
    let s = &fx.series;
    let n = series::LEN;
    let mut row = 0;

    panel::section(f, inner, row, "Today");
    row += 1;
    let now: Vec<f32> = (0..6).map(|k| s.population[k][n - 1]).collect();
    let total: f32 = now.iter().sum();
    util::line(f, inner, row, Line::from(sp("   species   count  share   240d", theme::DIM)));
    row += 1;
    for (k, id) in SpeciesId::ALL.iter().enumerate() {
        let series_k = &s.population[k];
        let st = stats(series_k);
        util::line(f, inner, row, Line::from(vec![
            bold(format!(" {} ", glyphs::FULL_BLOCK), id.color()),
            sp(format!("{:<8}", id.plural()), theme::TEXT),
            sp(format!("{:>6}", now[k].round() as u32), theme::TEXT),
            sp(format!("{:>6.0}%", now[k] / total * 100.0), theme::DIM),
            sp(format!("  {:>4}-{:<4}", st.min.round() as u32, st.max.round() as u32), theme::DIM),
        ]));
        row += 1;
    }
    util::line(f, inner, row, Line::from(vec![
        sp("   total    ", theme::TEXT),
        bold(format!("{:>6}", total.round() as u32), theme::TEXT_BRIGHT),
        sp(format!("    veg {:.0}%", s.vegetation[n - 1] * 100.0), theme::VEGETATION),
    ]));
    row += 2;

    panel::section(f, inner, row, "Share today");
    row += 1;
    // One-row stacked bar of today's shares.
    let bar_w = inner.width as usize - 2;
    let mut x = inner.x + 1;
    let y = inner.y + row;
    let buf = f.buffer_mut();
    let mut used = 0usize;
    for (k, id) in SpeciesId::ALL.iter().enumerate() {
        let w = if k == 5 { bar_w - used } else { (now[k] / total * bar_w as f32).round() as usize };
        let seg: String = std::iter::repeat_n(glyphs::FULL_BLOCK, w).collect();
        buf.set_stringn(x, y, &seg, w, Style::default().fg(id.color()).bg(theme::PANEL_BG));
        x += w as u16;
        used += w;
    }
    row += 1;
    let prey: f32 = now[..3].iter().sum();
    let pred: f32 = now[3..].iter().sum();
    util::line(f, inner, row, Line::from(sp(format!(" prey {:.0}%   predators {:.0}%", prey / total * 100.0, pred / total * 100.0), theme::DIM)));
    row += 2;

    panel::section(f, inner, row, "Peaks");
    row += 1;
    let totals: Vec<f32> = (0..n).map(|i| (0..6).map(|k| s.population[k][i]).sum()).collect();
    let ts = stats(&totals);
    util::line(f, inner, row, Line::from(vec![
        sp(" largest stack ", theme::TEXT),
        bold(format!("{}", ts.max.round() as u32), theme::ACCENT),
        sp(format!("  {}", day_label(s.day0 + ts.max_at as u32)), theme::DIM),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" smallest      ", theme::TEXT),
        bold(format!("{}", ts.min.round() as u32), theme::ACCENT),
        sp(format!("  {}", day_label(s.day0 + ts.min_at as u32)), theme::DIM),
    ]));
    row += 1;
    let vs = stats(&s.vegetation);
    util::line(f, inner, row, Line::from(vec![
        sp(" vegetation    ", theme::TEXT),
        bold(format!("{:.0}%", vs.min * 100.0), theme::VEGETATION),
        sp(format!(" min  {:.0}% max", vs.max * 100.0), theme::DIM),
    ]));
    row += 2;

    panel::section(f, inner, row, "Drought");
    row += 1;
    let before: f32 = totals[DROUGHT.0 - 1];
    let after: f32 = totals[DROUGHT.1 + 20];
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} ", glyphs::DROUGHT), theme::WARN),
        sp(format!("days {}-{} of the window", DROUGHT.0, DROUGHT.1), theme::TEXT),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(sp(format!(" stack {} -> {} twenty days later", before.round() as u32, after.round() as u32), theme::DIM)));
    row += 1;
    util::line(f, inner, row, Line::from(sp(" voles shrink first, lynx last", theme::DIM)));
    row += 2;

    panel::section(f, inner, row, "30-day trend");
    row += 1;
    for (k, id) in SpeciesId::ALL.iter().enumerate() {
        let v = &s.population[k];
        let (a, b) = (v[n - 31], v[n - 1]);
        let d = (b - a) / a.max(1.0) * 100.0;
        let color = if d > 3.0 { theme::GOOD } else if d < -3.0 { theme::BAD } else { theme::DIM };
        util::line(f, inner, row, Line::from(vec![
            bold(format!(" {} ", id.glyph().to_ascii_uppercase()), id.color()),
            sp(format!("{:<8}", id.plural()), theme::TEXT),
            sp(format!("{:>4} -> {:<4}", a.round() as u32, b.round() as u32), theme::DIM),
            Span::styled(format!(" {} {:+.0}%", arrow(d), d), Style::default().fg(color).bg(theme::PANEL_BG)),
        ]));
        row += 1;
    }
    row += 1;

    panel::section(f, inner, row, "How to read");
    row += 1;
    for l in [
        "Each column stacks the six species so",
        "the top edge is the whole population.",
        "Half blocks show fractional heights.",
        "The green dot is vegetation on the",
        "right axis (0-100% of maximum).",
    ] {
        util::line(f, inner, row, Line::from(sp(format!(" {}", l), theme::DIM)));
        row += 1;
    }
}
