//! Per-cell and per-creature colour rules for the single overlays (S02a–k):
//! heatmap cells, the species-density field, and the health, disease and
//! parasite creature tints. Moved verbatim from `map.rs`.

use ratatui::style::Color;

use crate::sim::disease::{PathogenId, Stage};
use crate::sim::species::SpeciesId;
use crate::sim::world::{Cell, Terrain, World};
use crate::{glyphs, theme};

use crate::sim::params::SuccessionParams;

use super::{terrain_base, Base, MapCreature, DENSITY_CAP, DENSITY_RADIUS, HEALTHY_FADE, PARASITE_HEAVY, PARASITE_LIGHT};

/// Glyph and colors for a cell under a base heatmap (before creatures are
/// drawn); `None` for the `None` and `Species` bases.
pub fn overlay_cell(cell: &Cell, base: Base) -> Option<(char, Color, Color)> {
    let (t, color) = match base {
        Base::Vegetation => (cell.vegetation, theme::veg(cell.vegetation)),
        Base::Pressure => {
            let t = (cell.pred_pressure * 0.7 + cell.prey_pressure * 0.5).min(1.0);
            (t, theme::heat(t))
        }
        Base::Moisture => {
            let t = if cell.terrain.is_water() { 1.0 } else { cell.moisture };
            (t, theme::water(t))
        }
        Base::Parasites => return Some(parasite_cell(cell)),
        Base::None | Base::Species | Base::Scent | Base::Succession => return None,
    };
    if cell.terrain == Terrain::DeepWater && base != Base::Moisture {
        return Some((glyphs::DEEP_WATER, theme::dim(theme::DEEP_WATER_FG, 0.4), theme::dim(theme::DEEP_WATER_BG, 0.4)));
    }
    if cell.terrain == Terrain::Rock && base != Base::Moisture {
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

/// Scent field (S02j): the strength of `species`' mark on every world cell,
/// straight from the grid; empty blocks read as all zero.
pub(super) fn scent_field(world: &World, species: SpeciesId) -> Vec<f32> {
    let block = world.scent_block(species);
    if block.is_empty() {
        return vec![0.0; world.width() * world.height()];
    }
    block.iter().map(|m| m.strength.clamp(0.0, 1.0)).collect()
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

/// The day counts the S02k succession overlay normalises a cell's counters against.
///
/// A copy of the `[succession]` tables, so the widget never reads `Params`.
/// A zero entry means "no rung to climb into".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SuccessionScale {
    /// Thriving days needed to climb *into* each terrain, by `terrain as usize`.
    pub climb_days: [u32; Terrain::COUNT],
    /// Worn days needed to drop one rung.
    pub wear_days: u32,
}

impl From<&SuccessionParams> for SuccessionScale {
    fn from(sp: &SuccessionParams) -> Self {
        let mut climb_days = [0u32; Terrain::COUNT];
        for (t, days) in &sp.climb_days {
            climb_days[crate::cast!(*t => usize)] = *days;
        }
        Self { climb_days, wear_days: sp.wear_days_needed }
    }
}

/// The S02k field: one value per cell, `+thrive_days / climb_days[next]` for
/// a cell climbing, `−wear_days / wear_days` for one wearing, clamped to
/// ±1, and 0 for ground that is neither or off the ladder.
pub(super) fn succession_field(world: &World, scale: &SuccessionScale) -> Vec<f32> {
    world
        .cells
        .iter()
        .map(|c| {
            if !c.terrain.on_ladder() {
                return 0.0;
            }
            if c.thrive_days > 0 {
                if let Some(next) = c.next_rung() {
                    let need = scale.climb_days[crate::cast!(next => usize)];
                    if need > 0 {
                        return (f32::from(c.thrive_days) / crate::cast!(need => f32)).min(1.0);
                    }
                }
            }
            if c.wear_days > 0 && scale.wear_days > 0 && c.terrain.wear().is_some() {
                return -(f32::from(c.wear_days) / crate::cast!(scale.wear_days => f32)).min(1.0);
            }
            0.0
        })
        .collect()
}

/// Glyph and colours for a cell under the succession overlay (S02k).
///
/// Given the field value `t`: climbing ground on the vegetation ramp, wearing
/// ground on the heat ramp, an arrow once it is ripe to flip; ground that
/// is neither, and ground off the ladder, keeps a dimmed terrain glyph.
pub fn succession_cell(cell: &Cell, t: f32) -> (char, Color, Color) {
    if cell.terrain == Terrain::DeepWater {
        return (glyphs::DEEP_WATER, theme::dim(theme::DEEP_WATER_FG, 0.4), theme::dim(theme::DEEP_WATER_BG, 0.4));
    }
    if cell.terrain == Terrain::Rock {
        return (glyphs::ROCK, theme::dim(theme::ROCK_FG, 0.5), theme::dim(theme::ROCK_BG, 0.5));
    }
    if t > 0.0 {
        let c = theme::veg(t.min(1.0));
        let g = if t >= 1.0 { glyphs::UP } else { glyphs::shade(t) };
        let g = if g == ' ' { glyphs::DIRT } else { g };
        return (g, c, theme::dim(c, 0.75));
    }
    if t < 0.0 {
        let c = theme::heat((-t).min(1.0));
        let g = if t <= -1.0 { glyphs::DOWN } else { glyphs::shade(-t) };
        let g = if g == ' ' { glyphs::DIRT } else { g };
        return (g, c, theme::dim(c, 0.75));
    }
    let (g, fg, bg) = terrain_base(cell.terrain, false);
    let fade = if cell.terrain.on_ladder() { 0.6 } else { 0.45 };
    (g, theme::dim(fg, fade), theme::dim(bg, fade))
}

