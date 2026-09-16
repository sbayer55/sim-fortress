//! World map renderer: terrain, resources, creatures, overlays, cursor, trails.

use std::collections::HashMap;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::sim::creatures::CreatureId;
use crate::sim::disease::{PathogenId, Stage};
use crate::sim::species::SpeciesId;
use crate::sim::world::{Cell, Terrain, World};
use crate::{glyphs, theme};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Overlay {
    None,
    Vegetation,
    Pressure,
    Moisture,
    /// Sense-range rings for a creature id.
    Sense(CreatureId),
    /// Named regions: tinted rectangles with centred labels.
    Region,
    /// Population density of one species (S02f).
    Species(SpeciesId),
    /// Health: every living creature coloured by its weakest vital (S02g).
    Health,
    /// Disease (S02h): every living creature coloured by its infection state
    /// for one pathogen slot, or for every pathogen when `None`.
    Disease(Option<PathogenId>),
    /// Parasites (S02i): a heatmap of `cell.parasite_load`, creatures on top
    /// coloured by their own load.
    Parasites,
}

/// How far the terrain fades under `Overlay::Health` so the creature colours
/// carry the picture.
pub const HEALTH_TERRAIN_DIM: f32 = 0.6;

/// Cells at or above this parasite load get a `WARN` background tint under
/// the disease overlay (S02h).
pub const PARASITE_TINT_THRESHOLD: f32 = 0.25;
/// Blend factor of that tint.
pub const PARASITE_TINT: f32 = 0.18;
/// Creature parasite-load bands (S02i): below `PARASITE_LIGHT` the species
/// colour dimmed, up to `PARASITE_HEAVY` amber, from there red and bold.
pub const PARASITE_LIGHT: f32 = 0.2;
pub const PARASITE_HEAVY: f32 = 0.5;
/// How far a healthy creature's species colour fades under S02h/S02i.
pub const HEALTHY_FADE: f32 = 0.55;

/// Kernel radius (ellipse metric, cells) of one creature's contribution to the
/// species-density field: 7 rows × 13 columns on screen.
pub const DENSITY_RADIUS: u16 = 3;
/// Weighted creature-equivalents within one kernel that read as 100 %.
pub const DENSITY_CAP: f32 = 6.0;

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
    /// Ramp colour for `Overlay::Species` (the species' own colour; a UI concern,
    /// so the caller supplies it).
    pub species_color: Color,
    /// Per-creature colour override (and forced bold) for the S02h disease and
    /// S02i parasite overlays. The caller computes it from the sim (see
    /// `disease_tint` / `parasite_tint`); a living creature with no entry draws
    /// as usual.
    pub creature_tint: Option<HashMap<CreatureId, (Color, bool)>>,
}

impl Default for MapOptions {
    fn default() -> Self {
        Self {
            overlay: Overlay::None,
            night: false,
            winter: false,
            cursor: None,
            follow: None,
            origin: (0, 0),
            creatures: true,
            fade_creatures: false,
            selected_region: None,
            species_color: theme::TEXT,
            creature_tint: None,
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
    pub species: SpeciesId,
    pub glyph: char,
    pub color: Color,
    pub sense_cells: u16,
    /// Weakest vital in 0..=1 (health, fullness, hydration or energy, whichever
    /// is lowest); drives the colour under `Overlay::Health`.
    pub condition: f32,
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
///
/// Land is tinted a little toward its biome's hue (cold ground blue-grey,
/// hot dry ground yellow) so the biomes read as areas; the winter palette,
/// water and rock are left alone.
pub fn terrain_cell(cell: &Cell, winter: bool) -> (char, Color, Color) {
    let (g, fg, bg) = terrain_base(cell.terrain, winter);
    if winter || cell.terrain.is_water() || cell.terrain == Terrain::Rock {
        return (g, fg, bg);
    }
    let hue = theme::biome(crate::cast!(cell.biome => u8));
    (g, theme::lerp(fg, hue, theme::BIOME_TINT_FG), theme::lerp(bg, hue, theme::BIOME_TINT_BG))
}

/// Glyph and colours for a terrain before any biome tint.
pub const fn terrain_base(terrain: Terrain, winter: bool) -> (char, Color, Color) {
    use Terrain::{DeepWater, ShallowWater, Sand, Dirt, GrassSparse, Grass, GrassDense, Forest, Rock, Marsh};
    let (g, fg, bg) = match terrain {
        DeepWater => (glyphs::DEEP_WATER, theme::DEEP_WATER_FG, theme::DEEP_WATER_BG),
        ShallowWater => (glyphs::SHALLOW_WATER, theme::SHALLOW_FG, theme::SHALLOW_BG),
        Sand => (glyphs::SAND, theme::SAND_FG, theme::SAND_BG),
        Dirt => (glyphs::DIRT, theme::DIRT_FG, theme::DIRT_BG),
        GrassSparse => (glyphs::GRASS_SPARSE, theme::GRASS_SPARSE_FG, theme::GRASS_BG),
        Grass => (glyphs::GRASS, theme::GRASS_FG, theme::GRASS_BG),
        GrassDense => (glyphs::GRASS_DENSE, theme::GRASS_DENSE_FG, theme::GRASS_BG),
        Forest => (glyphs::FOREST, theme::FOREST_FG, theme::FOREST_BG),
        Rock => (glyphs::ROCK, theme::ROCK_FG, theme::ROCK_BG),
        Marsh => (glyphs::MARSH, theme::MARSH_FG, theme::MARSH_BG),
    };
    if winter {
        match terrain {
            ShallowWater => (glyphs::SHALLOW_WATER, Color::Rgb(150, 190, 230), Color::Rgb(36, 66, 110)),
            Sand | Dirt | GrassSparse => (glyphs::SNOW, theme::SNOW_FG, theme::SNOW_BG),
            Grass | GrassDense => (glyphs::GRASS_SPARSE, Color::Rgb(170, 190, 170), theme::SNOW_BG),
            Forest => (glyphs::FOREST, Color::Rgb(70, 120, 80), Color::Rgb(48, 58, 62)),
            Rock => (glyphs::ROCK, theme::SNOW_FG, Color::Rgb(84, 84, 96)),
            Marsh => (glyphs::MARSH, Color::Rgb(140, 170, 160), Color::Rgb(30, 50, 56)),
            DeepWater => (g, fg, bg),
        }
    } else {
        (g, fg, bg)
    }
}

/// Glyph and colors for a serialised terrain code (0..=9), summer/day palette
/// with no biome tint (used by the S00 title-screen decorative strips, C6 FR1).
pub const fn terrain_code_cell(code: u8) -> (char, Color, Color) {
    terrain_base(Terrain::from_code(code), false)
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
        Overlay::Parasites => return Some(parasite_cell(cell)),
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

/// Species-density field (S02f): one value in 0..=1 per world cell. Every
///
/// living creature of `species` adds a kernel of radius `DENSITY_RADIUS` in
/// the 2:1 ellipse metric with linear falloff (`1 − d / (r + 1)`), and the sum
///
/// is clamped against the fixed `DENSITY_CAP` so the picture is comparable
/// across species and over time: a lone animal reads faint, a herd reads bright.
pub fn density_field(world: &World, creatures: &[MapCreature<'_>], species: SpeciesId) -> Vec<f32> {
    let (w, h) = (world.width(), world.height());
    let mut field = vec![0.0f32; w * h];
    let r = i32::from(DENSITY_RADIUS);
    for c in creatures.iter().filter(|c| c.alive && c.species == species) {
        for wy in (crate::cast!(c.y => i32) - r)..=(crate::cast!(c.y => i32) + r) {
            for wx in (crate::cast!(c.x => i32) - 2 * r)..=(crate::cast!(c.x => i32) + 2 * r) {
                if !world.in_bounds(wx, wy) {
                    continue;
                }
                let dx = crate::cast!((wx - crate::cast!(c.x => i32)) => f32) / 2.0;
                let dy = crate::cast!((wy - crate::cast!(c.y => i32)) => f32);
                let d = (dx * dx + dy * dy).sqrt();
                if d <= crate::cast!(r => f32) {
                    field[crate::cast!(wy => usize) * w + crate::cast!(wx => usize)] += 1.0 - d / (crate::cast!(r => f32) + 1.0);
                }
            }
        }
    }
    for v in &mut field {
        *v = (*v / DENSITY_CAP).min(1.0);
    }
    field
}

/// Glyph and colors for a cell under the species-density overlay, given the
/// field value `t` at that cell. Deep water and rock keep their dimmed glyphs
/// as on the vegetation overlay.
pub fn density_cell(cell: &Cell, t: f32, species: SpeciesId, color: Color) -> (char, Color, Color) {
    let _ = species;
    if cell.terrain == Terrain::DeepWater {
        return (glyphs::DEEP_WATER, theme::dim(theme::DEEP_WATER_FG, 0.4), theme::dim(theme::DEEP_WATER_BG, 0.4));
    }
    if cell.terrain == Terrain::Rock {
        return (glyphs::ROCK, theme::dim(theme::ROCK_FG, 0.5), theme::dim(theme::ROCK_BG, 0.5));
    }
    let c = theme::species_ramp(color, t);
    let g = glyphs::shade(t);
    let g = if g == ' ' { glyphs::DIRT } else { g };
    (g, c, theme::dim(c, 0.75))
}

/// Colour of a creature under the health overlay: the same good / warning /
/// bad bands the vital bars use (above 60 %, 30–60 %, below 30 %).
pub fn condition_color(condition: f32) -> Color {
    crate::widgets::bars::vital_color(condition, false)
}

/// Glyph and colours for a cell under the parasite heatmap (S02i): the
///
/// `theme::parasite` ramp at `cell.parasite_load`, deep water and rock keeping
/// their dimmed glyphs, and any water cell carrying a load drawn as `~` in
///
/// `WARN` (shared drinking spots are the hot spots).
pub fn parasite_cell(cell: &Cell) -> (char, Color, Color) {
    let t = cell.parasite_load.clamp(0.0, 1.0);
    if cell.terrain.is_water() {
        if t > 0.0 {
            return (glyphs::SHALLOW_WATER, theme::WARN, theme::dim(theme::parasite(t), 0.75));
        }
        return match cell.terrain {
            Terrain::DeepWater => (glyphs::DEEP_WATER, theme::dim(theme::DEEP_WATER_FG, 0.4), theme::dim(theme::DEEP_WATER_BG, 0.4)),
            _ => (glyphs::SHALLOW_WATER, theme::dim(theme::SHALLOW_FG, 0.4), theme::dim(theme::SHALLOW_BG, 0.4)),
        };
    }
    if cell.terrain == Terrain::Rock {
        return (glyphs::ROCK, theme::dim(theme::ROCK_FG, 0.5), theme::dim(theme::ROCK_BG, 0.5));
    }
    let color = theme::parasite(t);
    let g = glyphs::shade(t);
    let g = if g == ' ' { glyphs::DIRT } else { g };
    (g, color, theme::dim(color, 0.75))
}

/// Colour (and forced bold) of a living creature under the disease overlay
///
/// (S02h). `infection` is the creature's current infection, `immune` whether
/// it is immune to the shown pathogen (or to any, when all are shown), `load`
///
/// its parasite load. An infection with a pathogen other than the shown one
/// counts as healthy for this picture.
pub fn disease_tint(species: Color, shown: Option<PathogenId>, infection: Option<(PathogenId, Stage)>, immune: bool, load: f32) -> (Color, bool) {
    match infection {
        Some((p, stage)) if shown.is_none_or(|s| s == p) => match stage {
            Stage::Infectious => (theme::SICK, true),
            Stage::Incubating => (theme::dim(theme::SICK, 0.4), false),
        },
        _ if immune => (theme::IMMUNE, false),
        _ if load >= PARASITE_HEAVY => (theme::WARN, false),
        _ => (theme::dim(species, HEALTHY_FADE), false),
    }
}

/// Colour (and forced bold) of a living creature under the parasite overlay
/// (S02i), by its own load band.
pub fn parasite_tint(species: Color, load: f32) -> (Color, bool) {
    if load >= PARASITE_HEAVY {
        (theme::BAD, true)
    } else if load >= PARASITE_LIGHT {
        (theme::WARN, false)
    } else {
        (theme::dim(species, HEALTHY_FADE), false)
    }
}

pub fn render(buf: &mut Buffer, area: Rect, source: &dyn MapSource, opts: &MapOptions) {
    let world = source.world();
    let living = source.living_creatures();

    // Species-density field (S02f), computed once per frame from the living set.
    let density = match opts.overlay {
        Overlay::Species(sp) => Some((sp, opts.species_color, density_field(world, &living, sp))),
        _ => None,
    };

    draw_terrain(buf, area, world, opts, density.as_ref());

    // Region tint (under everything else).
    if opts.overlay == Overlay::Region {
        region_tint(buf, area, world, opts);
    }
    draw_resources(buf, area, world, opts);

    // Sense rings (drawn under creatures).
    draw_sense_ring(buf, area, source, world, opts);

    // Trail for the followed creature.
    draw_trail(buf, area, source, opts);

    // Region labels (under creatures so a passing creature stays visible).
    if opts.overlay == Overlay::Region {
        region_labels(buf, area, world, opts);
    }
    if opts.creatures {
        draw_creatures(buf, area, &living, opts);
    }
    draw_cursor(buf, area, world, opts);
}

/// Night dimming for one colour.
fn tint_color(c: Color, night: bool) -> Color {
    if night {
        theme::night(c)
    } else {
        c
    }
}

/// Write one glyph at world position `(wx, wy)` when it is inside the viewport.
#[allow(clippy::too_many_arguments)]
fn put_cell(buf: &mut Buffer, area: Rect, ox: usize, oy: usize, wx: usize, wy: usize, g: char, fg: Color, bold: bool) {
    if wx < ox || wy < oy {
        return;
    }
    let (sx, sy) = (crate::cast!((wx - ox) => u16), crate::cast!((wy - oy) => u16));
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
}

/// The terrain / overlay layer under everything else.
fn draw_terrain(buf: &mut Buffer, area: Rect, world: &World, opts: &MapOptions, density: Option<&(SpeciesId, Color, Vec<f32>)>) {
    let (ox, oy) = opts.origin;
    for sy in 0..area.height {
        for sx in 0..area.width {
            let (wx, wy) = (ox + crate::cast!(sx => usize), oy + crate::cast!(sy => usize));
            let Some(c) = buf.cell_mut((area.x + sx, area.y + sy)) else { continue };
            if wx >= world.width() || wy >= world.height() {
                c.set_char(' ');
                c.set_style(Style::default().bg(theme::BG));
                continue;
            }
            let cell = world.cell(wx, wy);
            let (g, fg, bg) = match &density {
                Some((sp, color, field)) => density_cell(cell, field[wy * world.width() + wx], *sp, *color),
                None if opts.overlay == Overlay::Health => {
                    let (g, fg, bg) = terrain_cell(cell, opts.winter);
                    (g, theme::dim(fg, HEALTH_TERRAIN_DIM), theme::dim(bg, HEALTH_TERRAIN_DIM))
                }
                // S02h: the health path's dimmed terrain, plus a warning tint
                // on the background of fouled cells.
                None if matches!(opts.overlay, Overlay::Disease(_)) => {
                    let (g, fg, bg) = terrain_cell(cell, opts.winter);
                    let mut bg = theme::dim(bg, HEALTH_TERRAIN_DIM);
                    if cell.parasite_load >= PARASITE_TINT_THRESHOLD {
                        bg = theme::lerp(bg, theme::WARN, PARASITE_TINT);
                    }
                    (g, theme::dim(fg, HEALTH_TERRAIN_DIM), bg)
                }
                None => overlay_cell(cell, opts.overlay).unwrap_or_else(|| terrain_cell(cell, opts.winter)),
            };
            c.set_char(g);
            c.set_style(Style::default().fg(tint_color(fg, opts.night)).bg(tint_color(bg, opts.night)));
        }
    }
}

/// Seeds, dens and carcasses.
fn draw_resources(buf: &mut Buffer, area: Rect, world: &World, opts: &MapOptions) {
    let (ox, oy) = opts.origin;
    let res_fade = if opts.overlay != Overlay::None && opts.fade_creatures { 0.5 } else { 0.0 };
    for &(x, y) in &world.seeds {
        put_cell(buf, area, ox, oy, x, y, glyphs::SEED, tint_color(theme::dim(theme::SEED, res_fade), opts.night), false);
    }
    for &(x, y) in &world.dens {
        put_cell(buf, area, ox, oy, x, y, glyphs::DEN, tint_color(theme::dim(theme::DEN, res_fade), opts.night), true);
    }
    // Carcasses render only from `world.carcasses` (FR Scope).
    for &(x, y) in &world.carcasses {
        put_cell(buf, area, ox, oy, x, y, glyphs::CARCASS, tint_color(theme::dim(theme::CARCASS, res_fade), opts.night), false);
    }
}

/// The detection ellipse of the creature under the sense overlay.
fn draw_sense_ring(buf: &mut Buffer, area: Rect, source: &dyn MapSource, world: &World, opts: &MapOptions) {
    let (ox, oy) = opts.origin;
    let sense = match opts.overlay {
        Overlay::Sense(id) => source.creature(id),
        _ => None,
    };
    if let Some(c) = sense {
        let r = i32::from(c.sense_cells);
        for wy in (crate::cast!(c.y => i32) - r)..=(crate::cast!(c.y => i32) + r) {
            for wx in (crate::cast!(c.x => i32) - 2 * r)..=(crate::cast!(c.x => i32) + 2 * r) {
                if !world.in_bounds(wx, wy) {
                    continue;
                }
                let dx = crate::cast!((wx - crate::cast!(c.x => i32)) => f32) / 2.0;
                let dy = crate::cast!((wy - crate::cast!(c.y => i32)) => f32);
                let d = (dx * dx + dy * dy).sqrt();
                if (d - crate::cast!(r => f32)).abs() < 0.55 {
                    put_cell(buf, area, ox, oy, crate::cast!(wx => usize), crate::cast!(wy => usize), glyphs::RING, theme::ACCENT, false);
                } else if d < crate::cast!(r => f32) {
                    tint_sense_cell(buf, crate::cast!(wx => usize), crate::cast!(wy => usize), ox, oy, area);
                }
            }
        }
    }
}

/// The followed creature's trail and target marker.
fn draw_trail(buf: &mut Buffer, area: Rect, source: &dyn MapSource, opts: &MapOptions) {
    let (ox, oy) = opts.origin;
    if let Some(id) = opts.follow {
        if let Some(c) = source.creature(id) {
            let n = crate::cast!(c.trail.len().max(1) => f32);
            for (i, &(x, y)) in c.trail.iter().enumerate() {
                let t = (crate::cast!(i => f32) + 1.0) / n;
                put_cell(buf, area, ox, oy, x, y, glyphs::TRAIL, theme::lerp(theme::dim(theme::TRAIL, 0.7), theme::TRAIL, t), false);
            }
            if let Some((tx, ty)) = c.target {
                put_cell(buf, area, ox, oy, tx, ty, glyphs::DIAMOND, theme::ACCENT, true);
            }
        }
    }
}

/// Every living creature, plus the follow highlight.
fn draw_creatures(buf: &mut Buffer, area: Rect, living: &[MapCreature<'_>], opts: &MapOptions) {
    let (ox, oy) = opts.origin;
    if opts.creatures {
        let fade = if opts.overlay != Overlay::None && opts.fade_creatures { 0.55 } else { 0.0 };
        let mut followed_pos: Option<(usize, usize)> = None;
        for c in living {
            if !c.alive {
                put_cell(buf, area, ox, oy, c.x, c.y, glyphs::CARCASS, tint_color(theme::CARCASS, opts.night), false);
                continue;
            }
            // Under the species overlay the shown species draws at full strength
            // over its own density; every other species fades.
            let own = matches!(opts.overlay, Overlay::Species(sp) if sp == c.species);
            // Under the health overlay the species colour gives way to the
            // creature's condition: green, amber or red at full strength.
            let mut color = if opts.overlay == Overlay::Health {
                tint_color(condition_color(c.condition), opts.night)
            } else {
                tint_color(theme::dim(c.color, if own { 0.0 } else { fade }), opts.night)
            };
            // S02h / S02i: the caller's per-creature tint wins outright.
            let mut bold = c.adult;
            if let Some((tc, force_bold)) = opts.creature_tint.as_ref().and_then(|m| m.get(&c.id)) {
                color = tint_color(*tc, opts.night);
                bold |= *force_bold;
            }
            if let Overlay::Sense(sid) = opts.overlay {
                if sid == c.id {
                    color = theme::TEXT_BRIGHT;
                }
            }
            put_cell(buf, area, ox, oy, c.x, c.y, c.glyph, color, bold);
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
}

/// The look cursor and its corner marks.
fn draw_cursor(buf: &mut Buffer, area: Rect, world: &World, opts: &MapOptions) {
    if let Some((cx, cy)) = opts.cursor {
        if let Some(cell) = cell_at(buf, area, opts, cx, cy) {
            cell.set_style(Style::default().fg(theme::CURSOR_FG).bg(theme::CURSOR_BG).add_modifier(Modifier::BOLD));
        }
        for (dx, dy) in [(-1i32, -1i32), (1, -1), (-1, 1), (1, 1)] {
            let (wx, wy) = (crate::cast!(cx => i32) + dx, crate::cast!(cy => i32) + dy);
            if world.in_bounds(wx, wy) {
                if let Some(cell) = cell_at(buf, area, opts, crate::cast!(wx => usize), crate::cast!(wy => usize)) {
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

/// Dim one interior cell of a sense ellipse, when it is on screen.
fn tint_sense_cell(buf: &mut Buffer, wx: usize, wy: usize, ox: usize, oy: usize, area: Rect) {
    if wx < ox || wy < oy {
        return;
    }
    let (sx, sy) = (crate::cast!((wx - ox) => u16), crate::cast!((wy - oy) => u16));
    if sx >= area.width || sy >= area.height {
        return;
    }
    if let Some(cell) = buf.cell_mut((area.x + sx, area.y + sy)) {
        let bg = theme::lerp(cell.bg, theme::ACCENT, 0.18);
        cell.set_bg(bg);
    }
}

/// Tint the background of every visible cell toward its region's colour.
fn region_tint(buf: &mut Buffer, area: Rect, world: &World, opts: &MapOptions) {
    let (ox, oy) = opts.origin;
    let (x1, y1) = ((ox + crate::cast!(area.width => usize)).min(world.width()), (oy + crate::cast!(area.height => usize)).min(world.height()));
    for wy in oy..y1 {
        for wx in ox..x1 {
            let i = world.region_index(wx, wy);
            let amount = if opts.selected_region == Some(i) { REGION_TINT_SELECTED } else { REGION_TINT };
            if let Some(cell) = cell_at(buf, area, opts, wx, wy) {
                let bg = theme::lerp(cell.bg, theme::region(i), amount);
                cell.set_bg(bg);
            }
        }
    }
}

/// Where region `ri`'s label starts in world coordinates: centred on the
/// region's centre cell, clamped so the whole label stays inside its
/// bounding box and the world.
pub fn region_label_origin(world: &World, ri: usize) -> (usize, usize) {
    let Some(r) = world.regions.get(ri) else { return (0, 0) };
    let w = r.0.chars().count();
    let (cx, cy) = world.region_centre(ri);
    let x = cx.saturating_sub(w.div_euclid(2)).max(r.1);
    let x = x.min(r.3.saturating_sub(w)).min(world.width().saturating_sub(w));
    (x, cy)
}

/// Draw each region's name, bold and bright, clipped (never shifted) at the
/// viewport edge.
fn region_labels(buf: &mut Buffer, area: Rect, world: &World, opts: &MapOptions) {
    for (i, r) in world.regions.iter().enumerate() {
        let (lx, ly) = region_label_origin(world, i);
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
    let (sx, sy) = (crate::cast!((wx - ox) => u16), crate::cast!((wy - oy) => u16));
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
        (glyphs::MARSH, theme::MARSH_FG, "marsh"),
        (glyphs::DEN, theme::DEN, "den / burrow"),
        (glyphs::CARCASS, theme::CARCASS, "carcass"),
        (glyphs::SEED, theme::SEED, "regrowth"),
    ]
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {

    use super::*;
    use crate::sim::world::Cell;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    struct TestSource<'a> {
        world: &'a World,
        creatures: Vec<MapCreature<'a>>,
    }

    impl MapSource for TestSource<'_> {
        fn world(&self) -> &World {
            self.world
        }
        fn living_creatures(&self) -> Vec<MapCreature<'_>> {
            self.creatures.clone()
        }
        fn creature(&self, _id: CreatureId) -> Option<MapCreature<'_>> {
            None
        }
    }

    /// A `w`×`h` all-dirt world split into two regions down the middle.
    fn two_region_world(w: usize, h: usize) -> World {
        let cell = Cell { terrain: Terrain::Dirt, biome: crate::sim::world::Biome::Grassland, elevation: 0.5, moisture: 0.5, temperature: 0.5, vegetation: 0.5, prey_pressure: 0.0, pred_pressure: 0.0, dried_from: None, parasite_load: 0.0 };
        World {
            cells: vec![cell; w * h],
            width: w,
            height: h,
            dens: vec![],
            carcasses: vec![],
            seeds: vec![],
            regions: vec![("Ab".to_string(), 0, 0, w.div_euclid(2), h), ("Cd".to_string(), w.div_euclid(2), 0, w, h)],
            region_map: vec![],
            wind: crate::sim::world::Wind::Westerly,
            water_cells_at_generation: 0,
            shore: vec![],
        }
    }

    fn draw(world: &World, opts: &MapOptions, w: u16, h: u16) -> Buffer {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let source = TestSource { world, creatures: Vec::new() };
        terminal.draw(|f| render(f.buffer_mut(), Rect::new(0, 0, w, h), &source, opts)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn creature(id: u32, x: usize, y: usize, species: SpeciesId) -> MapCreature<'static> {
        MapCreature { id: CreatureId(id), x, y, alive: true, adult: true, species, glyph: 'v', color: theme::TAN, sense_cells: 3, condition: 1.0, trail: &[], target: None }
    }

    #[test]
    fn density_field_peaks_under_the_creature_and_clamps() {
        let world = two_region_world(20, 12);
        let one = vec![creature(1, 10, 4, SpeciesId(0))];
        let f = density_field(&world, &one, SpeciesId(0));
        let at = |x: usize, y: usize| f[y * 20 + x];
        assert!((at(10, 4) - 1.0 / DENSITY_CAP).abs() < 1e-6, "peak is one creature-equivalent");
        assert!(at(10, 4) > at(12, 4) && at(12, 4) > at(14, 4), "linear falloff along the row");
        assert_eq!(at(10, 4 + crate::cast!(DENSITY_RADIUS => usize) + 1), 0.0, "outside the kernel");
        assert_eq!(at(0, 0), 0.0);
        // Another species contributes nothing.
        assert!(density_field(&world, &one, SpeciesId(1)).iter().all(|&v| v == 0.0));
        // Many creatures on one cell clamp at the cap.
        let herd: Vec<_> = (0..20).map(|i| creature(i, 10, 4, SpeciesId(0))).collect();
        let f = density_field(&world, &herd, SpeciesId(0));
        assert_eq!(f[4 * 20 + 10], 1.0);
        // Edge of the world: no panic, kernel truncated.
        let _ = density_field(&world, &[creature(1, 0, 0, SpeciesId(0))], SpeciesId(0));
    }

    #[test]
    fn species_overlay_shades_cells_and_keeps_own_species_bright() {
        let mut world = two_region_world(20, 8);
        world.cells[0].terrain = Terrain::DeepWater;
        let creatures = vec![creature(1, 10, 4, SpeciesId(0)), creature(2, 3, 4, SpeciesId(1))];
        let opts = MapOptions { overlay: Overlay::Species(SpeciesId(0)), fade_creatures: true, species_color: theme::TAN, ..MapOptions::default() };
        let backend = TestBackend::new(20, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        let source = TestSource { world: &world, creatures };
        terminal.draw(|f| render(f.buffer_mut(), Rect::new(0, 0, 20, 8), &source, &opts)).unwrap();
        let buf = terminal.backend().buffer().clone();
        // Far cells are the empty shade drawn as bare dirt; the vole cell has a shaded background.
        assert_eq!(buf[(18, 0)].symbol(), glyphs::DIRT.to_string());
        assert_eq!(buf[(11, 4)].symbol(), glyphs::shade(1.0 / DENSITY_CAP * (1.0 - 0.5 / 4.0)).to_string());
        // Deep water keeps its glyph.
        assert_eq!(buf[(0, 0)].symbol(), glyphs::DEEP_WATER.to_string());
        // The shown species is drawn at full colour; the other one is faded.
        assert_eq!(buf[(10, 4)].fg, theme::TAN);
        assert_eq!(buf[(3, 4)].fg, theme::dim(theme::TAN, 0.55));
    }

    #[test]
    fn health_overlay_colours_creatures_by_condition_and_dims_terrain() {
        let world = two_region_world(20, 8);
        let mut fit = creature(1, 10, 4, SpeciesId(0));
        fit.condition = 0.9;
        let mut strained = creature(2, 3, 4, SpeciesId(1));
        strained.condition = 0.45;
        let mut critical = creature(3, 6, 2, SpeciesId(2));
        critical.condition = 0.1;
        let creatures = vec![fit, strained, critical];
        let opts = MapOptions { overlay: Overlay::Health, fade_creatures: true, ..MapOptions::default() };
        let backend = TestBackend::new(20, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        let source = TestSource { world: &world, creatures };
        terminal.draw(|f| render(f.buffer_mut(), Rect::new(0, 0, 20, 8), &source, &opts)).unwrap();
        let buf = terminal.backend().buffer().clone();
        assert_eq!(buf[(10, 4)].fg, theme::GOOD, "healthy reads green");
        assert_eq!(buf[(3, 4)].fg, theme::WARN, "strained reads amber");
        assert_eq!(buf[(6, 2)].fg, theme::BAD, "critical reads red");
        // Terrain keeps its glyph but is dimmed under the creatures.
        let (g, fg, bg) = terrain_cell(world.cell(0, 0), false);
        assert_eq!(buf[(0, 0)].symbol(), g.to_string());
        assert_eq!(buf[(0, 0)].fg, theme::dim(fg, HEALTH_TERRAIN_DIM));
        assert_eq!(buf[(0, 0)].bg, theme::dim(bg, HEALTH_TERRAIN_DIM));
    }

    #[test]
    fn region_overlay_tints_bg() {
        let world = two_region_world(8, 4);
        let opts = MapOptions { overlay: Overlay::Region, selected_region: Some(1), ..MapOptions::default() };
        let buf = draw(&world, &opts, 8, 4);
        // Row 0 carries no label (labels sit on row 2), so its cells show the pure tint
        // over the (biome-tinted) terrain background.
        let base = terrain_cell(world.cell(0, 0), false).2;
        assert_eq!(buf[(0, 0)].bg, theme::lerp(base, theme::region(0), REGION_TINT));
        assert_eq!(buf[(7, 0)].bg, theme::lerp(base, theme::region(1), REGION_TINT_SELECTED));
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
        assert_eq!(region_label_origin(&wide, 1), (0, 2));
        let _ = draw(&wide, &opts, 6, 4);
    }
}
