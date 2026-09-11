//! World map renderer: terrain, resources, creatures, overlays, cursor, trails.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::sim::world::{Cell, Terrain, World};
use crate::{glyphs, theme};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Overlay {
    None,
    Vegetation,
    Pressure,
    Moisture,
    /// Sense-range rings for a creature index.
    Sense(usize),
}

#[derive(Clone, Debug)]
pub struct MapOptions {
    pub overlay: Overlay,
    pub night: bool,
    pub winter: bool,
    pub cursor: Option<(usize, usize)>,
    /// Creature index to highlight and draw a trail for.
    pub follow: Option<usize>,
    /// Top-left world cell shown at the top-left of the area.
    pub origin: (usize, usize),
    /// Draw creatures (false for pure terrain/overlay views).
    pub creatures: bool,
    /// When an overlay is active, fade creatures so the overlay reads.
    pub fade_creatures: bool,
}

impl Default for MapOptions {
    fn default() -> Self {
        MapOptions {
            overlay: Overlay::None,
            night: false,
            winter: false,
            cursor: None,
            follow: None,
            origin: (0, 0),
            creatures: true,
            fade_creatures: false,
        }
    }
}

/// A lightweight view of one creature, decoupled from any concrete creature
/// type so the map renderer only depends on the data it draws.
#[derive(Clone, Copy, Debug)]
pub struct MapCreature<'a> {
    pub x: usize,
    pub y: usize,
    pub alive: bool,
    pub adult: bool,
    pub glyph: char,
    pub color: Color,
    pub sense_cells: u16,
    pub trail: &'a [(usize, usize)],
    pub target: Option<(usize, usize)>,
}

/// The data the map renderer draws: a world plus a creature view.
pub struct MapData<'a> {
    pub world: &'a World,
    pub creatures: &'a [MapCreature<'a>],
    /// Creature index currently selected (used by later chunks' inspector flows).
    pub selected: Option<usize>,
}

/// Glyph and style for a bare terrain cell.
pub fn terrain_cell(cell: &Cell, winter: bool) -> (char, Color, Color) {
    use Terrain::*;
    let (g, fg, bg) = match cell.terrain {
        DeepWater => (glyphs::DEEP_WATER, theme::DEEP_WATER_FG, theme::DEEP_WATER_BG),
        ShallowWater => (glyphs::SHALLOW_WATER, theme::SHALLOW_FG, theme::SHALLOW_BG),
        Sand => (glyphs::SAND, theme::SAND_FG, theme::SAND_BG),
        Dirt => (glyphs::DIRT, theme::DIRT_FG, theme::DIRT_BG),
        GrassSparse => (glyphs::GRASS_SPARSE, theme::GRASS_SPARSE_FG, theme::GRASS_BG),
        Grass => (glyphs::GRASS, theme::GRASS_FG, theme::GRASS_BG),
        GrassDense => (glyphs::GRASS_DENSE, theme::GRASS_DENSE_FG, theme::GRASS_BG),
        Forest => (glyphs::FOREST, theme::FOREST_FG, theme::FOREST_BG),
        Rock => (glyphs::ROCK, theme::ROCK_FG, theme::ROCK_BG),
    };
    if winter {
        match cell.terrain {
            ShallowWater => (glyphs::SHALLOW_WATER, Color::Rgb(150, 190, 230), Color::Rgb(36, 66, 110)),
            Sand | Dirt | GrassSparse => (glyphs::SNOW, theme::SNOW_FG, theme::SNOW_BG),
            Grass | GrassDense => (glyphs::GRASS_SPARSE, Color::Rgb(170, 190, 170), theme::SNOW_BG),
            Forest => (glyphs::FOREST, Color::Rgb(70, 120, 80), Color::Rgb(48, 58, 62)),
            Rock => (glyphs::ROCK, theme::SNOW_FG, Color::Rgb(84, 84, 96)),
            _ => (g, fg, bg),
        }
    } else {
        (g, fg, bg)
    }
}

/// Glyph and colors for a cell under an overlay (before creatures are drawn).
pub fn overlay_cell(cell: &Cell, overlay: Overlay) -> Option<(char, Color, Color)> {
    let (t, color) = match overlay {
        Overlay::Vegetation => (cell.vegetation, theme::veg(cell.vegetation)),
        Overlay::Pressure => {
            let t = (cell.pred_pressure * 0.7 + cell.prey_pressure * 0.5).min(1.0);
            (t, theme::heat(t))
        }
        Overlay::Moisture => {
            let t = if cell.terrain.is_water() { 1.0 } else { cell.moisture };
            (t, theme::water(t))
        }
        _ => return None,
    };
    if cell.terrain == Terrain::DeepWater && overlay != Overlay::Moisture {
        return Some((glyphs::DEEP_WATER, theme::dim(theme::DEEP_WATER_FG, 0.4), theme::dim(theme::DEEP_WATER_BG, 0.4)));
    }
    if cell.terrain == Terrain::Rock && overlay != Overlay::Moisture {
        return Some((glyphs::ROCK, theme::dim(theme::ROCK_FG, 0.5), theme::dim(theme::ROCK_BG, 0.5)));
    }
    let g = glyphs::shade(t);
    let g = if g == ' ' { glyphs::DIRT } else { g };
    Some((g, color, theme::dim(color, 0.75)))
}

pub fn render(buf: &mut Buffer, area: Rect, data: &MapData, opts: &MapOptions) {
    let world = data.world;
    let (ox, oy) = opts.origin;
    let tint = |c: Color| if opts.night { theme::night(c) } else { c };

    // Terrain / overlay layer.
    for sy in 0..area.height {
        for sx in 0..area.width {
            let (wx, wy) = (ox + sx as usize, oy + sy as usize);
            let Some(c) = buf.cell_mut((area.x + sx, area.y + sy)) else { continue };
            if wx >= world.width() || wy >= world.height() {
                c.set_char(' ');
                c.set_style(Style::default().bg(theme::BG));
                continue;
            }
            let cell = world.cell(wx, wy);
            let (g, fg, bg) = overlay_cell(cell, opts.overlay).unwrap_or_else(|| terrain_cell(cell, opts.winter));
            c.set_char(g);
            c.set_style(Style::default().fg(tint(fg)).bg(tint(bg)));
        }
    }

    let put = |buf: &mut Buffer, wx: usize, wy: usize, g: char, fg: Color, bold: bool| {
        if wx < ox || wy < oy {
            return;
        }
        let (sx, sy) = ((wx - ox) as u16, (wy - oy) as u16);
        if sx >= area.width || sy >= area.height {
            return;
        }
        if let Some(c) = buf.cell_mut((area.x + sx, area.y + sy)) {
            c.set_char(g);
            let mut st = Style::default().fg(fg).bg(c.bg);
            if bold {
                st = st.add_modifier(Modifier::BOLD);
            }
            c.set_style(st);
        }
    };

    // Resources.
    let res_fade = if opts.overlay != Overlay::None && opts.fade_creatures { 0.5 } else { 0.0 };
    for &(x, y) in &world.seeds {
        put(buf, x, y, glyphs::SEED, tint(theme::dim(theme::SEED, res_fade)), false);
    }
    for &(x, y) in &world.dens {
        put(buf, x, y, glyphs::DEN, tint(theme::dim(theme::DEN, res_fade)), true);
    }
    for &(x, y) in &world.carcasses {
        put(buf, x, y, glyphs::CARCASS, tint(theme::dim(theme::CARCASS, res_fade)), false);
    }

    // Sense rings (drawn under creatures).
    if let Overlay::Sense(idx) = opts.overlay {
        let c = &data.creatures[idx];
        let r = c.sense_cells as i32;
        for wy in (c.y as i32 - r)..=(c.y as i32 + r) {
            for wx in (c.x as i32 - 2 * r)..=(c.x as i32 + 2 * r) {
                if !world.in_bounds(wx, wy) {
                    continue;
                }
                let dx = (wx - c.x as i32) as f32 / 2.0;
                let dy = (wy - c.y as i32) as f32;
                let d = (dx * dx + dy * dy).sqrt();
                if (d - r as f32).abs() < 0.55 {
                    put(buf, wx as usize, wy as usize, glyphs::RING, theme::ACCENT, false);
                } else if d < r as f32 {
                    if let Some(cell) = buf.cell_mut((area.x + (wx as usize - ox) as u16, area.y + (wy as usize - oy) as u16)) {
                        let bg = theme::lerp(cell.bg, theme::ACCENT, 0.18);
                        cell.set_bg(bg);
                    }
                }
            }
        }
    }

    // Trail for the followed creature.
    if let Some(idx) = opts.follow {
        let c = &data.creatures[idx];
        let n = c.trail.len().max(1) as f32;
        for (i, &(x, y)) in c.trail.iter().enumerate() {
            let t = (i as f32 + 1.0) / n;
            put(buf, x, y, glyphs::TRAIL, theme::lerp(theme::dim(theme::TRAIL, 0.7), theme::TRAIL, t), false);
        }
        if let Some((tx, ty)) = c.target {
            put(buf, tx, ty, glyphs::DIAMOND, theme::ACCENT, true);
        }
    }

    // Creatures.
    if opts.creatures {
        let fade = if opts.overlay != Overlay::None && opts.fade_creatures { 0.55 } else { 0.0 };
        for (i, c) in data.creatures.iter().enumerate() {
            if !c.alive {
                put(buf, c.x, c.y, glyphs::CARCASS, tint(theme::CARCASS), false);
                continue;
            }
            let mut color = tint(theme::dim(c.color, fade));
            if let Overlay::Sense(idx) = opts.overlay {
                if idx == i {
                    color = theme::TEXT_BRIGHT;
                }
            }
            put(buf, c.x, c.y, c.glyph, color, c.adult);
        }
        if let Some(idx) = opts.follow {
            let c = &data.creatures[idx];
            if let Some(cell) = cell_at(buf, area, opts, c.x, c.y) {
                cell.set_style(Style::default().fg(theme::CURSOR_FG).bg(theme::ACCENT).add_modifier(Modifier::BOLD));
            }
        }
    }

    // Cursor with corner marks.
    if let Some((cx, cy)) = opts.cursor {
        if let Some(cell) = cell_at(buf, area, opts, cx, cy) {
            cell.set_style(Style::default().fg(theme::CURSOR_FG).bg(theme::CURSOR_BG).add_modifier(Modifier::BOLD));
        }
        for (dx, dy) in [(-1i32, -1i32), (1, -1), (-1, 1), (1, 1)] {
            let (wx, wy) = (cx as i32 + dx, cy as i32 + dy);
            if world.in_bounds(wx, wy) {
                if let Some(cell) = cell_at(buf, area, opts, wx as usize, wy as usize) {
                    cell.set_char(glyphs::CORNER);
                    cell.set_fg(theme::CURSOR_BG);
                }
            }
        }
    }
}

fn cell_at<'a>(buf: &'a mut Buffer, area: Rect, opts: &MapOptions, wx: usize, wy: usize) -> Option<&'a mut ratatui::buffer::Cell> {
    let (ox, oy) = opts.origin;
    if wx < ox || wy < oy {
        return None;
    }
    let (sx, sy) = ((wx - ox) as u16, (wy - oy) as u16);
    if sx >= area.width || sy >= area.height {
        return None;
    }
    buf.cell_mut((area.x + sx, area.y + sy))
}

/// Legend rows: (glyph, color, label) for terrain and creatures.
pub fn legend() -> Vec<(char, Color, &'static str)> {
    vec![
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
    ]
}
