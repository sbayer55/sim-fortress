//! S13: live local zoom view — every world cell around the look cursor as a 3×3
//! tile, with the look-sidebar readout and an in-view creature list.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::creatures::CreatureId;
use crate::sim::{Sim, World};
use crate::ui::app::AppState;
use crate::ui::screens::s03_inspector::Inspector;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{bars, map, panel, util, Component, StatusBar};
use crate::{glyphs, theme};

const TILE_W: u16 = 3;
const TILE_H: u16 = 3;

#[derive(Debug)]
pub struct Zoom;

impl Default for Zoom {
    fn default() -> Self {
        Self::new()
    }
}

impl Zoom {
    pub const fn new() -> Self {
        Self
    }

    fn window(world: &World, cx: usize, cy: usize, inner: Rect) -> (usize, usize, usize, usize) {
        let w = crate::cast!(inner.width.div_ceil(TILE_W) => usize);
        let h = crate::cast!(inner.height.div_ceil(TILE_H) => usize);
        let x0 = cx.saturating_sub(w.div_euclid(2)).min(world.width().saturating_sub(w));
        let y0 = cy.saturating_sub(h.div_euclid(2)).min(world.height().saturating_sub(h));
        (x0, y0, w, h)
    }
}

impl Screen for Zoom {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        let Some(sim) = &app.sim else { return Action::None };
        let (w, h) = (sim.world.width(), sim.world.height());
        let cur = app.look_cursor;
        match key.code {
            KeyCode::Esc | KeyCode::Char('z') => Action::Pop,
            KeyCode::Left => {
                if let Some((x, y)) = cur {
                    app.look_cursor = Some((x.saturating_sub(1), y));
                }
                Action::None
            }
            KeyCode::Right => {
                if let Some((x, y)) = cur {
                    app.look_cursor = Some(((x + 1).min(w - 1), y));
                }
                Action::None
            }
            KeyCode::Up => {
                if let Some((x, y)) = cur {
                    app.look_cursor = Some((x, y.saturating_sub(1)));
                }
                Action::None
            }
            KeyCode::Down => {
                if let Some((x, y)) = cur {
                    app.look_cursor = Some((x, (y + 1).min(h - 1)));
                }
                Action::None
            }
            KeyCode::Enter => {
                if let Some((x, y)) = cur {
                    if let Some(id) = crate::ui::screens::s01_map::WorldMap::creature_at_cursor(sim, x, y) {
                        app.leave_look();
                        return Action::Push(Box::new(Inspector::new(id)));
                    }
                }
                Action::None
            }
            KeyCode::Char('f') => {
                if let Some((x, y)) = cur {
                    if let Some(id) = crate::ui::screens::s01_map::WorldMap::creature_at_cursor(sim, x, y) {
                        app.follow = Some(id);
                        app.leave_look();
                        return Action::Pop;
                    }
                }
                Action::None
            }
            KeyCode::Tab => {
                // Jump the cursor to the next in-view creature, by dist then id.
                let Some((cx, cy)) = cur else {
                    return Action::None;
                };
                let ids = sim.creatures.living_ids();
                if ids.is_empty() {
                    return Action::None;
                }
                let mut order: Vec<(f32, CreatureId)> = ids
                    .iter()
                    .filter_map(|&id| sim.creatures.get(id).map(|c| (crate::sim::dist(cx, cy, c.x, c.y), id)))
                    .collect();
                order.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
                let cur_id = sim.creatures.living().find(|c| c.x == cx && c.y == cy).map(|c| c.id);
                let pos = cur_id.and_then(|id| order.iter().position(|&(_, x)| x == id)).unwrap_or(0);
                let next = order[(pos + 1) % order.len()].1;
                if let Some(c) = sim.creatures.get(next) {
                    app.look_cursor = Some((c.x, c.y));
                }
                Action::None
            }
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else { return };
        let world = &sim.world;
        let Some((cx, cy)) = app.look_cursor else { return };

        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let map_area = Rect::new(area.x, area.y, (area.width * 2).div_euclid(3), body_h);
        let side_area = Rect::new(area.x + map_area.width, area.y, area.width - map_area.width, body_h);

        let probe = Rect::new(map_area.x + 1, map_area.y + 1, map_area.width - 2, map_area.height - 2);
        let (x0, y0, ww, wh) = Self::window(world, cx, cy, probe);
        let title = format!("Local Zoom  ({}, {})  {}", cx, cy, world.region_name(cx, cy));
        let hint = format!("x {}-{}  y {}-{}   3x zoom", x0, x0 + ww - 1, y0, y0 + wh - 1);
        let inner = panel::draw_with_hint(f, map_area, &title, &hint, panel::Kind::Outer);
        draw_tiles(f, inner, sim, (x0, y0, ww, wh), (cx, cy));
        sidebar(f, side_area, sim, (x0, y0, ww, wh), (cx, cy));

        let right = format!("{}  {} day", sim.time.clock_label(), glyphs::SUN);
        StatusBar::new(&[("z", "zoom out"), ("↑↓←→", "move"), ("Enter", "inspect"), ("f", "follow"), ("Tab", "next"), ("Esc", "back")]).right(&right).render(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1));
    }
}

fn draw_tiles(f: &mut Frame<'_>, inner: Rect, sim: &Sim, win: (usize, usize, usize, usize), cursor: (usize, usize)) {
    let (x0, y0, ww, wh) = win;
    let (cx, cy) = cursor;
    let world = &sim.world;
    let buf = f.buffer_mut();
    util::fill(buf, inner, Style::default().bg(theme::PANEL_BG));

    for ty in 0..wh {
        for tx in 0..ww {
            let (wx, wy) = (x0 + tx, y0 + ty);
            let (g, fg, bg) = map::world_cell(world, wx, wy, false);
            let tile = Rect::new(inner.x + crate::cast!(tx => u16) * TILE_W, inner.y + crate::cast!(ty => u16) * TILE_H, TILE_W, TILE_H).intersection(inner);
            util::fill(buf, tile, Style::default().bg(bg));
            let faint = Style::default().fg(theme::lerp(bg, fg, 0.35)).bg(bg);
            let full = Rect::new(inner.x + crate::cast!(tx => u16) * TILE_W, inner.y + crate::cast!(ty => u16) * TILE_H, TILE_W, TILE_H);
            draw_tile(buf, inner, full, g, fg, bg, faint);
        }
    }

    let center = |wx: usize, wy: usize| -> Option<(u16, u16)> {
        if wx < x0 || wy < y0 || wx >= x0 + ww || wy >= y0 + wh {
            return None;
        }
        let (sx, sy) = (inner.x + crate::cast!((wx - x0) => u16) * TILE_W + 1, inner.y + crate::cast!((wy - y0) => u16) * TILE_H + 1);
        if sx >= inner.right() || sy >= inner.bottom() {
            return None;
        }
        Some((sx, sy))
    };
    let put = |wx: usize, wy: usize, g: char, fg: Color, bold: bool| {
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

    draw_resources(world, sim, put);

    // Cursor tile.
    if let Some((sx, sy)) = center(cx, cy) {
        paint_cursor_tile(buf, sx, sy);
    }
    draw_corners(buf, &center, cx, cy);
}


/// The five lit glyph cells of one 3x3 tile.
fn draw_tile(buf: &mut Buffer, inner: Rect, full: Rect, g: char, fg: Color, bg: Color, faint: Style) {
    for (dx, dy) in [(0u16, 0u16), (2, 0), (0, 2), (2, 2), (1, 1)] {
        let (px, py) = (full.x + dx, full.y + dy);
        let st = if (dx, dy) == (1, 1) { Style::default().fg(fg).bg(bg) } else { faint };
        if px < inner.right() && py < inner.bottom() {
            buf.set_stringn(px, py, g.to_string(), 1, st);
        }
    }
}

/// Seeds, dens, carcasses and living creatures, via the tile `put` closure.
fn draw_resources(world: &World, sim: &Sim, mut put: impl FnMut(usize, usize, char, Color, bool)) {
    for &(x, y) in &world.seeds {
        put(x, y, glyphs::SEED, theme::SEED, false);
    }
    for &(x, y) in &world.dens {
        put(x, y, glyphs::DEN, theme::DEN, true);
    }
    for &(x, y) in &world.carcasses {
        put(x, y, glyphs::CARCASS, theme::CARCASS, false);
    }
    for c in sim.creatures.living() {
        let glyph = if c.adult { sim.roster().adult_glyph(c.species) } else { sim.roster().glyph(c.species) };
        put(c.x, c.y, glyph, sim.roster().color(c.species), c.adult);
    }
}

/// The four corner ticks framing the cursor tile.
fn draw_corners(buf: &mut Buffer, center: &impl Fn(usize, usize) -> Option<(u16, u16)>, cx: usize, cy: usize) {
    for (dx, dy) in [(-1i32, -1i32), (1, -1), (-1, 1), (1, 1)] {
        let (wx, wy) = (crate::cast!(cx => i32) + dx, crate::cast!(cy => i32) + dy);
        if wx >= 0 && wy >= 0 {
            if let Some((sx, sy)) = center(crate::cast!(wx => usize), crate::cast!(wy => usize)) {
                let (ox, oy) = (i32::from(sx) - dx, i32::from(sy) - dy);
                if let Some(c) = buf.cell_mut((crate::cast!(ox => u16), crate::cast!(oy => u16))) {
                    c.set_char(glyphs::CORNER);
                    c.set_fg(theme::CURSOR_BG);
                }
            }
        }
    }
}

/// Paint the 3×3 cursor tile: every cell bold, keeping the centre glyph.
fn paint_cursor_tile(buf: &mut Buffer, sx: u16, sy: u16) {
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

fn sidebar(f: &mut Frame<'_>, area: Rect, sim: &Sim, win: (usize, usize, usize, usize), cursor: (usize, usize)) {
    let inner = panel::draw(f, area, "Look", panel::Kind::Outer);
    let (x0, y0, ww, wh) = win;
    let (cx, cy) = cursor;
    let cell = sim.world.cell(cx, cy);
    let mut row = 0u16;

    panel::section(f, inner, row, "Cursor cell");
    row += 1;
    let (g, fg, bg) = map::world_cell(&sim.world, cx, cy, false);
    util::line(f, inner, row, Line::from(vec![
        Span::styled(" ", theme::text()),
        Span::styled(format!(" {g} "), Style::default().fg(fg).bg(bg)),
        Span::styled(format!(" {}", sim.world.terrain_name(cx, cy)), theme::title()),
        Span::styled(format!("   ({cx}, {cy})"), theme::text()),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        Span::styled(" region ", theme::dim_text()),
        Span::styled(sim.world.region_name(cx, cy), theme::text()),
    ]));
    row += 1;
    bars::labeled(f.buffer_mut(), inner, row, " vegetation", cell.vegetation, theme::VEGETATION, 14, 16);
    row += 1;
    let here = sim.creatures.living().find(|c| c.x == cx && c.y == cy);
    let den = sim.world.dens.iter().any(|&(x, y)| (x, y) == (cx, cy));
    let occupant = if let Some(c) = here {
        format!("{} {} is here", c.name_str(sim.roster()), c.tag(sim.roster()))
    } else if den {
        "a den is here".to_string()
    } else {
        "nothing is standing here".to_string()
    };
    util::line(f, inner, row, Line::from(Span::styled(format!(" {occupant}"), theme::dim_text())));
    row += 2;

    let mut near: Vec<(f32, CreatureId)> = sim
        .creatures
        .living()
        .filter(|c| c.x >= x0 && c.x < x0 + ww && c.y >= y0 && c.y < y0 + wh)
        .map(|c| (crate::sim::dist(cx, cy, c.x, c.y), c.id))
        .collect();
    near.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
    panel::section(f, inner, row, &format!("In view ({})", near.len()));
    row += 1;
    util::line(f, inner, row, Line::from(Span::styled("   tag    name     dist", theme::label())));
    row += 1;
    for (d, id) in near.iter().take(14) {
        let Some(c) = sim.creatures.get(*id) else { continue };
        let glyph = if c.adult { sim.roster().adult_glyph(c.species) } else { sim.roster().glyph(c.species) };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {glyph} "), Style::default().fg(sim.roster().color(c.species)).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:<6} {:<8} {:>4.0}", c.tag(sim.roster()), c.name_str(sim.roster()), d), theme::text()),
        ]));
        row += 1;
    }
    if near.len() > 14 {
        util::line(f, inner, row, Line::from(Span::styled(format!("   … and {} more", near.len() - 14), theme::dim_text())));
    }
}
