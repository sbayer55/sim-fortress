//! World map renderer: terrain, resources, creatures, overlays, cursor, trails.

use std::collections::HashMap;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::sim::creatures::CreatureId;
use crate::sim::species::SpeciesId;
use crate::sim::world::World;

use crate::{glyphs, theme};

mod labels;
mod overlay;
mod palette;
pub mod stack;

pub use labels::region_label_origin;
pub use overlay::{condition_color, density_cell, density_field, disease_tint, overlay_cell, parasite_cell, parasite_tint};
pub use palette::{legend, terrain_base, terrain_cell, terrain_code_cell, world_cell};
pub use stack::{Base, Disease, Layer, OverlayStack};

/// How far the terrain fades under the Health and Disease marks so the creature colours
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
    /// The layers composed this frame (S14 item 16).
    pub stack: OverlayStack,
    pub night: bool,
    pub winter: bool,
    pub cursor: Option<(usize, usize)>,
    /// Creature id to highlight and draw a trail for.
    pub follow: Option<CreatureId>,
    /// Top-left world cell shown at the top-left of the area.
    pub origin: (usize, usize),
    /// Draw creatures (false for pure terrain/overlay views).
    pub creatures: bool,
    /// Region index drawn brighter under the Regions mark.
    pub selected_region: Option<usize>,
    /// Ramp colour for the Species base (the species' own colour; a UI concern,
    /// so the caller supplies it).
    pub species_color: Color,
    /// Per-creature colour override (and forced bold) for the Disease mark
    /// (S02h) or the Parasites base (S02i). The caller computes it from the
    /// sim (see `disease_tint` / `parasite_tint`), Disease first when both are
    /// on (S14 item 17); a living creature with no entry draws as usual.
    pub creature_tint: Option<HashMap<CreatureId, (Color, bool)>>,
}

impl Default for MapOptions {
    fn default() -> Self {
        Self {
            stack: OverlayStack::PLAIN,
            night: false,
            winter: false,
            cursor: None,
            follow: None,
            origin: (0, 0),
            creatures: true,
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
    /// is lowest); drives the colour under the Health mark.
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

pub fn render(buf: &mut Buffer, area: Rect, source: &dyn MapSource, opts: &MapOptions) {
    let world = source.world();
    let living = source.living_creatures();
    let stack = &opts.stack;

    // Species-density field (S02f), computed once per frame from the living
    // set, or the species' scent grid (S02j): both paint the species ramp.
    let density = match stack.base {
        Base::Species => Some((stack.species, opts.species_color, density_field(world, &living, stack.species))),
        Base::Scent => Some((stack.species, opts.species_color, overlay::scent_field(world, stack.species))),
        _ => None,
    };

    draw_terrain(buf, area, world, opts, density.as_ref());

    // Region tint (under everything else).
    if stack.regions {
        region_tint(buf, area, world, opts);
    }
    // Sense ring: the interior tint applies over the region tint (S14 edge cases).
    draw_sense_ring(buf, area, source, world, opts);

    draw_resources(buf, area, world, opts);

    // Trail for the followed creature.
    draw_trail(buf, area, source, opts);

    // Region and feature labels (under creatures so a passing creature
    // stays visible).
    if stack.regions {
        labels::feature_labels(buf, area, world, opts);
        labels::region_labels(buf, area, world, opts);
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

/// The glyph and colours of one cell before the marks: the base heatmap
/// (S02a/b/c/f/i rules), or the terrain.
fn base_cell(world: &World, wx: usize, wy: usize, opts: &MapOptions, density: Option<&(SpeciesId, Color, Vec<f32>)>) -> (char, Color, Color) {
    let cell = world.cell(wx, wy);
    match (density, opts.stack.base) {
        (Some((sp, color, field)), _) => density_cell(cell, field[wy * world.width() + wx], *sp, *color),
        (None, base @ (Base::Vegetation | Base::Pressure | Base::Moisture | Base::Parasites)) => overlay_cell(cell, base).unwrap_or_else(|| world_cell(world, wx, wy, opts.winter)),
        // Health and Disease over plain terrain draw the terrain without the
        // waterfall mark, as the single overlays did.
        (None, Base::None | Base::Species | Base::Scent) if opts.stack.health || opts.stack.disease.is_on() => terrain_cell(cell, opts.winter),
        (None, Base::None | Base::Species | Base::Scent) => world_cell(world, wx, wy, opts.winter),
    }
}

/// The terrain / overlay layer under everything else: the base cell, then
/// the Health dim (S02g item 19) and the Disease ground tint (S02h item 20).
fn draw_terrain(buf: &mut Buffer, area: Rect, world: &World, opts: &MapOptions, density: Option<&(SpeciesId, Color, Vec<f32>)>) {
    let (ox, oy) = opts.origin;
    let dim = opts.stack.health || opts.stack.disease.is_on();
    let ground_tint = opts.stack.disease.is_on();
    for sy in 0..area.height {
        for sx in 0..area.width {
            let (wx, wy) = (ox + crate::cast!(sx => usize), oy + crate::cast!(sy => usize));
            let Some(c) = buf.cell_mut((area.x + sx, area.y + sy)) else { continue };
            if wx >= world.width() || wy >= world.height() {
                c.set_char(' ');
                c.set_style(Style::default().bg(theme::BG));
                continue;
            }
            let (g, mut fg, mut bg) = base_cell(world, wx, wy, opts, density);
            if dim {
                fg = theme::dim(fg, HEALTH_TERRAIN_DIM);
                bg = theme::dim(bg, HEALTH_TERRAIN_DIM);
            }
            if ground_tint && world.cell(wx, wy).parasite_load >= PARASITE_TINT_THRESHOLD {
                bg = theme::lerp(bg, theme::WARN, PARASITE_TINT);
            }
            c.set_char(g);
            c.set_style(Style::default().fg(tint_color(fg, opts.night)).bg(tint_color(bg, opts.night)));
        }
    }
}

/// Seeds, dens and carcasses.
fn draw_resources(buf: &mut Buffer, area: Rect, world: &World, opts: &MapOptions) {
    let (ox, oy) = opts.origin;
    let res_fade = if opts.stack.fades_creatures() { 0.5 } else { 0.0 };
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

/// The sense subject drawn this frame, when the Sense mark is on and it lives.
fn sense_subject<'a>(source: &'a dyn MapSource, opts: &MapOptions) -> Option<MapCreature<'a>> {
    if !opts.stack.sense {
        return None;
    }
    source.creature(opts.stack.sense_subject?)
}

/// The detection ellipse of the sense subject.
fn draw_sense_ring(buf: &mut Buffer, area: Rect, source: &dyn MapSource, world: &World, opts: &MapOptions) {
    let (ox, oy) = opts.origin;
    if let Some(c) = sense_subject(source, opts) {
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

/// A living creature's colour and whether it is forced bold (S14 item 17).
///
/// Precedence: Health band, then the caller's Disease / Parasites tint, then
/// the Species base (own species full, others faded), then the plain fade
/// under any base heatmap, then the species colour. The sense subject is
/// bright under every combination.
pub fn creature_color(stack: &OverlayStack, c: &MapCreature<'_>, tint: Option<&(Color, bool)>) -> (Color, bool) {
    let (color, forced) = if stack.health {
        (condition_color(c.condition), false)
    } else if let Some(&(tc, force_bold)) = tint {
        (tc, force_bold)
    } else if stack.base == Base::Species && stack.species == c.species {
        (c.color, false)
    } else if stack.fades_creatures() {
        (theme::dim(c.color, HEALTHY_FADE), false)
    } else {
        (c.color, false)
    };
    if stack.sense && stack.sense_subject == Some(c.id) {
        return (theme::TEXT_BRIGHT, forced);
    }
    (color, forced)
}

/// Every living creature, plus the follow highlight.
fn draw_creatures(buf: &mut Buffer, area: Rect, living: &[MapCreature<'_>], opts: &MapOptions) {
    let (ox, oy) = opts.origin;
    if opts.creatures {
        let mut followed_pos: Option<(usize, usize)> = None;
        for c in living {
            if !c.alive {
                put_cell(buf, area, ox, oy, c.x, c.y, glyphs::CARCASS, tint_color(theme::CARCASS, opts.night), false);
                continue;
            }
            let tint = opts.creature_tint.as_ref().and_then(|m| m.get(&c.id));
            let (color, force_bold) = creature_color(&opts.stack, c, tint);
            put_cell(buf, area, ox, oy, c.x, c.y, c.glyph, tint_color(color, opts.night), c.adult || force_bold);
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

pub(super) fn cell_at<'a>(buf: &'a mut Buffer, area: Rect, opts: &MapOptions, wx: usize, wy: usize) -> Option<&'a mut ratatui::buffer::Cell> {
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

#[cfg(test)]
mod tests;
