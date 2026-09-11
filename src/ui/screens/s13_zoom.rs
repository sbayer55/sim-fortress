//! S13: live local zoom view — every world cell around the look cursor as a 3×3
//! tile, with the look-sidebar readout and an in-view creature list.

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
use crate::widgets::{bars, map, panel, status, util};
use crate::{glyphs, theme};

const TILE_W: u16 = 3;
const TILE_H: u16 = 3;

pub struct Zoom;

impl Default for Zoom {
    fn default() -> Self {
        Self::new()
    }
}

impl Zoom {
    pub fn new() -> Self {
        Zoom
    }

    fn window(world: &World, cx: usize, cy: usize, inner: Rect) -> (usize, usize, usize, usize) {
        let w = inner.width.div_ceil(TILE_W) as usize;
        let h = inner.height.div_ceil(TILE_H) as usize;
        let x0 = cx.saturating_sub(w / 2).min(world.width().saturating_sub(w));
        let y0 = cy.saturating_sub(h / 2).min(world.height().saturating_sub(h));
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
                        return Action::Push(Box::new(Inspector::new(id)));
                    }
                }
                Action::None
            }
            KeyCode::Char('f') => {
                if let Some((x, y)) = cur {
                    if let Some(id) = crate::ui::screens::s01_map::WorldMap::creature_at_cursor(sim, x, y) {
                        app.follow = Some(id);
                        app.look_cursor = None;
                        return Action::Pop;
                    }
                }
                Action::None
            }
            KeyCode::Tab => {
                // Jump the cursor to the next in-view creature, by dist then id.
                if let Some((cx, cy)) = cur {
                    let ids = sim.creatures.living_ids();
                    if !ids.is_empty() {
                        let mut order: Vec<(f32, CreatureId)> = ids
                            .iter()
                            .filter_map(|&id| {
                                let c = sim.creatures.get(id)?;
                                Some((crate::sim::dist(cx, cy, c.x, c.y), id))
                            })
                            .collect();
                        order.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)));
                        let cur_id = sim.creatures.living().find(|c| c.x == cx && c.y == cy).map(|c| c.id);
                        let pos = cur_id.and_then(|id| order.iter().position(|&(_, x)| x == id)).unwrap_or(0);
                        let next = order[(pos + 1) % order.len()].1;
                        if let Some(c) = sim.creatures.get(next) {
                            app.look_cursor = Some((c.x, c.y));
                        }
                    }
                }
                Action::None
            }
            _ => Action::Unhandled,
        }
    }

    fn render(&self, app: &AppState, f: &mut Frame, area: Rect) {
        let Some(sim) = &app.sim else { return };
        let world = &sim.world;
        let Some((cx, cy)) = app.look_cursor else { return };

        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let map_area = Rect::new(area.x, area.y, area.width * 2 / 3, body_h);
        let side_area = Rect::new(area.x + map_area.width, area.y, area.width - map_area.width, body_h);

        let probe = Rect::new(map_area.x + 1, map_area.y + 1, map_area.width - 2, map_area.height - 2);
        let (x0, y0, ww, wh) = Self::window(world, cx, cy, probe);
        let title = format!("Local Zoom  ({}, {})  {}", cx, cy, world.region_name(cx, cy));
        let hint = format!("x {}-{}  y {}-{}   3x zoom", x0, x0 + ww - 1, y0, y0 + wh - 1);
        let inner = panel::draw_with_hint(f, map_area, &title, &hint, panel::Kind::Outer);
        draw_tiles(f, inner, sim, (x0, y0, ww, wh), (cx, cy));
        sidebar(f, side_area, sim, (x0, y0, ww, wh), (cx, cy));

        let right = format!("{}  {} day", sim.time.clock_label(), glyphs::SUN);
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("z", "zoom out"), ("↑↓←→", "move"), ("Enter", "inspect"), ("f", "follow"), ("Tab", "next"), ("Esc", "back")],
            &right,
        );
    }
}

fn draw_tiles(f: &mut Frame, inner: Rect, sim: &Sim, win: (usize, usize, usize, usize), cursor: (usize, usize)) {
    let (x0, y0, ww, wh) = win;
    let (cx, cy) = cursor;
    let world = &sim.world;
    let buf = f.buffer_mut();
    util::fill(buf, inner, Style::default().bg(theme::PANEL_BG));

    for ty in 0..wh {
        for tx in 0..ww {
            let (wx, wy) = (x0 + tx, y0 + ty);
            let cell = world.cell(wx, wy);
            let (g, fg, bg) = map::terrain_cell(cell, false);
            let tile = Rect::new(inner.x + tx as u16 * TILE_W, inner.y + ty as u16 * TILE_H, TILE_W, TILE_H).intersection(inner);
            util::fill(buf, tile, Style::default().bg(bg));
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
        let glyph = if c.adult { c.species.glyph().to_ascii_uppercase() } else { c.species.glyph() };
        put(c.x, c.y, glyph, c.species.color(), c.adult);
    }

    // Cursor tile.
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
                let (ox, oy) = (sx as i32 - dx, sy as i32 - dy);
                if let Some(c) = buf.cell_mut((ox as u16, oy as u16)) {
                    c.set_char(glyphs::CORNER);
                    c.set_fg(theme::CURSOR_BG);
                }
            }
        }
    }
}

fn sidebar(f: &mut Frame, area: Rect, sim: &Sim, win: (usize, usize, usize, usize), cursor: (usize, usize)) {
    let inner = panel::draw(f, area, "Look", panel::Kind::Outer);
    let (x0, y0, ww, wh) = win;
    let (cx, cy) = cursor;
    let cell = sim.world.cell(cx, cy);
    let mut row = 0u16;

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
        Span::styled(sim.world.region_name(cx, cy), theme::text()),
    ]));
    row += 1;
    bars::labeled(f.buffer_mut(), inner, row, " vegetation", cell.vegetation, theme::VEGETATION, 14, 16);
    row += 1;
    let here = sim.creatures.living().find(|c| c.x == cx && c.y == cy);
    let den = sim.world.dens.iter().any(|&(x, y)| (x, y) == (cx, cy));
    let occupant = if let Some(c) = here {
        format!("{} {} is here", c.name_str(), c.tag())
    } else if den {
        "a den is here".to_string()
    } else {
        "nothing is standing here".to_string()
    };
    util::line(f, inner, row, Line::from(Span::styled(format!(" {}", occupant), theme::dim_text())));
    row += 2;

    let mut near: Vec<(f32, crate::sim::creatures::CreatureId)> = sim
        .creatures
        .living()
        .filter(|c| c.x >= x0 && c.x < x0 + ww && c.y >= y0 && c.y < y0 + wh)
        .map(|c| (crate::sim::dist(cx, cy, c.x, c.y), c.id))
        .collect();
    near.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)));
    panel::section(f, inner, row, &format!("In view ({})", near.len()));
    row += 1;
    util::line(f, inner, row, Line::from(Span::styled("   tag    name     dist", theme::label())));
    row += 1;
    for (d, id) in near.iter().take(14) {
        let c = sim.creatures.get(*id).unwrap();
        let glyph = if c.adult { c.species.glyph().to_ascii_uppercase() } else { c.species.glyph() };
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} ", glyph), Style::default().fg(c.species.color()).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:<6} {:<8} {:>4.0}", c.tag(), c.name_str(), d), theme::text()),
        ]));
        row += 1;
    }
    if near.len() > 14 {
        util::line(f, inner, row, Line::from(Span::styled(format!("   … and {} more", near.len() - 14), theme::dim_text())));
    }
}
