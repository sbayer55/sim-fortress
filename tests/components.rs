//! Pinned renders of every example in `docs/components/*.md`.
//!
//! The example fences in the sheets are the fixtures: each registered example
//! is rendered at the width its heading declares and compared cell for cell.
//! A sheet whose component has not landed yet is listed in `PENDING_SHEETS`;
//! an example that cannot be reproduced (the fixture data no longer exists)
//! is listed in `UNREPRODUCIBLE` with the reason. Everything else must be in
//! the registry, so a new example fails until it is registered.

// Test crates are separate compilation roots, so they do not inherit the allow
// list in `src/lib.rs`. Indices here come from checked loops over buffers and
// fixture rows; unwrap/expect/panic are on fixture files and headings whose
// absence is itself the failure being reported.
#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim_fortress::cast;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use sim_fortress::widgets::Panel;

#[path = "components/registry.rs"]
pub mod registry;

/// Sheets whose component has not shipped yet. Shrinks to empty by the end of
/// the "builders beside helpers" migration step.
const PENDING_SHEETS: &[&str] = &[
    "button-row",
    "chart",
    "checkbox",
    "columns",
    "filter-strip",
    "histogram",
    "hstack",
    "key-hint",
    "labeled-bar",
    "legend",
    "menu",
    "modal",
    "range-bar",
    "scroll-region",
    "sparkline",
    "status-bar",
    "stepper",
    "table",
    "text-field",
    "ticker",
    "trend-arrow",
    "vstack",
];

/// Examples no test can draw, with the reason. Each must still exist in its sheet.
const UNREPRODUCIBLE: &[(&str, &str, &str)] = &[
    ("chart", "Time, upper chart (63 columns)", "cut from S05a; its 240-day fixture series no longer exists"),
    ("chart", "Time, Annotation and Now, columns 63 to 112 of S05a (50 columns)", "cut from S05a; fixture series gone"),
    ("chart", "Stacked, top and bottom rows (61 columns)", "cut from S05c; StackedChart stays in S05"),
    ("modal", "Scrolling", "no fence; the sheet points at ../screens/renders/S11a.txt"),
];

/// Sheets that are not component sheets.
const NOT_SHEETS: &[&str] = &["README", "_template", "keys"];

/// How a fixture relates to the buffer the closure draws into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    /// The closure draws everything, including any border.
    Bare,
    /// `║ … ║` rows cut from a panel: the harness draws an untitled panel
    /// around the rows and hands the closure the inner area.
    Rows,
}

#[derive(Debug)]
pub struct Entry {
    pub sheet: &'static str,
    pub heading: &'static str,
    pub frame: Frame,
    /// Cells whose foreground must match, in fixture coordinates.
    pub fg: &'static [(u16, u16, Color)],
    /// Cells whose background must match, in fixture coordinates.
    pub bg: &'static [(u16, u16, Color)],
    pub draw: fn(&mut Buffer, Rect),
}

struct Example {
    heading: String,
    width: Option<u16>,
    lines: Vec<String>,
}

struct Sheet {
    name: String,
    headings: Vec<String>,
    examples: Vec<Example>,
}

fn sheets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("docs").join("components")
}

fn sheets() -> Vec<Sheet> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(sheets_dir())
        .expect("docs/components exists")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    paths.sort();
    paths
        .iter()
        .filter(|p| !NOT_SHEETS.contains(&p.file_stem().unwrap().to_str().unwrap()))
        .map(|p| parse_sheet(p))
        .collect()
}

fn parse_sheet(path: &Path) -> Sheet {
    let text = std::fs::read_to_string(path).unwrap();
    let name = path.file_stem().unwrap().to_string_lossy().into_owned();
    let mut sheet = Sheet { name, headings: Vec::new(), examples: Vec::new() };
    let mut in_examples = false;
    let mut heading: Option<String> = None;
    let mut fence: Option<Vec<String>> = None;
    for line in text.lines() {
        if let Some(open) = fence.as_mut() {
            if line.starts_with("```") {
                let h = heading.clone().unwrap();
                sheet.examples.push(Example { width: declared_width(&h), heading: h, lines: fence.take().unwrap() });
            } else {
                open.push(line.to_string());
            }
            continue;
        }
        if line.starts_with("## ") {
            in_examples = line.trim() == "## Examples";
            heading = None;
            continue;
        }
        if !in_examples {
            continue;
        }
        if let Some(h) = line.strip_prefix("### ") {
            let h = h.trim().to_string();
            sheet.headings.push(h.clone());
            heading = Some(h);
            continue;
        }
        if line.starts_with("```") && heading.is_some() {
            fence = Some(Vec::new());
        }
    }
    sheet
}

/// `(N columns)` at the end of the heading, or `, N columns wide`.
fn declared_width(heading: &str) -> Option<u16> {
    let tail = heading
        .rsplit_once('(')
        .and_then(|(_, t)| t.strip_suffix(')'))
        .or_else(|| heading.rsplit_once(", ").map(|(_, t)| t))?;
    tail.split_whitespace().next()?.parse().ok()
}

fn width_of(ex: &Example) -> u16 {
    ex.width.unwrap_or_else(|| cast!(ex.lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) => u16))
}

fn find<'a>(all: &'a [Sheet], entry: &Entry) -> &'a Example {
    all.iter()
        .find(|s| s.name == entry.sheet)
        .unwrap_or_else(|| panic!("{}.md: no such sheet", entry.sheet))
        .examples
        .iter()
        .find(|e| e.heading == entry.heading)
        .unwrap_or_else(|| panic!("{}.md: no fenced example under `### {}`", entry.sheet, entry.heading))
}

/// Render `entry` into a fresh buffer whose top-left fixture cell is `origin`.
fn render(entry: &Entry, width: u16, rows: u16, origin: (u16, u16), pad: u16) -> Buffer {
    let (ox, oy) = origin;
    let full = Rect::new(0, 0, width + 2 * pad, rows + 2 * pad);
    let mut buf = Buffer::empty(full);
    if pad > 0 {
        for y in full.top()..full.bottom() {
            for x in full.left()..full.right() {
                buf[(x, y)].set_symbol("¤");
            }
        }
    }
    let area = Rect::new(ox, oy, width, rows);
    match entry.frame {
        Frame::Bare => (entry.draw)(&mut buf, area),
        Frame::Rows => {
            let inner = Panel::untitled().render(&mut buf, area);
            (entry.draw)(&mut buf, inner);
        }
    }
    buf
}

fn row_text(buf: &Buffer, x0: u16, y: u16, width: u16) -> String {
    (x0..x0 + width).map(|x| buf[(x, y)].symbol().to_string()).collect()
}

/// The fixture rows, right-padded to `width`, plus the panel rows a `Rows` frame adds.
fn expected(entry: &Entry, ex: &Example, width: u16) -> Vec<String> {
    let pad = |l: &String| {
        let n = l.chars().count();
        format!("{l}{}", " ".repeat(usize::from(width).saturating_sub(n)))
    };
    match entry.frame {
        Frame::Bare => ex.lines.iter().map(pad).collect(),
        Frame::Rows => {
            let edge = "═".repeat(usize::from(width).saturating_sub(2));
            let mut v = vec![format!("╔{edge}╗")];
            v.extend(ex.lines.iter().map(pad));
            v.push(format!("╚{edge}╝"));
            v
        }
    }
}

#[test]
fn fixtures_fit_their_declared_width() {
    let mut bad = Vec::new();
    for sheet in sheets() {
        for ex in &sheet.examples {
            let Some(w) = ex.width else { continue };
            for (i, l) in ex.lines.iter().enumerate() {
                let n = l.chars().count();
                if n > usize::from(w) {
                    bad.push(format!("{}.md `### {}` row {i} is {n} cells, heading says {w}", sheet.name, ex.heading));
                }
            }
        }
    }
    assert!(bad.is_empty(), "fixtures wider than their heading:\n{}", bad.join("\n"));
}

#[test]
fn every_example_is_registered_or_listed() {
    let all = sheets();
    let registered: BTreeSet<(&str, &str)> = registry::examples().iter().map(|e| (e.sheet, e.heading)).collect();
    let listed: BTreeSet<(&str, &str)> = UNREPRODUCIBLE.iter().map(|(s, h, _)| (*s, *h)).collect();
    let mut problems = Vec::new();
    for pending in PENDING_SHEETS {
        if !all.iter().any(|s| s.name == *pending) {
            problems.push(format!("PENDING_SHEETS names {pending}.md, which does not exist"));
        }
    }
    for sheet in &all {
        let pending = PENDING_SHEETS.contains(&sheet.name.as_str());
        for h in &sheet.headings {
            let key = (sheet.name.as_str(), h.as_str());
            if registered.contains(&key) && pending {
                problems.push(format!("{}.md is registered but still in PENDING_SHEETS", sheet.name));
            }
            if !pending && !registered.contains(&key) && !listed.contains(&key) {
                problems.push(format!("{}.md `### {h}` is not registered in tests/components/registry.rs", sheet.name));
            }
        }
    }
    for (s, h) in &registered {
        if !all.iter().any(|sh| sh.name == *s && sh.headings.iter().any(|x| x == h)) {
            problems.push(format!("registry names {s}.md `### {h}`, which the sheet does not have"));
        }
        if listed.contains(&(s, h)) {
            problems.push(format!("{s}.md `### {h}` is both registered and UNREPRODUCIBLE"));
        }
    }
    for (s, h, _) in UNREPRODUCIBLE {
        if !all.iter().any(|sh| sh.name == *s && sh.headings.iter().any(|x| x == h)) {
            problems.push(format!("UNREPRODUCIBLE names {s}.md `### {h}`, which the sheet does not have"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn examples_render_cell_for_cell() {
    let all = sheets();
    let mut failures = Vec::new();
    for entry in registry::examples() {
        let ex = find(&all, &entry);
        let width = width_of(ex);
        let want = expected(&entry, ex, width);
        let rows = cast!(ex.lines.len() => u16);
        let buf = render(&entry, width, rows + if entry.frame == Frame::Rows { 2 } else { 0 }, (0, 0), 0);
        for (y, want_row) in want.iter().enumerate() {
            let got = row_text(&buf, 0, cast!(y => u16), width);
            if got != *want_row {
                let x = want_row.chars().zip(got.chars()).position(|(a, b)| a != b).unwrap_or(0);
                failures.push(format!(
                    "{}.md `### {}` differs at ({x}, {y})\nexpected: {want_row}\nactual:   {got}\n          {}^",
                    entry.sheet,
                    entry.heading,
                    " ".repeat(x)
                ));
            }
        }
        for &(x, y, color) in entry.fg {
            let got = buf[(x, y)].fg;
            if got != color {
                failures.push(format!("{}.md `### {}` fg at ({x}, {y}) is {got:?}, expected {color:?}", entry.sheet, entry.heading));
            }
        }
        for &(x, y, color) in entry.bg {
            let got = buf[(x, y)].bg;
            if got != color {
                failures.push(format!("{}.md `### {}` bg at ({x}, {y}) is {got:?}, expected {color:?}", entry.sheet, entry.heading));
            }
        }
    }
    assert!(failures.is_empty(), "{} example(s) differ:\n\n{}", failures.len(), failures.join("\n\n"));
}

#[test]
fn examples_stay_inside_their_area() {
    let all = sheets();
    let mut leaks = Vec::new();
    for entry in registry::examples() {
        let ex = find(&all, &entry);
        let width = width_of(ex);
        let rows = cast!(ex.lines.len() => u16) + if entry.frame == Frame::Rows { 2 } else { 0 };
        let buf = render(&entry, width, rows, (1, 1), 1);
        let full = buf.area;
        for y in full.top()..full.bottom() {
            for x in full.left()..full.right() {
                let ring = x == 0 || y == 0 || x == full.right() - 1 || y == full.bottom() - 1;
                if ring && buf[(x, y)].symbol() != "¤" {
                    leaks.push(format!("{}.md `### {}` wrote outside its area at ({x}, {y})", entry.sheet, entry.heading));
                }
            }
        }
    }
    assert!(leaks.is_empty(), "{}", leaks.join("\n"));
}
