//! S13: local zoom view. Every world cell around the look cursor is drawn as
//! a 3x3 tile so terrain, resources and creatures can be read individually.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::s01_map::{LOOK_CURSOR, MAP_PANEL_W, MAP_ROWS, SIDEBAR_W};
use super::Prototype;
use crate::fixtures::{self, Creature, Fixtures};
use crate::widgets::map;
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

pub struct Zoom;

pub fn all() -> Vec<Box<dyn Prototype>> {
    vec![Box::new(Zoom)]
}

const TILE_W: u16 = 3;
const TILE_H: u16 = 3;

/// Zoom window in world cells (x0, y0, w, h), centered on the cursor and
/// clamped to the world.
fn window(fx: &Fixtures, inner: Rect) -> (usize, usize, usize, usize) {
    // Round up so partial tiles fill the right/bottom edge instead of leaving slack.
    let w = inner.width.div_ceil(TILE_W) as usize;
    let h = inner.height.div_ceil(TILE_H) as usize;
    let (cx, cy) = LOOK_CURSOR;
    let x0 = cx.saturating_sub(w / 2).min(fx.world.width().saturating_sub(w));
    let y0 = cy.saturating_sub(h / 2).min(fx.world.height().saturating_sub(h));
    (x0, y0, w, h)
}

impl Prototype for Zoom {
    fn id(&self) -> &'static str {
        "S13a"
    }
    fn name(&self) -> &'static str {
        "Local Zoom View"
    }
    fn variant(&self) -> &'static str {
        "3x3 tiles around cursor"
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        let fx = fixtures::get();
        let (cx, cy) = LOOK_CURSOR;

        // ---- zoomed map panel
        let map_area = Rect::new(area.x, area.y, MAP_PANEL_W, MAP_ROWS);
        let probe = Rect::new(map_area.x + 1, map_area.y + 1, map_area.width - 2, map_area.height - 2);
        let (x0, y0, ww, wh) = window(fx, probe);
        let title = format!("Local Zoom  ({}, {})  {}", cx, cy, fx.world.region_name(cx, cy));
        let hint = format!("x {}-{}  y {}-{}   3x zoom", x0, x0 + ww - 1, y0, y0 + wh - 1);
        let inner = panel::draw_with_hint(f, map_area, &title, &hint, panel::Kind::Outer);
        self.draw_tiles(f, inner, fx, (x0, y0, ww, wh));

        // ---- sidebar
        let side = Rect::new(area.x + MAP_PANEL_W, area.y, SIDEBAR_W, MAP_ROWS);
        self.sidebar(f, side, fx, (x0, y0, ww, wh));

        // ---- ticker row: describe the window
        let ticker_row = area.y + MAP_ROWS;
        let ticker = Rect::new(area.x, ticker_row, area.width, 1);
        util::fill(f.buffer_mut(), ticker, Style::default().bg(theme::BG));
        let last = fx.events.last().unwrap();
        util::line(f, ticker, 0, Line::from(vec![
            Span::styled(format!(" {} ", last.kind.glyph()), Style::default().fg(last.kind.color()).bg(theme::BG).add_modifier(Modifier::BOLD)),
            Span::styled(last.text.clone(), Style::default().fg(theme::TEXT).bg(theme::BG)),
            Span::styled("   (e: full log)", Style::default().fg(theme::DIM).bg(theme::BG)),
        ]));

        // ---- status bar
        let status_row = area.y + area.height - 1;
        let keys: &[(&str, &str)] = &[("z", "zoom out"), ("↑↓←→", "move"), ("Enter", "inspect"), ("f", "follow"), ("Tab", "next creature"), ("Esc", "back to map")];
        let right = format!("{}  {} day", fx.clock.label(), glyphs::SUN);
        status::render(f, Rect::new(area.x, status_row, area.width, 1), keys, &right);
    }
}

impl Zoom {
    fn draw_tiles(&self, f: &mut Frame, inner: Rect, fx: &Fixtures, win: (usize, usize, usize, usize)) {
        let (x0, y0, ww, wh) = win;
        let (cx, cy) = LOOK_CURSOR;
        let buf = f.buffer_mut();
        // Unused slack on the right/bottom stays panel background.
        util::fill(buf, inner, Style::default().bg(theme::PANEL_BG));

        // Terrain tiles.
        for ty in 0..wh {
            for tx in 0..ww {
                let (wx, wy) = (x0 + tx, y0 + ty);
                let cell = fx.world.cell(wx, wy);
                let (g, fg, bg) = map::terrain_cell(cell, false);
                let tile = Rect::new(inner.x + tx as u16 * TILE_W, inner.y + ty as u16 * TILE_H, TILE_W, TILE_H).intersection(inner);
                util::fill(buf, tile, Style::default().bg(bg));
                // Faint texture in the corners so large same-terrain areas still read.
                let faint = Style::default().fg(theme::lerp(bg, fg, 0.35)).bg(bg);
                let full = Rect::new(inner.x + tx as u16 * TILE_W, inner.y + ty as u16 * TILE_H, TILE_W, TILE_H);
                for (dx, dy) in [(0u16, 0u16), (2, 0), (0, 2), (2, 2), (1, 1)] {
                    let (px, py) = (full.x + dx, full.y + dy);
                    if px < inner.right() && py < inner.bottom() {
                        let st = if (dx, dy) == (1, 1) { Style::default().fg(fg).bg(bg) } else { faint };
                        buf.set_stringn(px, py, g.to_string(), 1, st);
                    }
                }
            }
        }

        let center = |wx: usize, wy: usize| -> Option<(u16, u16)> {
            if wx < x0 || wy < y0 || wx >= x0 + ww || wy >= y0 + wh {
                return None;
            }
            let (sx, sy) = (inner.x + (wx - x0) as u16 * TILE_W + 1, inner.y + (wy - y0) as u16 * TILE_H + 1);
            if sx >= inner.right() || sy >= inner.bottom() {
                return None;
            }
            Some((sx, sy))
        };
        let mut put = |wx: usize, wy: usize, g: char, fg: Color, bold: bool| {
            if let Some((sx, sy)) = center(wx, wy) {
                if let Some(c) = buf.cell_mut((sx, sy)) {
                    c.set_char(g);
                    let mut st = Style::default().fg(fg).bg(c.bg);
                    if bold {
                        st = st.add_modifier(Modifier::BOLD);
                    }
                    c.set_style(st);
                }
            }
        };

        // Resources, then creatures on top.
        for &(x, y) in &fx.world.seeds {
            put(x, y, glyphs::SEED, theme::SEED, false);
        }
        for &(x, y) in &fx.world.dens {
            put(x, y, glyphs::DEN, theme::DEN, true);
        }
        for &(x, y) in &fx.world.carcasses {
            put(x, y, glyphs::CARCASS, theme::CARCASS, false);
        }
        for c in &fx.creatures {
            if c.alive {
                put(c.x, c.y, c.glyph(), c.species.color(), c.adult);
            } else {
                put(c.x, c.y, glyphs::CARCASS, theme::CARCASS, false);
            }
        }

        // Cursor tile: inverted, with corner marks in the neighbouring tiles.
        if let Some((sx, sy)) = center(cx, cy) {
            for dy in 0..TILE_H {
                for dx in 0..TILE_W {
                    if let Some(c) = buf.cell_mut((sx - 1 + dx, sy - 1 + dy)) {
                        let st = Style::default().fg(theme::CURSOR_FG).bg(theme::CURSOR_BG).add_modifier(Modifier::BOLD);
                        c.set_style(st);
                        if (dx, dy) != (1, 1) {
                            c.set_char(' ');
                        }
                    }
                }
            }
        }
        for (dx, dy) in [(-1i32, -1i32), (1, -1), (-1, 1), (1, 1)] {
            let (wx, wy) = (cx as i32 + dx, cy as i32 + dy);
            if wx >= 0 && wy >= 0 {
                if let Some((sx, sy)) = center(wx as usize, wy as usize) {
                    // Corner of that tile nearest the cursor.
                    let (ox, oy) = (sx as i32 - dx, sy as i32 - dy);
                    if let Some(c) = buf.cell_mut((ox as u16, oy as u16)) {
                        c.set_char(glyphs::CORNER);
                        c.set_fg(theme::CURSOR_BG);
                    }
                }
            }
        }
    }

    fn sidebar(&self, f: &mut Frame, area: Rect, fx: &Fixtures, win: (usize, usize, usize, usize)) {
        let inner = panel::draw(f, area, "Look", panel::Kind::Outer);
        let (x0, y0, ww, wh) = win;
        let (cx, cy) = LOOK_CURSOR;
        let cell = fx.world.cell(cx, cy);
        let mut row = 0u16;

        // ---- cursor cell
        panel::section(f, inner, row, "Cursor cell");
        row += 1;
        let (g, fg, bg) = map::terrain_cell(cell, false);
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" ", theme::text()),
            Span::styled(format!(" {} ", g), Style::default().fg(fg).bg(bg)),
            Span::styled(format!(" {}", cell.terrain.name()), theme::title()),
            Span::styled(format!("   ({}, {})", cx, cy), theme::text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" region ", theme::dim_text()),
            Span::styled(fx.world.region_name(cx, cy), theme::text()),
        ]));
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " elevation", cell.elevation, theme::ROCK_FG, 14, 16);
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " moisture", cell.moisture, theme::SHALLOW_FG, 14, 16);
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " vegetation", cell.vegetation, theme::VEGETATION, 14, 16);
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " prey traffic", cell.prey_pressure, theme::GOOD, 14, 16);
        row += 1;
        bars::labeled(f.buffer_mut(), inner, row, " pred traffic", cell.pred_pressure, theme::BAD, 14, 16);
        row += 1;
        let here: Vec<&Creature> = fx.creatures.iter().filter(|c| c.x == cx && c.y == cy).collect();
        let den = fx.world.dens.iter().any(|&(x, y)| (x, y) == (cx, cy));
        let occupant = if let Some(c) = here.first() {
            format!("{} {} is here", c.name, c.tag())
        } else if den {
            "a den is here".to_string()
        } else {
            "nothing is standing here".to_string()
        };
        util::line(f, inner, row, Line::from(Span::styled(format!(" {}", occupant), theme::dim_text())));
        row += 2;

        // ---- creatures in the window
        let mut near: Vec<(&Creature, f32)> = fx
            .creatures
            .iter()
            .filter(|c| c.x >= x0 && c.x < x0 + ww && c.y >= y0 && c.y < y0 + wh)
            .map(|c| {
                let dx = c.x as f32 - cx as f32;
                let dy = c.y as f32 - cy as f32;
                (c, (dx * dx + dy * dy).sqrt())
            })
            .collect();
        near.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        panel::section(f, inner, row, &format!("In view ({})", near.len()));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled("   tag    name     goal             dist", theme::label())));
        row += 1;
        let max_rows = 14usize;
        for (c, d) in near.iter().take(max_rows) {
            let goal: String = if c.alive {
                c.goal.chars().take(16).collect()
            } else {
                "carcass".to_string()
            };
            let (glyph, color) = if c.alive { (c.glyph(), c.species.color()) } else { (glyphs::CARCASS, theme::CARCASS) };
            let dir = direction(c.x as i32 - cx as i32, c.y as i32 - cy as i32);
            util::line(f, inner, row, Line::from(vec![
                Span::styled(format!(" {} ", glyph), Style::default().fg(color).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<6} {:<8} {:<16} ", c.tag(), c.name, goal), if c.alive { theme::text() } else { theme::dim_text() }),
                Span::styled(format!("{:>2.0} {:<2}", d, dir), theme::dim_text()),
            ]));
            row += 1;
        }
        if near.len() > max_rows {
            util::line(f, inner, row, Line::from(Span::styled(format!("   … and {} more", near.len() - max_rows), theme::dim_text())));
            row += 1;
        }
        row += 1;

        // ---- mini legend
        panel::section(f, inner, row, "Legend");
        row += 1;
        let legend = map::legend();
        for pair in legend.chunks(2) {
            let mut spans = vec![Span::styled(" ", theme::text())];
            for (g, color, label) in pair {
                spans.push(Span::styled(g.to_string(), Style::default().fg(*color).bg(theme::PANEL_BG)));
                spans.push(Span::styled(format!(" {:<17}", label), theme::dim_text()));
            }
            util::line(f, inner, row, Line::from(spans));
            row += 1;
        }
        util::line(f, inner, row, Line::from(vec![
            Span::styled(" ", theme::text()),
            Span::styled("   ", Style::default().bg(theme::CURSOR_BG)),
            Span::styled(" cursor tile   ", theme::dim_text()),
            Span::styled(glyphs::CORNER.to_string(), Style::default().fg(theme::CURSOR_BG).bg(theme::PANEL_BG)),
            Span::styled(" corner marks", theme::dim_text()),
        ]));
        row += 1;
        util::line(f, inner, row, Line::from(Span::styled(" 1 tile = 1 world cell (3x3 screen cells)", theme::dim_text())));
    }
}

/// Compass direction label for an offset from the cursor.
fn direction(dx: i32, dy: i32) -> &'static str {
    match (dx.signum(), dy.signum()) {
        (0, 0) => "",
        (0, -1) => "N",
        (0, 1) => "S",
        (1, 0) => "E",
        (-1, 0) => "W",
        (1, -1) => "NE",
        (-1, -1) => "NW",
        (1, 1) => "SE",
        _ => "SW",
    }
}
