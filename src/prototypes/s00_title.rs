//! S00: title screen / main menu.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::Prototype;
use crate::fixtures;
use crate::widgets::map;
use crate::widgets::{panel, status, util};
use crate::{glyphs, theme};

pub struct Title;

pub fn all() -> Vec<Box<dyn Prototype>> {
    vec![Box::new(Title)]
}

/// 5-row block-letter font built from CP437 half/full blocks. Every glyph is
/// exactly 5 cells wide.
fn letter(c: char) -> [&'static str; 5] {
    match c {
        'S' => ["▄████", "█    ", "▀███▄", "    █", "████▀"],
        'I' => ["█████", "  █  ", "  █  ", "  █  ", "█████"],
        'M' => ["█▄ ▄█", "█▀█▀█", "█ ▀ █", "█   █", "█   █"],
        'F' => ["█████", "█    ", "████ ", "█    ", "█    "],
        'O' => ["▄███▄", "█   █", "█   █", "█   █", "▀███▀"],
        'R' => ["████▄", "█   █", "████▀", "█  █ ", "█   █"],
        'T' => ["█████", "  █  ", "  █  ", "  █  ", "  █  "],
        'E' => ["█████", "█    ", "████ ", "█    ", "█████"],
        _ => ["     ", "     ", "     ", "     ", "     "],
    }
}

const MENU: [&str; 4] = ["New World", "Load World", "Options", "Quit"];

impl Prototype for Title {
    fn id(&self) -> &'static str {
        "S00a"
    }
    fn name(&self) -> &'static str {
        "Title / Main Menu"
    }
    fn variant(&self) -> &'static str {
        "default"
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        let fx = fixtures::get();
        let status_row = area.y + area.height - 1;
        util::fill(f.buffer_mut(), area, Style::default().bg(theme::BG));
        let bg = |st: Style| st.bg(theme::BG);

        // ---- big title ------------------------------------------------------
        let word = "SIM FORTRESS";
        let title_w = (word.chars().count() * 7 - 2) as u16;
        let tx = area.x + (area.width - title_w) / 2;
        let ty = area.y + 3;
        {
            let buf = f.buffer_mut();
            for row in 0..5 {
                // Warm gradient from top to bottom of the letters.
                let color = theme::lerp(theme::TITLE, theme::ACCENT, row as f32 / 4.0);
                let mut x = tx;
                for ch in word.chars() {
                    let s = letter(ch)[row];
                    buf.set_stringn(x, ty + row as u16, s, 5, Style::default().fg(color).bg(theme::BG).add_modifier(Modifier::BOLD));
                    x += 7;
                }
            }
            // Soft shadow line under the letters.
            let shadow: String = std::iter::repeat_n(glyphs::SHADE_1, title_w as usize).collect();
            buf.set_stringn(tx, ty + 5, &shadow, title_w as usize, Style::default().fg(theme::dim(theme::ACCENT, 0.6)).bg(theme::BG));
        }

        // ---- tagline --------------------------------------------------------
        let tagline = format!(
            "predator {} prey {} evolution {} scarcity",
            glyphs::DOT, glyphs::DOT, glyphs::DOT
        );
        center(f, area, 10, Line::from(Span::styled(tagline, bg(theme::dim_text()).add_modifier(Modifier::ITALIC))));

        // ---- terrain strips: real world rows, sampled -----------------------
        let strip_w = 120u16;
        let sx = area.x + (area.width - strip_w) / 2;
        terrain_strip(f, fx, sx, area.y + 12, strip_w, &[18, 19]);
        terrain_strip(f, fx, sx, area.y + 36, strip_w, &[30, 31]);

        // ---- menu box -------------------------------------------------------
        let box_w = 34u16;
        let box_h = 8u16;
        let bx = area.x + (area.width - box_w) / 2;
        let by = area.y + 16;
        let inner = panel::draw(f, Rect::new(bx, by, box_w, box_h), "Main Menu", panel::Kind::Focus);
        for (i, entry) in MENU.iter().enumerate() {
            let row = 1 + i as u16;
            let selected = i == 0;
            let text = if selected {
                format!("   {} {:<24}", glyphs::PLAY, entry)
            } else {
                format!("     {:<24}", entry)
            };
            let style = if selected { theme::selected() } else { theme::text() };
            let line = Line::from(vec![
                Span::styled(format!("{:<w$}", text, w = inner.width as usize), style),
            ]);
            util::line(f, inner, row, line);
            if selected {
                // Key hint at the right edge of the selected row.
                f.buffer_mut().set_stringn(inner.right() - 8, inner.y + row, "[Enter]", 7, Style::default().fg(theme::KEY).bg(theme::SELECT_BG).add_modifier(Modifier::BOLD));
            }
        }

        // ---- last world summary ---------------------------------------------
        let prey: u32 = fx.species.iter().filter(|s| s.id.kind() == fixtures::Kind::Prey).map(|s| s.count).sum();
        let pred: u32 = fx.species.iter().filter(|s| s.id.kind() == fixtures::Kind::Predator).map(|s| s.count).sum();
        center(
            f,
            area,
            26,
            Line::from(vec![
                Span::styled("last world: ", bg(theme::dim_text())),
                Span::styled("The Valley of Sunfall", bg(theme::title())),
                Span::styled(format!("  {} Year {}", glyphs::DOT, fx.clock.year), bg(theme::text())),
                Span::styled(format!("  {} {} prey / {} predators", glyphs::DOT, prey, pred), bg(theme::text())),
                Span::styled(format!("  {} seed 0xC0FFEE", glyphs::DOT), bg(theme::dim_text())),
            ]),
        );
        center(
            f,
            area,
            27,
            Line::from(vec![
                Span::styled(format!("autosaved {}  {} {}", fx.clock.label(), fx.clock.season.glyph(), fx.clock.season.name()), bg(theme::dim_text())),
            ]),
        );

        // ---- species roll-call ----------------------------------------------
        let mut spans = vec![Span::styled("inhabitants  ", bg(theme::dim_text()))];
        for sp in &fx.species {
            spans.push(Span::styled(
                format!("{}", sp.id.glyph().to_ascii_uppercase()),
                Style::default().fg(sp.id.color()).bg(theme::BG).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(format!(" {} {:<4}  ", sp.id.plural(), sp.count), bg(theme::text())));
        }
        center(f, area, 30, Line::from(spans));

        // ---- footer ---------------------------------------------------------
        center(
            f,
            area,
            33,
            Line::from(vec![
                Span::styled("Sim Fortress ", bg(theme::text())),
                Span::styled("v0.1.0-proto", bg(theme::label())),
                Span::styled(format!("  {}  ratatui {} truecolor {} CP437", glyphs::DOT, glyphs::DOT, glyphs::DOT), bg(theme::dim_text())),
            ]),
        );
        center(
            f,
            area,
            40,
            Line::from(Span::styled(
                format!("{} every creature you see is born, hunts, breeds and dies by the numbers in its genome {}", glyphs::DIAMOND, glyphs::DIAMOND),
                bg(theme::dim_text()),
            )),
        );

        // ---- status bar -----------------------------------------------------
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("↑↓", "select"), ("Enter", "confirm"), ("q", "quit")],
            "no world loaded",
        );
    }
}

/// Draw a centered line at `row` (relative to `area`) on the plain background.
fn center(f: &mut Frame, area: Rect, row: u16, line: Line) {
    let w = line.width() as u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    util::line(f, Rect::new(x, area.y, w.min(area.width), area.height), row, line);
}

/// A band of real terrain glyphs sampled from the given world rows, drawn in
/// terrain colors with the backgrounds dimmed so the band reads as decoration.
fn terrain_strip(f: &mut Frame, fx: &fixtures::Fixtures, x: u16, y: u16, w: u16, rows: &[usize]) {
    let buf = f.buffer_mut();
    let world = &fx.world;
    let start = (world.width() - w as usize) / 2;
    for (i, &wy) in rows.iter().enumerate() {
        for sx in 0..w as usize {
            let cell = world.cell(start + sx, wy);
            let (g, fg, bg) = map::terrain_cell(cell, false);
            // Fade toward the edges so the strip melts into the background.
            let edge = (sx.min(w as usize - 1 - sx) as f32 / 12.0).min(1.0);
            let fg = theme::lerp(theme::BG, fg, edge);
            let bg = theme::lerp(theme::BG, theme::dim(bg, 0.35), edge);
            if let Some(c) = buf.cell_mut((x + sx as u16, y + i as u16)) {
                c.set_char(g);
                c.set_style(Style::default().fg(fg).bg(bg));
            }
        }
    }
}
