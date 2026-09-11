//! World map renderer: terrain, resources, creatures, overlays, cursor, trails.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::sim::creatures::CreatureId;
use crate::sim::world::{Cell, Terrain, World};
use crate::{glyphs, theme};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Overlay {
    None,
    Vegetation,
    Pressure,
    Moisture,
    /// Sense-range rings for a creature id.
    Sense(CreatureId),
    /// Named regions: tinted rectangles with centred labels.
    Region,
}

#[derive(Clone, Debug)]
pub struct MapOptions {
    pub overlay: Overlay,
    pub night: bool,
    pub winter: bool,
    pub cursor: Option<(usize, usize)>,
    /// Creature id to highlight and draw a trail for.
    pub follow: Option<CreatureId>,
    /// Top-left world cell shown at the top-left of the area.
    pub origin: (usize, usize),
    /// Draw creatures (false for pure terrain/overlay views).
    pub creatures: bool,
    /// When an overlay is active, fade creatures so the overlay reads.
    pub fade_creatures: bool,
    /// Region index drawn brighter under `Overlay::Region`.
    pub selected_region: Option<usize>,
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
            selected_region: None,
        }
    }
}

/// A lightweight view of one creature, decoupled from any concrete creature
/// type so the map renderer only depends on the data it draws.
#[derive(Clone, Copy, Debug)]
pub struct MapCreature<'a> {
    pub id: CreatureId,
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

/// The data the map renderer draws. Implemented by `Fixtures` and `Sim` (FR Scope).
pub trait MapSource {
    fn world(&self) -> &World;
    /// All living creatures, in a stable order.
    fn living_creatures(&self) -> Vec<MapCreature<'_>>;
    /// One creature by id, for follow/sense highlights.
    fn creature(&self, id: CreatureId) -> Option<MapCreature<'_>>;
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

pub fn render(buf: &mut Buffer, area: Rect, source: &dyn MapSource, opts: &MapOptions) {
    let world = source.world();
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

    // Region tint (under everything else).
    if opts.overlay == Overlay::Region {
        region_tint(buf, area, world, opts);
    }

    // Resources.
    let res_fade = if opts.overlay != Overlay::None && opts.fade_creatures { 0.5 } else { 0.0 };
    for &(x, y) in &world.seeds {
        put(buf, x, y, glyphs::SEED, tint(theme::dim(theme::SEED, res_fade)), false);
    }
    for &(x, y) in &world.dens {
        put(buf, x, y, glyphs::DEN, tint(theme::dim(theme::DEN, res_fade)), true);
    }
    // Carcasses render only from `world.carcasses` (FR Scope).
    for &(x, y) in &world.carcasses {
        put(buf, x, y, glyphs::CARCASS, tint(theme::dim(theme::CARCASS, res_fade)), false);
    }

    // Sense rings (drawn under creatures).
    if let Overlay::Sense(id) = opts.overlay {
        if let Some(c) = source.creature(id) {
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
    }

    // Trail for the followed creature.
    if let Some(id) = opts.follow {
        if let Some(c) = source.creature(id) {
            let n = c.trail.len().max(1) as f32;
            for (i, &(x, y)) in c.trail.iter().enumerate() {
                let t = (i as f32 + 1.0) / n;
                put(buf, x, y, glyphs::TRAIL, theme::lerp(theme::dim(theme::TRAIL, 0.7), theme::TRAIL, t), false);
            }
            if let Some((tx, ty)) = c.target {
                put(buf, tx, ty, glyphs::DIAMOND, theme::ACCENT, true);
            }
        }
    }

    // Region labels (under creatures so a passing creature stays visible).
    if opts.overlay == Overlay::Region {
        region_labels(buf, area, world, opts);
    }

    // Creatures.
    if opts.creatures {
        let fade = if opts.overlay != Overlay::None && opts.fade_creatures { 0.55 } else { 0.0 };
        let mut followed_pos: Option<(usize, usize)> = None;
        for c in source.living_creatures() {
            if !c.alive {
                put(buf, c.x, c.y, glyphs::CARCASS, tint(theme::CARCASS), false);
                continue;
            }
            let mut color = tint(theme::dim(c.color, fade));
            if let Overlay::Sense(sid) = opts.overlay {
                if sid == c.id {
                    color = theme::TEXT_BRIGHT;
                }
            }
            put(buf, c.x, c.y, c.glyph, color, c.adult);
            if opts.follow == Some(c.id) {
                followed_pos = Some((c.x, c.y));
            }
        }
        if let Some((x, y)) = followed_pos {
            if let Some(cell) = cell_at(buf, area, opts, x, y) {
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

/// Blend factor of the region tint over the terrain background.
pub const REGION_TINT: f32 = 0.30;
/// Blend factor for the selected region.
pub const REGION_TINT_SELECTED: f32 = 0.50;

/// Tint the background of every visible cell of every region toward that
/// region's colour. Iterates the region rectangles clipped to the viewport
/// rather than looking up a region per cell.
fn region_tint(buf: &mut Buffer, area: Rect, world: &World, opts: &MapOptions) {
    let (ox, oy) = opts.origin;
    let (vx1, vy1) = (ox + area.width as usize, oy + area.height as usize);
    for (i, r) in world.regions.iter().enumerate() {
        let (x0, y0, x1, y1) = (r.1.max(ox), r.2.max(oy), r.3.min(vx1).min(world.width()), r.4.min(vy1).min(world.height()));
        if x0 >= x1 || y0 >= y1 {
            continue;
        }
        let amount = if opts.selected_region == Some(i) { REGION_TINT_SELECTED } else { REGION_TINT };
        let color = theme::region(i);
        for wy in y0..y1 {
            for wx in x0..x1 {
                if let Some(cell) = cell_at(buf, area, opts, wx, wy) {
                    let bg = theme::lerp(cell.bg, color, amount);
                    cell.set_bg(bg);
                }
            }
        }
    }
}

/// Where a region's label starts in world coordinates: centred on the
/// rectangle, clamped so the whole label stays inside it.
pub fn region_label_origin(r: &crate::sim::RegionRect, world_w: usize) -> (usize, usize) {
    let w = r.0.chars().count();
    let cx = (r.1 + r.3) / 2;
    let cy = (r.2 + r.4) / 2;
    let x = cx.saturating_sub(w / 2).max(r.1);
    let x = x.min(r.3.saturating_sub(w)).min(world_w.saturating_sub(w));
    (x, cy)
}

/// Draw each region's name, bold and bright, clipped (never shifted) at the
/// viewport edge.
fn region_labels(buf: &mut Buffer, area: Rect, world: &World, opts: &MapOptions) {
    for (i, r) in world.regions.iter().enumerate() {
        let (lx, ly) = region_label_origin(r, world.width());
        let selected = opts.selected_region == Some(i);
        for (k, ch) in r.0.chars().enumerate() {
            if let Some(cell) = cell_at(buf, area, opts, lx + k, ly) {
                cell.set_char(ch);
                let st = if selected {
                    theme::selected()
                } else {
                    Style::default().fg(theme::TEXT_BRIGHT).bg(cell.bg).add_modifier(Modifier::BOLD)
                };
                cell.set_style(st);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::world::Cell;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    struct TestSource<'a> {
        world: &'a World,
    }

    impl<'a> MapSource for TestSource<'a> {
        fn world(&self) -> &World {
            self.world
        }
        fn living_creatures(&self) -> Vec<MapCreature<'_>> {
            Vec::new()
        }
        fn creature(&self, _id: CreatureId) -> Option<MapCreature<'_>> {
            None
        }
    }

    /// A `w`×`h` all-dirt world split into two regions down the middle.
    fn two_region_world(w: usize, h: usize) -> World {
        let cell = Cell { terrain: Terrain::Dirt, elevation: 0.5, moisture: 0.5, vegetation: 0.5, prey_pressure: 0.0, pred_pressure: 0.0, dried_from: None };
        World {
            cells: vec![cell; w * h],
            width: w,
            height: h,
            dens: vec![],
            carcasses: vec![],
            seeds: vec![],
            regions: vec![("Ab".to_string(), 0, 0, w / 2, h), ("Cd".to_string(), w / 2, 0, w, h)],
            water_cells_at_generation: 0,
            shore: vec![],
        }
    }

    fn draw(world: &World, opts: &MapOptions, w: u16, h: u16) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let source = TestSource { world };
        terminal.draw(|f| render(f.buffer_mut(), Rect::new(0, 0, w, h), &source, opts)).unwrap();
        terminal.backend().buffer().clone()
    }

    #[test]
    fn region_overlay_tints_bg() {
        let world = two_region_world(8, 4);
        let opts = MapOptions { overlay: Overlay::Region, selected_region: Some(1), ..MapOptions::default() };
        let buf = draw(&world, &opts, 8, 4);
        // Row 0 carries no label (labels sit on row 2), so its cells show the pure tint.
        assert_eq!(buf[(0, 0)].bg, theme::lerp(theme::DIRT_BG, theme::region(0), REGION_TINT));
        assert_eq!(buf[(7, 0)].bg, theme::lerp(theme::DIRT_BG, theme::region(1), REGION_TINT_SELECTED));
        // Terrain glyph is kept.
        assert_eq!(buf[(0, 0)].symbol(), glyphs::DIRT.to_string());
        // Label "Ab" is centred in the left region: x = (0+4)/2 - 1 = 1, y = (0+4)/2 = 2.
        assert_eq!(buf[(1, 2)].symbol(), "A");
        assert_eq!(buf[(2, 2)].symbol(), "b");
        assert!(buf[(1, 2)].modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn region_labels_clip_at_viewport_edge() {
        let world = two_region_world(8, 4);
        // Origin x = 2 hides column 1 ("A"); the "b" must stay at world x = 2 → screen x = 0.
        let opts = MapOptions { overlay: Overlay::Region, origin: (2, 0), ..MapOptions::default() };
        let buf = draw(&world, &opts, 6, 4);
        assert_eq!(buf[(0, 2)].symbol(), "b");
        assert_eq!(buf[(1, 2)].symbol(), glyphs::DIRT.to_string());
        // A label wider than its region is clamped inside the world, never past it.
        let mut wide = two_region_world(8, 4);
        wide.regions[1].0 = "Toolongname".to_string();
        assert_eq!(region_label_origin(&wide.regions[1], wide.width()), (0, 2));
        let _ = draw(&wide, &opts, 6, 4);
    }
}
