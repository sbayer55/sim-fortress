//! The terrain palette: glyph and colours for a cell, with the biome tint,
//! the winter variants and the waterfall mark, plus the map legend.

use ratatui::style::Color;

use crate::sim::world::{Cell, Terrain, World};
use crate::{glyphs, theme};

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

/// Glyph and style for the cell at `(x, y)` of `world`: the terrain, or the
/// waterfall mark where a river drops over rock.
pub fn world_cell(world: &World, x: usize, y: usize, winter: bool) -> (char, Color, Color) {
    if world.is_fall(x, y) {
        return (glyphs::FALLS, theme::FALLS_FG, theme::FALLS_BG);
    }
    terrain_cell(world.cell(x, y), winter)
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
        (glyphs::FALLS, theme::FALLS_FG, "waterfall"),
        (glyphs::DEN, theme::DEN, "den / burrow"),
        (glyphs::CARCASS, theme::CARCASS, "carcass"),
        (glyphs::SEED, theme::SEED, "regrowth"),
    ]
}

