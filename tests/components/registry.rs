//! One entry per example heading in `docs/components/*.md`. Keep the heading
//! text verbatim, including inline code; `examples_render_cell_for_cell`
//! matches on it.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use sim_fortress::widgets::Constraint::{Fill, Fixed, Min};
use sim_fortress::widgets::{
    util, Align, Bar, Block, ButtonRow, Chart, Checkbox, Column, Columns, Component, Divider, FilterStrip, HStack, Histogram, Inverted, KeyHint, Kind,
    LabeledBar, Legend, Menu, Modal, Panel, RaceChart, RaceSeries, RangeBar, Row, ScrollRegion, Series, Spacer, Sparkline, StatusBar, Stepper, Table, TableCell, TableRow,
    Text, TextField, Ticker, TrendArrow, VStack,
};
use sim_fortress::sim::Params;
use sim_fortress::{cast, glyphs, theme};

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
fn population_row(buf: &mut Buffer, area: Rect, head: &str, series: &[u16], color: Color) {
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

const TAIL_70: &str = "    prey 368  pred 24  ratio 15.3:1   births 0";
const TAIL_155: &str = "    prey 368  pred 24  ratio 15.3:1   births 0  deaths 0   net +0 today     traits = species means x100;  /d = yesterday";

const S00_KEYS: &[(&str, &str)] = &[("↑↓", "select"), ("Enter", "confirm"), ("q", "quit")];
const S01_CLOCK: &str = "Year 12, Day 4 of Autumn  14:00  ☼ day";

fn key_hint() -> Vec<Entry> {
    vec![
        Entry {
            fg: &[(2, 1, theme::DIM)],
            ..rows("key-hint", "In a panel body, S01 sidebar (43 columns)", |b, a| {
                let (gap, hints) = (Spacer::cols(1), KeyHint::row(&[("k", "look"), ("e", "events"), ("g", "charts")]).separator(" · ").dim());
                HStack::new().child(&gap).child_with(Fill(1), &hints).render(b, a);
            })
        },
        Entry {
            fg: &[(2, 1, theme::KEY), (9, 1, theme::DIM)],
            ..rows("key-hint", "Modal hint line, S10 (43 columns)", |b, a| {
                let (gap, hints) = (Spacer::cols(1), KeyHint::row(&[("Space", "pause"), ("Esc", "close")]).label_style(theme::dim_text()));
                HStack::new().child(&gap).child_with(Fill(1), &hints).render(b, a);
            })
        },
        Entry {
            fg: &[(34, 1, theme::KEY)],
            bg: &[(1, 1, theme::SELECT_BG), (34, 1, theme::SELECT_BG)],
            ..rows("key-hint", "Menu, right-aligned on the selected row (43 columns)", |b, a| {
                util::fill(b, a, theme::selected());
                let (label, hint, gap) = (Text::new("   ► New World").style(theme::selected()), KeyHint::key("Enter").bg(theme::SELECT_BG), Spacer::cols(1));
                HStack::new().child_with(Fill(1), &label).child_with(Fixed(7), &hint).child(&gap).render(b, a);
            })
        },
        Entry {
            fg: &[(3, 1, theme::DIM)],
            ..rows("key-hint", "Bracketless, centred, S12 (43 columns)", |b, a| {
                KeyHint::row(&[("Enter", "select"), ("←→", "move"), ("Esc", "continue")]).bracketless().separator("   ").center().render(b, a);
            })
        },
        Entry {
            fg: &[(2, 0, theme::KEY), (38, 0, theme::ACCENT)],
            bg: &[(0, 0, theme::STATUS_BG)],
            ..bare("key-hint", "In the status bar (43 columns)", |b, a| {
                StatusBar::new(&[("k", "look"), ("e", "events"), ("g", "charts")]).right("help").render(b, a);
            })
        },
    ]
}

fn status_bar() -> Vec<Entry> {
    vec![
        bare("status-bar", "Default, from S00a (155 columns)", |b, a| StatusBar::new(S00_KEYS).right("no world loaded").render(b, a)),
        Entry {
            fg: &[(116, 0, theme::TITLE)],
            ..bare("status-bar", "Coloured right, from S01a (155 columns)", |b, a| {
                const KEYS: &[(&str, &str)] =
                    &[("k", "look"), ("o", "overlay"), ("Space", "pause"), ("+/-", "speed"), ("s", "species"), ("g", "graphs"), ("e", "events"), ("?", "help")];
                StatusBar::new(KEYS).right(S01_CLOCK).right_color(theme::TITLE).render(b, a);
            })
        },
        bare("status-bar", "Modal repaint, from S12a (155 columns)", |b, a| {
            const KEYS: &[(&str, &str)] = &[("Enter", "select"), ("←→", "move"), ("l", "lineage"), ("Space", "pause"), ("Esc", "continue")];
            StatusBar::new(KEYS).right(format!("{} paused on extinction", glyphs::PAUSE_STR)).render(b, a);
        }),
        bare("status-bar", "Default, from S03a (155 columns)", |b, a| {
            const KEYS: &[(&str, &str)] = &[("f", "follow"), ("l", "lineage"), ("Tab", "next creature"), ("←→", "panel"), ("↑↓", "scroll"), ("Esc", "back")];
            StatusBar::new(KEYS).right("Sedge d#494  Year 2, Day 1 of Spring  06:00").render(b, a);
        }),
        bare("status-bar", "Short (60 columns)", |b, a| StatusBar::new(S00_KEYS).right("no world loaded").render(b, a)),
        bare("status-bar", "Narrow, pairs dropped [Planned] (60 columns)", |b, a| {
            const KEYS: &[(&str, &str)] = &[
                ("k", "look"),
                ("Tab", "wide"),
                ("←→↑↓", "scroll"),
                ("1-9", "overlay"),
                ("Space", "pause"),
                ("+/-", "speed"),
                ("p", "controls"),
                ("?", "help"),
                ("q", "world"),
            ];
            StatusBar::new(KEYS).right(S01_CLOCK).render(b, a);
        }),
    ]
}

fn ticker() -> Vec<Entry> {
    vec![
        Entry {
            fg: &[(1, 0, theme::TEXT), (46, 0, theme::DIM)],
            bg: &[(0, 0, theme::BG)],
            ..bare("ticker", "Note, from S01a row 44 (155 columns)", |b, a| {
                Ticker::new(Some((glyphs::NOTE, theme::TEXT, "Ashfang w#042 is stalking Bramble h#217"))).render(b, a);
            })
        },
        Entry {
            fg: &[(1, 0, theme::BAD)],
            ..bare("ticker", "Death by predation (155 columns)", |b, a| {
                Ticker::new(Some((glyphs::DEATH, theme::BAD, "Thistle d#133 was killed by Ashfang w#042 in the Fenlands"))).render(b, a);
            })
        },
        Entry {
            fg: &[(1, 0, theme::GOOD)],
            ..bare("ticker", "Cut at width, Pointer lost (43 columns)", |b, a| {
                Ticker::new(Some((glyphs::BIRTH, theme::GOOD, "Clover v#031 gave birth to 4 young in Reedmace Hollow"))).render(b, a);
            })
        },
        Entry { bg: &[(0, 0, theme::BG), (42, 0, theme::BG)], ..bare("ticker", "Empty (43 columns)", |b, a| Ticker::new(None).render(b, a)) },
    ]
}

/// The S01 Population columns: ` V `, name, count, gap, arrow, gap, strip, rest.
fn population_columns() -> Columns {
    Columns::new(&[Fixed(3), Fixed(6), Min(5), Fixed(1), Fixed(1), Fixed(4), Fixed(18), Fill(1)]).align(2, Align::Right)
}

fn population_cells(glyph: &'static str, name: &'static str, count: &'static str, series: &[u16], color: Color) -> Row<'static> {
    Row::new()
        .cell(Text::new(glyph).fg(color).bold())
        .cell(Text::new(name))
        .cell(Text::new(count))
        .cell(Spacer::cols(1))
        .cell(TrendArrow::new(series))
        .cell(Spacer::cols(4))
        .cell(Sparkline::new(series).color(color))
        .cell(Spacer::cols(0))
}

/// Thirty days of deer: flat at 4133 (`↔`), the strip falling to it.
fn deer_series() -> Vec<u16> {
    let mut v = vec![4133u16; 12];
    v.extend([4433, 4433, 4333, 4333, 4333, 4333]);
    v.extend([4233; 9]);
    v.extend([4133; 3]);
    v
}

fn hare_series() -> Vec<u16> {
    let mut v = vec![1u16; 26];
    v.extend([0; 4]);
    v
}

fn columns() -> Vec<Entry> {
    vec![
        rows("columns", "Fixed, the S01 Population rows (43 columns)", |b, a| {
            let zeros = [0u16; 30];
            let rows = [
                Row::span(Divider::new("Population")),
                population_cells(" V ", "Vole", "0", &zeros, theme::TAN),
                population_cells(" H ", "Hare", "0", &hare_series(), theme::CREAM),
                population_cells(" D ", "Deer", "4133", &deer_series(), theme::ROSE),
                population_cells(" F ", "Fox", "0", &zeros, theme::WARN),
                population_cells(" W ", "Wolf", "0", &zeros, theme::PRED),
                population_cells(" L ", "Lynx", "0", &zeros, theme::MAGENTA),
            ];
            Block::new(population_columns()).rows(rows).render(b, a);
        }),
        rows("columns", "Measured, one count wider than the floor (43 columns)", |b, a| {
            let zeros = [0u16; 30];
            let rows = [population_cells(" V ", "Vole", "0", &zeros, theme::TAN), population_cells(" D ", "Deer", "413300", &deer_series(), theme::ROSE)];
            Block::new(population_columns()).rows(rows).render(b, a);
        }),
        rows("columns", "Grid, two Legend columns (43 columns)", |b, a| {
            let rows = [
                Row::new().cell(Text::new(" ≈ deep water")).cell(Text::new("~ shallow water")),
                Row::new().cell(Text::new(" · sand")).cell(Text::new(". bare dirt")),
            ];
            Block::new(Columns::new(&[Fixed(20), Fill(1)])).rows(rows).render(b, a);
        }),
    ]
}

/// The S04a species table columns, Marker excluded.
const SPECIES_COLUMNS: [Column; 26] = [
    Column::new(Fixed(2)),
    Column::titled("Species", Fixed(8)),
    Column::titled("Kind", Fixed(6)),
    Column::titled("Count", Fixed(5)).right(),
    Column::titled("Adults", Fixed(7)).right(),
    Column::titled("Juv", Fixed(6)).right(),
    Column::titled("Birth/d", Fixed(8)).right(),
    Column::titled("Death/d", Fixed(8)).right(),
    Column::titled("Sick", Fixed(6)).right(),
    Column::titled("Peak", Fixed(6)).right(),
    Column::titled("Gen", Fixed(5)).right(),
    Column::new(Fixed(2)),
    Column::titled("30-day trend", Fixed(15)),
    Column::new(Fixed(1)),
    Column::new(Fixed(1)),
    Column::titled("Spd", Fixed(6)).right(),
    Column::titled("Siz", Fixed(4)).right(),
    Column::titled("Sen", Fixed(4)).right(),
    Column::titled("Met", Fixed(4)).right(),
    Column::titled("Agg", Fixed(4)).right(),
    Column::titled("Cam", Fixed(4)).right(),
    Column::titled("Fer", Fixed(4)).right(),
    Column::titled("Lon", Fixed(4)).right(),
    Column::titled("Res", Fixed(4)).right(),
    Column::titled("Soc", Fixed(4)).right(),
    Column::titled("Mat", Fixed(4)).right(),
];

/// A species row up to the Gen column.
fn species_row(glyph: char, name: &'static str, kind: &'static str, nums: [&'static str; 8]) -> TableRow<'static> {
    let mut cells = vec![TableCell::Glyph(glyph, theme::TAN), TableCell::text(name), TableCell::dim(kind)];
    cells.extend(nums.into_iter().map(TableCell::text));
    TableRow::new(cells)
}

fn species_totals(tail: &'static str) -> TableRow<'static> {
    TableRow::new([TableCell::Blank, TableCell::text("totals"), TableCell::Blank, TableCell::text("392")]).tail(Text::new(tail))
}

fn species_rows() -> [TableRow<'static>; 6] {
    [
        species_row('H', "Hare", "prey", ["201", "201", "0", "0", "1", "0", "703", "4"]),
        species_row('V', "Vole", "prey", ["154", "154", "0", "0", "2", "0", "544", "6"]),
        species_row('D', "Deer", "prey", ["13", "11", "2", "0", "0", "0", "230", "2"]),
        species_row('F', "Fox", "pred", ["10", "10", "0", "0", "0", "0", "11", "4"]),
        species_row('W', "Wolf", "pred", ["8", "7", "1", "0", "0", "0", "18", "3"]),
        species_row('L', "Lynx", "pred", ["6", "6", "0", "0", "0", "0", "7", "3"]),
    ]
}

fn table() -> Vec<Entry> {
    vec![
        bare("table", "Species table cut after Gen, Spaced with totals (70 columns)", |b, a| {
            let inner = Panel::new("Species").info(Table::sort_info("count")).render(b, a);
            let rows = species_rows();
            Table::new(&SPECIES_COLUMNS, &rows[..]).selected(Some(0)).totals(species_totals(TAIL_70)).spaced(true).render(b, inner);
        }),
        bare("table", "Full width, header, selected row and totals (155 columns)", |b, a| {
            const STRIP: [u16; 14] = [3, 3, 3, 2, 2, 2, 2, 2, 2, 1, 1, 1, 0, 0];
            let inner = Panel::new("Species").info(Table::sort_info("count")).render(b, a);
            let (gap, spark) = (Spacer::cols(1), Sparkline::new(&STRIP).color(theme::TAN));
            let trend = HStack::new().child(&gap).child_with(Fixed(14), &spark);
            let mut cols = SPECIES_COLUMNS.to_vec();
            cols.extend([Column::new(Fixed(3)), Column::titled(" Diet", Fill(1))]);
            let [mut hare, ..] = species_rows();
            hare.cells.extend([TableCell::Blank, TableCell::widget(trend), TableCell::Blank, TableCell::widget(TrendArrow::new(&STRIP).bold())]);
            let traits = ["82", "24", "65", "61", "10", "56", "75", "35", "35", "26", "51"];
            let colors = [theme::INFO, theme::TAN, theme::ACCENT, theme::WARN, theme::BAD, theme::VEGETATION, theme::MAGENTA, theme::ROSE, theme::SICK, theme::CREAM, theme::SEED];
            hare.cells.extend(traits.iter().zip(colors).map(|(t, c)| TableCell::styled(*t, c)));
            hare.cells.extend([TableCell::Blank, TableCell::dim("grass, bark")]);
            let rows = [hare];
            Table::new(&cols, &rows[..]).selected(Some(0)).totals(species_totals(TAIL_155)).spaced(true).render(b, inner);
        }),
        rows("table", "Marker and Bare bars, S06 regions (63 columns)", |b, a| {
            const COLS: [Column; 5] = [
                Column::titled("region", Fixed(18)),
                Column::titled("cells", Fixed(5)).right(),
                Column::titled("water", Fixed(8)).right(),
                Column::new(Fixed(3)),
                Column::titled("  vegetation", Fixed(26)),
            ];
            let (b1, t1, b2, t2) = (Bar::new(0.32).color(theme::VEGETATION), Text::new(" 0.32"), Bar::new(0.49).color(theme::VEGETATION), Text::new(" 0.49"));
            let veg1 = HStack::new().child_with(Fixed(20), &b1).child_with(Fill(1), &t1);
            let veg2 = HStack::new().child_with(Fixed(20), &b2).child_with(Fill(1), &t2);
            let rows = [
                TableRow::new([TableCell::text("Northmarch"), TableCell::text("700"), TableCell::text("18%"), TableCell::Blank, TableCell::widget(veg1)]),
                TableRow::new([TableCell::text("Ashen Ridge"), TableCell::text("600"), TableCell::text("12%"), TableCell::Blank, TableCell::widget(veg2)]),
            ];
            Table::new(&COLS, &rows[..]).selected(Some(1)).render(b, a);
        }),
    ]
}

/// The twelve map entries the sheets were drawn with (before Marsh was added).
const MAP_ENTRIES: [(char, Color, &str); 12] = [
    (glyphs::DEEP_WATER, theme::DEEP_WATER_FG, "deep water"),
    (glyphs::SHALLOW_WATER, theme::SHALLOW_FG, "shallow water"),
    (glyphs::SAND, theme::SAND_FG, "sand"),
    (glyphs::DIRT, theme::DIRT_FG, "bare dirt"),
    (glyphs::GRASS_SPARSE, theme::GRASS_SPARSE_FG, "sparse grass"),
    (glyphs::GRASS, theme::GRASS_FG, "grassland"),
    (glyphs::GRASS_DENSE, theme::GRASS_DENSE_FG, "meadow"),
    (glyphs::FOREST, theme::FOREST_FG, "forest"),
    (glyphs::ROCK, theme::ROCK_FG, "rock"),
    (glyphs::DEN, theme::DEN, "den / burrow"),
    (glyphs::CARCASS, theme::CARCASS, "carcass"),
    (glyphs::SEED, theme::SEED, "regrowth"),
];

const TERRAIN_NOTES: [&str; 3] = ["impassable, drinkable", "drinkable, slow", "no forage"];

/// The nine-row S01 legend: twelve entries in two columns, six species, Footer.
fn sidebar_legend() -> Legend<'static> {
    Legend::new(&MAP_ENTRIES).species(&Params::default().species)
}

/// A `║ … │` row frame: the modal's left border and the column rule.
fn help_frame(buf: &mut Buffer, area: Rect) -> Rect {
    for y in area.top()..area.bottom() {
        buf.set_stringn(area.x, y, "║", 1, theme::border());
        buf.set_stringn(area.right() - 1, y, glyphs::V_LINE.to_string(), 1, theme::border());
    }
    Rect::new(area.x + 1, area.y, area.width - 2, area.height)
}

fn legend() -> Vec<Entry> {
    vec![
        rows("legend", "Sidebar, S01 rows (43 columns)", |b, a| sidebar_legend().render(b, a)),
        rows("legend", "Sidebar under its Divider (43 columns)", |b, a| {
            let (d, l) = (Divider::new("Legend"), sidebar_legend());
            VStack::new().child(&d).child(&l).render(b, a);
        }),
        Entry {
            fg: &[(2, 0, theme::DEEP_WATER_FG), (18, 0, theme::DIM)],
            ..bare("legend", "Help, one column with notes (40 columns)", |b, a| {
                let inner = help_frame(b, a);
                Legend::new(&MAP_ENTRIES[..3]).columns(1).label_w(14).notes(&TERRAIN_NOTES).render(b, inner);
            })
        },
        bare("legend", "Creatures, S11 column (40 columns)", |b, a| {
            for y in a.top()..a.bottom() {
                b.set_stringn(a.x, y, glyphs::V_LINE.to_string(), 1, theme::border());
                b.set_stringn(a.right() - 1, y, glyphs::V_LINE.to_string(), 1, theme::border());
            }
            let inner = Rect::new(a.x + 1, a.y, a.width - 2, a.height);
            let roster = Params::default().species;
            let legend = Legend::creatures(&roster);
            // The sheet shows the header, the first two species and the Footer.
            let (h, f) = (legend.height(inner.width), inner.height);
            let stack = ScrollRegion::new(&legend);
            stack.render(b, Rect { height: f - 1, ..inner });
            let footer = ScrollRegion::new(&legend).offset(h - 1);
            footer.render(b, Rect::new(inner.x, inner.bottom() - 1, inner.width, 1));
        }),
    ]
}

/// A titled Legend panel showing a two-row window of `legend` at `offset`.
fn scrolled(buf: &mut Buffer, area: Rect, legend: &Legend<'_>, offset: u16) {
    let region = ScrollRegion::new(legend).offset(offset);
    let ov = region.overflow(Panel::inner(area));
    let inner = Panel::new("Legend").foot(ov.foot().unwrap_or_default()).render(buf, area);
    region.render(buf, inner);
}

fn scroll_region() -> Vec<Entry> {
    vec![
        bare("scroll-region", "Fits, no Foot (43 columns)", |b, a| scrolled(b, a, &Legend::new(&MAP_ENTRIES[..4]), 0)),
        bare("scroll-region", "Top, offset 0 (43 columns)", |b, a| scrolled(b, a, &sidebar_legend(), 0)),
        bare("scroll-region", "Middle, offset 3 (43 columns)", |b, a| scrolled(b, a, &sidebar_legend(), 3)),
        bare("scroll-region", "Bottom, offset 7 (43 columns)", |b, a| scrolled(b, a, &sidebar_legend(), 7)),
    ]
}

fn button_row() -> Vec<Entry> {
    vec![
        Entry {
            bg: &[(9, 1, theme::SELECT_BG)],
            ..rows("button-row", "Centred, the S12 extinction alert (64 columns)", |b, a| {
                ButtonRow::new(&["[ Continue ]", "[ View lineage ]", "[ Pause ]"]).focused(Some(0)).render(b, a);
            })
        },
        rows("button-row", "Centred, the confirm dialog (43 columns)", |b, a| ButtonRow::new(&["[ Yes ]", "[ No ]"]).focused(Some(0)).render(b, a)),
        Entry {
            bg: &[(5, 1, theme::WARN)],
            ..rows("button-row", "Left with Primary, the S09 form foot (66 columns)", |b, a| {
                ButtonRow::new(&["[ Generate ]", "[ Randomize seed ]", "[ Back ]"]).focused(None).primary(0).left(4).gap(3).render(b, a);
            })
        },
    ]
}

/// Draw `lines` as text rows from the top of `body`.
fn body_lines(buf: &mut Buffer, body: Rect, lines: &[&str]) {
    for (i, l) in lines.iter().enumerate() {
        if let Some(t) = l.strip_prefix('^') {
            Text::new(t).center().render(buf, row(body, cast!(i => u16)));
        } else {
            Text::new(*l).render(buf, row(body, cast!(i => u16)));
        }
    }
}

fn modal() -> Vec<Entry> {
    vec![
        Entry {
            fg: &[(0, 0, theme::BORDER_FOCUS)],
            ..bare("modal", "Simple (43 columns)", |b, a| {
                let modal = Modal::new(43, 6).title("Greeting").buttons(&["[ OK ]", "[ Cancel ]"], Some(0)).hint("Enter select   Esc close");
                let body = modal.render(b, a);
                body_lines(b, body, &[" Hello!"]);
            })
        },
        bare("modal", "Confirm, untitled (50 columns)", |b, a| {
            let modal = Modal::new(50, 7).buttons(&["[ Yes ]", "[ No ]"], Some(0)).hint("←→ move  Enter select  Esc no");
            let body = modal.render(b, a);
            body_lines(b, body, &["", "World has unsaved changes. Return to title?"]);
        }),
        Entry {
            fg: &[(24, 0, theme::MAGENTA)],
            ..bare("modal", "Alert, Banner (64 columns)", |b, a| {
                let modal = Modal::new(64, 12)
                    .banner(format!("{} EXTINCTION {}", glyphs::EXTINCTION, glyphs::EXTINCTION), theme::MAGENTA)
                    .buttons(&["[ Continue ]", "[ View lineage ]", "[ Pause ]"], Some(0))
                    .hint("Enter select   ←→ move   Esc continue");
                let body = modal.render(b, a);
                body_lines(
                    b,
                    body,
                    &[
                        "",
                        "^The Lynx are extinct",
                        "",
                        "  Year 12, Day 4 of Autumn ♫   last individual: Gloam l#088",
                        "  x starved in Sunfall Coast at (131, 9), age 412 days",
                        "",
                        "  peak population 32   generations survived 19   years 12",
                    ],
                );
            })
        },
        bare("modal", "Controls, titled with Info and Hint only (60 columns)", |b, a| {
            let hint = KeyHint::row(&[("Space", "pause"), ("+/-", "speed"), (".", "step"), ("Esc", "close")]).label_style(theme::dim_text()).center();
            let modal = Modal::new(60, 18).title("Simulation Controls").info("Esc closes").hint_with(hint);
            let body = modal.render(b, a);
            let rows: Vec<Box<dyn Component>> = vec![
                Box::new(Text::new(" state   ► RUNNING      ►► x2  ││ Space toggles")),
                Box::new(Spacer::rows(1)),
                Box::new(Text::new(" speed    x1   x2   x5   x10   x25     +/- or 1-5")),
                Box::new(Text::new(" step     1 tick   6 hours   1 day     →│ . steps once")),
                Box::new(Spacer::rows(1)),
                Box::new(Divider::new("Clock")),
                Box::new(Text::new(" tick 1,064,772    day 4 of Autumn ♫    year 12    14:00 ☼")),
                Box::new(Text::new(" 1 tick = 1 hour   1 day = 24 ticks   x2 = 4 ticks/s")),
                Box::new(Spacer::rows(1)),
                Box::new(Divider::new("Options")),
                Box::new(Checkbox::new("auto-pause on extinction", true).key("a")),
                Box::new(Checkbox::new("log births to the event log", false).key("b")),
                Box::new(Checkbox::new("pause when a followed creature dies", true).key("c")),
            ];
            VStack::from_boxes(&rows).render(b, body);
        }),
    ]
}

fn menu() -> Vec<Entry> {
    vec![
        Entry {
            fg: &[(25, 2, theme::KEY)],
            bg: &[(1, 2, theme::SELECT_BG), (32, 2, theme::SELECT_BG)],
            ..bare("menu", "Marker, the S00 main menu (34 columns)", |b, a| {
                let inner = Panel::new("Main Menu").kind(Kind::Focus).render(b, a);
                let menu = Menu::new(&["New World", "Load World", "Options", "Quit"]).selected(0).hint("[Enter]");
                menu.render(b, Rect::new(inner.x, inner.y + 1, inner.width, 4));
            })
        },
        Entry {
            fg: &[(6, 2, theme::DIM)],
            ..bare("menu", "Marker with a disabled row (43 columns)", |b, a| {
                let inner = Panel::new("Main Menu").kind(Kind::Focus).render(b, a);
                Menu::new(&["New World", "Load World", "Options", "Quit"]).selected(0).disabled(&[1]).hint("[Enter]").note("(empty)").render(b, inner);
            })
        },
        Entry {
            bg: &[(1, 1, theme::SELECT_BG), (67, 1, theme::SELECT_BG)],
            ..bare("menu", "Row highlight, the Load World list (70 columns)", |b, a| {
                let inner = Panel::new("Load World").kind(Kind::Focus).info("3 saves  ·  saved 2026-09-14 21:12").render(b, a);
                let rows = [
                    "The Valley of Sunfall   Year 12, Day 4    587 prey / 79 pred",
                    "Ashen Hollow            Year 3, Day 117  402 prey / 51 pred",
                    "Reedwater               Year 1, Day 9    310 prey / 40 pred  v2",
                ];
                Menu::new(&rows).selected(0).marker(false).render(b, inner);
            })
        },
    ]
}

/// The S09 species row: ` V Voles     prey     ◄  240 ►   [bar]  42%`.
fn species_stepper_row(buf: &mut Buffer, area: Rect, head: &str, count: &str, share: f32, pct: &str) {
    let (t, st, gap, gauge, v) = (Text::new(head), Stepper::compact(count), Spacer::cols(3), Bar::new(share), Text::new(pct).right());
    HStack::new().child_with(Fixed(22), &t).child_with(Fixed(8), &st).child(&gap).child_with(Fixed(22), &gauge).child_with(Fixed(5), &v).render(buf, area);
}

fn stepper() -> Vec<Entry> {
    vec![
        rows("stepper", "Numeric, the S09 form (66 columns)", |b, a| {
            let (w, s) = (Stepper::new("Water %", "20").hint("lakes + rivers"), Stepper::new("Season length", "90 days").hint("30 - 180 days"));
            VStack::new().child(&w).child(&s).render(b, a);
        }),
        rows("stepper", "Choice (66 columns)", |b, a| Stepper::new("Rainfall", "normal").hint("dry/normal/wet").render(b, a)),
        Entry {
            fg: &[(2, 1, theme::ACCENT), (22, 1, theme::KEY)],
            bg: &[(24, 1, theme::SELECT_BG)],
            ..rows("stepper", "Focused (66 columns)", |b, a| Stepper::new("Map width", "150").hint("100 - 1000 cells").focused(true).render(b, a))
        },
        rows("stepper", "Typing (66 columns)", |b, a| Stepper::new("Water %", "20").hint("lakes + rivers").typing(Some("15")).render(b, a)),
        rows("stepper", "Compact, inside the species table (66 columns)", |b, a| {
            species_stepper_row(b, row(a, 0), " V Voles     prey     ", "240", 0.42, "42%");
            species_stepper_row(b, row(a, 1), " F Foxes     predator ", "30", 0.05, "5%");
        }),
        rows("stepper", "Inline, the S10 autosave row (60 columns)", |b, a| {
            let (gap, days, off) = (Spacer::cols(2), Stepper::inline("autosave every 7 days").key("[←→]"), Stepper::inline("autosave every off").key("[←→]"));
            HStack::new().child(&gap).child_with(Fill(1), &days).render(b, row(a, 0));
            HStack::new().child(&gap).child_with(Fill(1), &off).render(b, row(a, 1));
        }),
        rows("stepper", "Narrow, `label_w` 8 and `box_w` 14 (43 columns)", |b, a| {
            Stepper::new("Water %", "20").hint("lakes + rivers").label_w(8).box_w(14).render(b, a);
        }),
    ]
}

fn text_field() -> Vec<Entry> {
    vec![
        Entry {
            bg: &[(24, 1, theme::BG)],
            ..rows("text-field", "Plain, the S09 form (66 columns)", |b, a| {
                let (n, s) = (TextField::new("World name", "The Valley of Sunfall").hint("text"), TextField::new("Seed", "0xC0FFEE").hint("hex/decimal"));
                VStack::new().child(&n).child(&s).render(b, a);
            })
        },
        Entry {
            fg: &[(22, 1, theme::KEY)],
            bg: &[(24, 1, theme::SELECT_BG)],
            ..rows("text-field", "Focused (66 columns)", |b, a| TextField::new("Seed", "0xC0FFEE").hint("hex/decimal").focused(true).render(b, a))
        },
        rows("text-field", "After the first key (66 columns)", |b, a| TextField::new("World name", "T").hint("text").focused(true).render(b, a)),
        rows("text-field", "Narrow, `label_w` 8 and `box_w` 14 (43 columns)", |b, a| {
            TextField::new("Seed", "0xC0FFEE").hint("hex/decimal").label_w(8).box_w(14).render(b, a);
        }),
    ]
}

fn checkbox() -> Vec<Entry> {
    vec![
        Entry {
            fg: &[(2, 1, theme::GOOD), (42, 1, theme::KEY)],
            ..rows("checkbox", "Toggle, the S10 Options section (60 columns)", |b, a| {
                let (x, y, z) = (
                    Checkbox::new("auto-pause on extinction", true).key("a"),
                    Checkbox::new("log births to the event log", false).key("b"),
                    Checkbox::new("pause when a followed creature dies", true).key("c"),
                );
                VStack::new().child(&x).child(&y).child(&z).render(b, a);
            })
        },
        rows("checkbox", "Cycle (60 columns)", |b, a| {
            let (t, d) = (Checkbox::new("day/night tint: map", true).key("t"), Checkbox::new("auto-pause on epidemic", true).key("d"));
            VStack::new().child(&t).child(&d).render(b, a);
        }),
        rows("checkbox", "Narrow, `label_w` 32 (43 columns)", |b, a| {
            let (x, y, z) = (
                Checkbox::new("auto-pause on extinction", true).key("a").label_w(32),
                Checkbox::new("log births to the event log", false).key("b").label_w(32),
                Checkbox::new("day/night tint: map", true).key("t").label_w(32),
            );
            VStack::new().child(&x).child(&y).child(&z).render(b, a);
        }),
        Entry {
            fg: &[(3, 3, theme::GOOD), (32, 4, theme::DIM)],
            ..rows("checkbox", "Radio, the S14 Base heatmap tab (34 columns)", |b, a| {
                let row = |label, on| Checkbox::new(label, on).radio(true).label_w(11).indent(1);
                let (n, v, m, s) = (row("None", false), row("Vegetation", false), row("Moisture", true), row("Species", false).value("hare").cue());
                VStack::new().child(&n).child(&v).child(&m).child(&s).render(b, a);
            })
        },
        Entry {
            fg: &[(3, 1, theme::DIM), (32, 3, theme::ACCENT)],
            bg: &[(1, 3, theme::SELECT_BG), (32, 3, theme::SELECT_BG), (1, 2, theme::PANEL_BG)],
            ..rows("checkbox", "Focused (34 columns)", |b, a| {
                let row = |label, on| Checkbox::new(label, on).label_w(11).indent(1);
                let (s, r, d) = (row("Sense", false).value("no predators").cue().disabled(true), row("Regions", true), row("Disease", true).value("All pathogens").cue().focused(true));
                VStack::new().child(&s).child(&r).child(&d).render(b, a);
            })
        },
    ]
}

const CHIPS: [(char, &str); 9] = [
    ('*', "all"),
    (glyphs::BIRTH, "births"),
    (glyphs::DEATH, "deaths"),
    (glyphs::MUTATION, "mutations"),
    (glyphs::MIGRATION, "migrations"),
    (glyphs::EXTINCTION, "extinctions"),
    (glyphs::DROUGHT, "droughts"),
    (glyphs::DISEASE, "disease"),
    (glyphs::ALERT, "wary"),
];
const CHIP_COLORS: [Color; 9] = [theme::KEY, theme::GOOD, theme::BAD, theme::INFO, theme::ACCENT, theme::MAGENTA, theme::WARN, theme::SICK, theme::WARN];
const HINT: &str = "[f] cycles, [1-9] toggles";

fn filter_strip() -> Vec<Entry> {
    vec![
        Entry {
            bg: &[(3, 1, theme::SELECT_BG), (10, 1, theme::PANEL_BG)],
            ..rows("filter-strip", "All (121 columns)", |b, a| {
                FilterStrip::new(&CHIPS).active(&[true]).colors(&CHIP_COLORS).hint(HINT).render(b, a);
            })
        },
        Entry {
            bg: &[(3, 1, theme::PANEL_BG), (19, 1, theme::SELECT_BG), (53, 1, theme::SELECT_BG)],
            ..rows("filter-strip", "Filtered (121 columns)", |b, a| {
                FilterStrip::new(&CHIPS).active(&[false, false, true, false, false, true]).colors(&CHIP_COLORS).hint(HINT).render(b, a);
            })
        },
        rows("filter-strip", "Hint dropped (95 columns)", |b, a| {
            FilterStrip::new(&CHIPS).active(&[true]).colors(&CHIP_COLORS).hint(HINT).render(b, a);
        }),
    ]
}

/// A 24-cell S04b trait block at one cell of left margin.
fn trait_block(buf: &mut Buffer, area: Rect, buckets: &[u16], header: (&str, f32, f32, f32)) {
    let (name, min, mean, max) = header;
    Histogram::new(buckets).color(theme::INFO).header(name, min, mean, max).mark(mean).footer().render(buf, Rect::new(area.x + 1, area.y, 24, 7));
}

fn histogram() -> Vec<Entry> {
    vec![
        bare("histogram", "Trait, one bucket (43 columns)", |b, a| {
            let inner = Panel::new("Speed").render(b, a);
            trait_block(b, inner, &[0, 0, 0, 0, 0, 0, 0, 0, 10, 0, 0, 0], ("Speed", 0.67, 0.72, 0.73));
        }),
        rows("histogram", "Trait, half cells (43 columns)", |b, a| trait_block(b, a, &[0, 0, 0, 0, 0, 1, 5, 1, 3, 0, 0, 0], ("Metabolism", 0.45, 0.60, 0.68))),
        rows("histogram", "Trait, two buckets (43 columns)", |b, a| trait_block(b, a, &[0, 0, 0, 4, 0, 6, 0, 0, 0, 0, 0, 0], ("Size", 0.31, 0.39, 0.50))),
        bare("histogram", "Bare, `col_w` 4 (24 columns)", |b, a| Histogram::new(&[5, 3, 1, 0, 2, 1]).col_w(4).rows(3).color(theme::TAN).render(b, a)),
    ]
}

fn chart() -> Vec<Entry> {
    vec![rows("chart", "Empty, live axis rule (43 columns)", |b, a| {
        Chart::new().series(Series::new(&[]).color(theme::PREY)).y_step(100.0).render(b, a);
    })]
}

fn race_chart() -> Vec<Entry> {
    vec![
        bare("race-chart", "Lead and three rivals (35 columns)", |b, a| {
            let lead = [4.0, 13.0, 27.0, 45.0, 67.0, 92.0, 113.0, 139.0, 169.0, 202.0, 240.0, 243.0];
            let r1 = [3.0, 9.0, 17.0, 24.0, 33.0, 41.0, 52.0, 66.0, 79.0, 90.0, 99.0, 102.0];
            let r2 = [0.0, 6.0, 15.0, 22.0, 31.0, 40.0, 49.0, 61.0, 70.0, 81.0, 88.0, 89.0];
            let r3 = [2.0, 8.0, 12.0, 19.0, 30.0, 44.0, 56.0, 63.0, 71.0, 78.0, 85.0, 87.0];
            RaceChart::new("kills")
                .series(RaceSeries::new(&r1).color(theme::TAN))
                .series(RaceSeries::new(&r2).color(theme::ROSE))
                .series(RaceSeries::new(&r3).color(theme::PREY))
                .series(RaceSeries::new(&lead).color(theme::PRED).lead(true))
                .rows(5)
                .render(b, a);
        }),
        bare("race-chart", "Rising and falling steps, a late start (21 columns)", |b, a| {
            RaceChart::new("young")
                .series(RaceSeries::new(&[1.0, 4.0, 4.0, 2.0, 6.0, 6.0, 3.0]).color(theme::PRED).lead(true))
                .series(RaceSeries::new(&[5.0, 5.0, 2.0, 2.0, 1.0]).start(2).color(theme::TAN))
                .rows(5)
                .render(b, a);
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
    v.extend(key_hint());
    v.extend(status_bar());
    v.extend(ticker());
    v.extend(columns());
    v.extend(table());
    v.extend(legend());
    v.extend(scroll_region());
    v.extend(button_row());
    v.extend(modal());
    v.extend(menu());
    v.extend(stepper());
    v.extend(text_field());
    v.extend(checkbox());
    v.extend(filter_strip());
    v.extend(histogram());
    v.extend(chart());
    v.extend(race_chart());
    v.extend(text());
    v.sort_by_key(|e| (e.sheet, e.heading));
    v
}
