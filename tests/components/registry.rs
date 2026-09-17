//! One entry per example heading in `docs/components/*.md`. Keep the heading
//! text verbatim, including inline code; `examples_render_cell_for_cell`
//! matches on it.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use sim_fortress::{cast, theme};
use sim_fortress::widgets::Constraint::{Fill, Fixed};
use sim_fortress::widgets::{
    Bar, Component, Divider, HStack, Inverted, Kind, LabeledBar, Panel, RangeBar, Spacer, Sparkline, Text, TrendArrow, VStack,
};

use super::{Entry, Frame};

/// Row `i` of `area`, one row high.
const fn row(area: Rect, i: u16) -> Rect {
    Rect::new(area.x, area.y + i, area.width, 1)
}

fn bare(sheet: &'static str, heading: &'static str, draw: fn(&mut Buffer, Rect)) -> Entry {
    Entry { sheet, heading, frame: Frame::Bare, fg: &[], bg: &[], draw }
}

fn rows(sheet: &'static str, heading: &'static str, draw: fn(&mut Buffer, Rect)) -> Entry {
    Entry { sheet, heading, frame: Frame::Rows, fg: &[], bg: &[], draw }
}

/// A panel with one text row per line, for the area examples.
fn panel_with(buf: &mut Buffer, area: Rect, panel: &Panel<'_>, lines: &[&str]) {
    let inner = panel.render(buf, area);
    for (i, l) in lines.iter().enumerate() {
        Text::new(*l).render(buf, row(inner, cast!(i => u16)));
    }
}

/// The S01 sidebar head: Divider, Text, Text, Spacer, Divider, Labeled Bar.
fn sidebar_head(buf: &mut Buffer, area: Rect) {
    let inner = Panel::new("Status").render(buf, area);
    let (d1, t1, t2) = (Divider::new("Clock"), Text::new(" Year 12  Day 4     ♫ Autumn"), Text::new(" 14:00  ☼ day      ►► x2  running"));
    let (sp, d2, gauge) = (Spacer::rows(1), Divider::new("Resources"), LabeledBar::new(" vegetation", 0.44).color(theme::VEGETATION));
    VStack::new().child(&d1).child(&t1).child(&t2).child(&sp).child(&d2).child(&gauge).render(buf, inner);
}

/// A population row: ` V Vole    267 `, the arrow, four blanks, an 18-cell strip.
fn population_row(buf: &mut Buffer, area: Rect, head: &str, series: &[u16], color: ratatui::style::Color) {
    let (t, arrow, gap, spark) = (Text::new(head), TrendArrow::new(series), Spacer::cols(4), Sparkline::new(series).color(color));
    HStack::new().child_with(Fixed(15), &t).child_with(Fixed(1), &arrow).child(&gap).child_with(Fixed(18), &spark).render(buf, area);
}

/// Twelve days at `start`, then the eighteen listed values.
fn thirty_days(start: u16, last18: &[u16]) -> Vec<u16> {
    let mut v = vec![start; 12];
    v.extend_from_slice(last18);
    v
}

const VOLE: [u16; 18] = [310, 308, 305, 302, 300, 296, 290, 285, 281, 275, 268, 262, 258, 254, 252, 250, 251, 267];
const FOX: [u16; 18] = [20, 20, 20, 20, 20, 20, 30, 30, 30, 40, 40, 40, 40, 50, 50, 50, 50, 35];
const WOLF: [u16; 18] = [20, 20, 20, 24, 24, 24, 24, 28, 28, 28, 32, 32, 32, 32, 32, 28, 28, 27];

fn greeting_with_metadata(buf: &mut Buffer, area: Rect) {
    let inner = Panel::new("Greeting").render(buf, area);
    let (a, d, b) = (Text::new(" Hello!"), Divider::new("Metadata"), Text::new(" Sent Sep 9th, 2026"));
    VStack::new().child(&a).child(&d).child(&b).render(buf, inner);
}

fn panel() -> Vec<Entry> {
    vec![
        bare("panel", "Simple (43 columns)", |b, a| panel_with(b, a, &Panel::new("Greeting"), &[" Hello!"])),
        bare("panel", "With Info (43 columns)", |b, a| {
            panel_with(b, a, &Panel::new("Status").info("Esc closes"), &[" Year 12  Day 4     ♫ Autumn"]);
        }),
        Entry {
            fg: &[(0, 0, theme::BORDER_FOCUS), (42, 2, theme::BORDER_FOCUS)],
            ..bare("panel", "Focus (43 columns)", |b, a| {
                let p = Panel::new("Simulation Controls").info("Esc closes").kind(Kind::Focus);
                panel_with(b, a, &p, &[" state   ► RUNNING      ►► x2"]);
            })
        },
        bare("panel", "Inner (43 columns)", |b, a| {
            panel_with(b, a, &Panel::new("Surroundings").kind(Kind::Inner), &[" tile contents"]);
        }),
        bare("panel", "Nested, Inner inside Outer (43 columns)", |b, a| {
            let inner = Panel::new("Life").render(b, a);
            let nested = Rect::new(inner.x + 1, inner.y, inner.width - 2, 3);
            let p = Panel::new("Surroundings").kind(Kind::Inner);
            panel_with(b, nested, &p, &["~~~~~♠♣\"\"\"\"\"\"\"***,,*,♣♣♠♠♠♠\"\"\"\"\"\"\"\"**"]);
        }),
        bare("panel", "Scrolled, Foot set by Scroll Region (43 columns)", |b, a| {
            let p = Panel::new("Legend").foot("↑3 ↓12");
            panel_with(b, a, &p, &[" ≈ deep water       ~ shallow water", " · sand             . bare dirt"]);
        }),
        bare("panel", "Divider inside a panel (43 columns)", greeting_with_metadata),
    ]
}

fn divider() -> Vec<Entry> {
    vec![
        rows("divider", "In-panel (43 columns)", |b, a| Divider::new("Clock").render(b, a)),
        bare("divider", "In a panel body (43 columns)", greeting_with_metadata),
        rows("divider", "No text (43 columns)", |b, a| Divider::rule().render(b, a)),
        bare("divider", "Bare, 41 columns wide", |b, a| Divider::new("Population, last 240 days").render(b, a)),
        rows("divider", "Text cut at width − 2 (43 columns)", |b, a| {
            Divider::new("Offspring forecast (with an average mate) tonight").render(b, a);
        }),
    ]
}

fn spacer() -> Vec<Entry> {
    vec![
        rows("spacer", "One blank row between sections (43 columns)", |b, a| {
            let (t, s, d) = (Text::new(" 14:00  ☼ day      ►► x2  running"), Spacer::rows(1), Divider::new("Resources"));
            VStack::new().child(&t).child(&s).child(&d).render(b, a);
        }),
    ]
}

fn vstack() -> Vec<Entry> {
    vec![
        bare("vstack", "Content, the S01 sidebar head (43 columns)", sidebar_head),
        bare("vstack", "Clipped, the same stack in a 3-row area (43 columns)", sidebar_head),
    ]
}

fn hstack() -> Vec<Entry> {
    vec![
        bare("hstack", "Split, two Fill(1) panels (43 columns)", |b, a| {
            let inner = Panel::new("Split").render(b, a);
            let (l, r) = (Panel::new("Left").kind(Kind::Inner), Panel::new("Right").kind(Kind::Inner));
            let areas = HStack::new().child_with(Fill(1), &l).child_with(Fill(1), &r).areas(inner);
            panel_with(b, areas[0], &l, &[" a"]);
            panel_with(b, areas[1], &r, &[" b"]);
        }),
        rows("hstack", "Row, a Labeled Bar as cells (43 columns)", |b, a| {
            let (t, gauge, gap, v) = (Text::new(" vegetation"), Bar::new(0.44).color(theme::VEGETATION), Spacer::cols(3), Text::new(" 44%").right());
            HStack::new().child_with(Fixed(12), &t).child_with(Fixed(20), &gauge).child(&gap).child_with(Fixed(4), &v).render(b, a);
        }),
        rows("hstack", "Columns, one Legend row (43 columns)", |b, a| {
            let (l, r) = (Text::new(" ≈ deep water"), Text::new("~ shallow water"));
            HStack::new().child_with(Fixed(20), &l).child_with(Fill(1), &r).render(b, a);
        }),
    ]
}

fn labeled_bar() -> Vec<Entry> {
    vec![
        rows("labeled-bar", "Percent (43 columns)", |b, a| LabeledBar::new(" vegetation", 0.63).color(theme::VEGETATION).render(b, a)),
        Entry {
            fg: &[(11, 1, theme::WARN)],
            ..rows("labeled-bar", "Vital, inverted (43 columns)", |b, a| {
                LabeledBar::vital(" hunger", 0.41, Inverted::Yes).label_and_bar(9, 24).render(b, a);
            })
        },
        rows("labeled-bar", "Count (43 columns)", |b, a| LabeledBar::new(" carcasses", 8.0 / 60.0).value_text("8").render(b, a)),
        rows("labeled-bar", "Suffix (43 columns)", |b, a| {
            LabeledBar::new(" age", 0.99).label_and_bar(5, 14).suffix("848 / 858 days").render(b, a);
        }),
        rows("labeled-bar", "Bare, inside a table row (43 columns)", |b, a| {
            let (t, gauge, v) = (Text::new(" Sunfall Coast"), Bar::new(0.70), Text::new("28").right());
            HStack::new().child_with(Fixed(18), &t).child_with(Fixed(14), &gauge).child_with(Fixed(5), &v).render(b, a);
        }),
        rows("labeled-bar", "Marker (43 columns)", |b, a| {
            let (t, gauge, v) = (Text::new(" Speed  0.80  0.82 "), Bar::new(0.82).marker(0.80), Text::new("  +0.02"));
            HStack::new().child_with(Fixed(19), &t).child_with(Fixed(14), &gauge).child_with(Fill(1), &v).render(b, a);
        }),
    ]
}

fn range_bar() -> Vec<Entry> {
    vec![
        rows("range-bar", "Species range (43 columns)", |b, a| {
            let (t, r) = (Text::new(" Speed       0.75 ±+0.09 "), RangeBar::new(0.62, 0.66, 0.75).width(11));
            HStack::new().child_with(Fixed(25), &t).child_with(Fixed(11), &r).render(b, a);
        }),
        rows("range-bar", "Spread (43 columns)", |b, a| {
            let (t, r) = (Text::new(" Speed       0.82   +0.02  "), RangeBar::new(0.72, 0.82, 0.88));
            HStack::new().child_with(Fixed(27), &t).child_with(Fixed(13), &r).render(b, a);
        }),
        rows("range-bar", "Forecast with Text, real S03 row (52 columns)", |b, a| {
            let (t, r) = (Text::new(" Speed       "), RangeBar::new(0.61, 0.71, 0.81).width(22).text());
            HStack::new().child_with(Fixed(13), &t).child_with(Fill(1), &r).render(b, a);
        }),
        rows("range-bar", "Lineage (43 columns)", |b, a| {
            let (t, r) = (Text::new(" +0.09      "), RangeBar::new(0.66, 0.79, 0.80).width(26));
            HStack::new().child_with(Fixed(12), &t).child_with(Fixed(26), &r).render(b, a);
        }),
        rows("range-bar", "Narrowest, `w` 5 (43 columns)", |b, a| {
            let (t, r) = (Text::new(" Speed "), RangeBar::new(0.2, 0.5, 0.9).width(5));
            HStack::new().child_with(Fixed(7), &t).child_with(Fixed(5), &r).render(b, a);
        }),
    ]
}

fn sparkline() -> Vec<Entry> {
    vec![
        rows("sparkline", "Recent, S01 sidebar row (43 columns)", |b, a| population_row(b, a, " V Vole    267 ", &thirty_days(250, &VOLE), theme::TAN)),
        rows("sparkline", "Flat series, S04 table cell (43 columns)", |b, a| {
            let (t, spark, gap, arrow) = (Text::new(" D Deer    prey     13   "), Sparkline::new(&[13; 14]), Spacer::cols(1), TrendArrow::new(&[30, 27]));
            HStack::new().child_with(Fixed(25), &t).child_with(Fixed(14), &spark).child(&gap).child_with(Fixed(1), &arrow).render(b, a);
        }),
        rows("sparkline", "Fewer values than width (43 columns)", |b, a| {
            population_row(b, a, " V Vole    267 ", &[250, 262, 275, 290, 300, 310, 305, 296, 281, 267], theme::TAN);
        }),
        rows("sparkline", "Wide, prototype S06 row (90 columns)", |b, a| {
            const V: [u16; 44] = [2, 2, 3, 3, 3, 3, 3, 2, 2, 1, 1, 1, 1, 1, 1, 1, 2, 2, 3, 3, 3, 3, 3, 2, 2, 2, 1, 1, 0, 0, 0, 0, 0, 1, 1, 3, 3, 3, 3, 3, 2, 2, 2, 1];
            let (gauge, gap, spark) = (LabeledBar::new(" vegetation", 0.44).color(theme::VEGETATION).label_w(13), Spacer::cols(3), Sparkline::new(&V).color(theme::VEGETATION));
            HStack::new().child_with(Fixed(40), &gauge).child(&gap).child_with(Fixed(44), &spark).render(b, a);
        }),
    ]
}

fn trend_arrow() -> Vec<Entry> {
    vec![
        Entry {
            fg: &[(16, 1, theme::GOOD)],
            ..rows("trend-arrow", "Rising (43 columns)", |b, a| population_row(b, a, " V Vole    267 ", &thirty_days(250, &VOLE), theme::TAN))
        },
        Entry {
            fg: &[(16, 1, theme::DIM)],
            ..rows("trend-arrow", "Flat (43 columns)", |b, a| population_row(b, a, " F Fox      35 ", &thirty_days(35, &FOX), theme::ROSE))
        },
        Entry {
            fg: &[(16, 1, theme::BAD)],
            ..rows("trend-arrow", "Falling (43 columns)", |b, a| population_row(b, a, " W Wolf     27 ", &thirty_days(30, &WOLF), theme::PRED))
        },
        rows("trend-arrow", "With word (43 columns)", |b, a| {
            const V: [u16; 14] = [3, 2, 2, 2, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0];
            let (t, spark, gap, arrow) = (Text::new(" 30-day trend "), Sparkline::new(&V), Spacer::cols(1), TrendArrow::new(&V).word());
            HStack::new().child_with(Fixed(14), &t).child_with(Fixed(14), &spark).child(&gap).child_with(Fixed(11), &arrow).render(b, a);
        }),
    ]
}

fn text() -> Vec<Entry> {
    vec![
        rows("text", "Plain (43 columns)", |b, a| Text::new(" Hello!").render(b, a)),
        rows("text", "Right-aligned (43 columns)", |b, a| Text::new(" 44%").right().render(b, a)),
        rows("text", "Centred (43 columns)", |b, a| Text::new("Enter select   ←→ move   Esc continue").center().render(b, a)),
        rows("text", "Cut at the right edge (43 columns)", |b, a| {
            Text::new("The quick brown fox jumps over the lazy dog and on").render(b, a);
        }),
        Entry {
            fg: &[(1, 1, theme::TEXT), (21, 1, theme::ACCENT)],
            ..rows("text", "Styled spans (43 columns)", |b, a| {
                Text::spans(vec![
                    Span::styled(" Year 12  Day 4", theme::text()),
                    Span::styled("     ♫ Autumn", Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG)),
                ])
                .render(b, a);
            })
        },
        Entry {
            fg: &[(1, 1, theme::DIM)],
            ..rows("text", "Dim (43 columns)", |b, a| Text::new(" tick 5,184").style(theme::dim_text()).render(b, a))
        },
    ]
}

pub fn examples() -> Vec<Entry> {
    let mut v = Vec::new();
    v.extend(panel());
    v.extend(divider());
    v.extend(spacer());
    v.extend(vstack());
    v.extend(hstack());
    v.extend(labeled_bar());
    v.extend(range_bar());
    v.extend(sparkline());
    v.extend(trend_arrow());
    v.extend(text());
    v.sort_by_key(|e| (e.sheet, e.heading));
    v
}
