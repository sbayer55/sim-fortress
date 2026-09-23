//! Tests for the map renderer.

#![allow(clippy::float_cmp)]

use super::*;
use crate::sim::world::{Cell, Terrain};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::collections::HashMap;

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
    fn creature(&self, id: CreatureId) -> Option<MapCreature<'_>> {
        self.creatures.iter().find(|c| c.id == id).copied()
    }
}

/// A `w`×`h` all-dirt world split into two regions down the middle.
fn two_region_world(w: usize, h: usize) -> World {
    let cell = Cell { terrain: Terrain::Dirt, biome: crate::sim::world::Biome::Grassland, elevation: 0.5, moisture: 0.5, temperature: 0.5, vegetation: 0.5, prey_pressure: 0.0, pred_pressure: 0.0, dried_from: None, parasite_load: 0.0, thrive_days: 0, wear_days: 0 };
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
        falls: vec![],
        history: vec![],
        names: crate::sim::world::Names::default(),
        scent: vec![],
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

/// Render `world` with `creatures` under `opts` into a `w`×`h` buffer.
fn draw_with(world: &World, creatures: Vec<MapCreature<'_>>, opts: &MapOptions, w: u16, h: u16) -> Buffer {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    let source = TestSource { world, creatures };
    terminal.draw(|f| render(f.buffer_mut(), Rect::new(0, 0, w, h), &source, opts)).unwrap();
    terminal.backend().buffer().clone()
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
    let stack = OverlayStack { base: Base::Species, species: SpeciesId(0), ..OverlayStack::PLAIN };
    let opts = MapOptions { stack, species_color: theme::TAN, ..MapOptions::default() };
    let buf = draw_with(&world, creatures, &opts, 20, 8);
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
    let opts = MapOptions { stack: OverlayStack { health: true, ..OverlayStack::PLAIN }, ..MapOptions::default() };
    let buf = draw_with(&world, creatures, &opts, 20, 8);
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
    let opts = MapOptions { stack: OverlayStack { regions: true, ..OverlayStack::PLAIN }, selected_region: Some(1), ..MapOptions::default() };
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
    let opts = MapOptions { stack: OverlayStack { regions: true, ..OverlayStack::PLAIN }, origin: (2, 0), ..MapOptions::default() };
    let buf = draw(&world, &opts, 6, 4);
    assert_eq!(buf[(0, 2)].symbol(), "b");
    assert_eq!(buf[(1, 2)].symbol(), glyphs::DIRT.to_string());
    // A label wider than its region is clamped inside the world, never past it.
    let mut wide = two_region_world(8, 4);
    wide.regions[1].0 = "Toolongname".to_string();
    assert_eq!(region_label_origin(&wide, 1), (0, 2));
    let _ = draw(&wide, &opts, 6, 4);
}

#[test]
fn health_beats_disease_on_a_creature() {
    let world = two_region_world(20, 8);
    let mut sick = creature(1, 10, 4, SpeciesId(0));
    sick.condition = 0.1;
    let mut tints = HashMap::new();
    tints.insert(CreatureId(1), (theme::SICK, true));
    // Disease alone: the tint wins and forces bold.
    let stack = OverlayStack { disease: Disease::On(None), ..OverlayStack::PLAIN };
    assert_eq!(creature_color(&stack, &sick, tints.get(&CreatureId(1))), (theme::SICK, true));
    // Health and Disease: the condition band wins (S14 item 17).
    let both = OverlayStack { health: true, ..stack };
    assert_eq!(creature_color(&both, &sick, tints.get(&CreatureId(1))), (theme::BAD, false));
    let opts = MapOptions { stack: both, creature_tint: Some(tints), ..MapOptions::default() };
    let buf = draw_with(&world, vec![sick], &opts, 20, 8);
    assert_eq!(buf[(10, 4)].fg, theme::BAD);
    // The disease ground tint still shows under both.
    let mut fouled = two_region_world(20, 8);
    fouled.cells[0].parasite_load = 0.9;
    let buf = draw_with(&fouled, vec![], &opts, 20, 8);
    let (_, _, bg) = terrain_cell(fouled.cell(0, 0), false);
    assert_eq!(buf[(0, 0)].bg, theme::lerp(theme::dim(bg, HEALTH_TERRAIN_DIM), theme::WARN, PARASITE_TINT));
    // The sense subject is bright under every combination.
    let subject = OverlayStack { sense: true, sense_subject: Some(CreatureId(1)), ..both };
    assert_eq!(creature_color(&subject, &sick, None).0, theme::TEXT_BRIGHT);
}

#[test]
fn regions_and_sense_tints_both_apply() {
    let world = two_region_world(20, 8);
    let subject = creature(1, 10, 4, SpeciesId(0));
    let stack = OverlayStack { regions: true, sense: true, sense_subject: Some(CreatureId(1)), ..OverlayStack::PLAIN };
    let opts = MapOptions { stack, ..MapOptions::default() };
    let buf = draw_with(&world, vec![subject], &opts, 20, 8);
    // Row 0 is outside the ring (radius 3 around y = 4): the region tint alone.
    let base = terrain_cell(world.cell(0, 0), false).2;
    assert_eq!(buf[(0, 0)].bg, theme::lerp(base, theme::region(0), REGION_TINT));
    // Beside the subject: the region tint, then the ring interior tint over it.
    let regioned = theme::lerp(base, theme::region(1), REGION_TINT);
    assert_eq!(buf[(11, 4)].bg, theme::lerp(regioned, theme::ACCENT, 0.18));
    // The ring edge is drawn and the subject is bright; neither layer fades it.
    assert_eq!(buf[(10, 1)].symbol(), glyphs::RING.to_string());
    assert_eq!(buf[(10, 4)].fg, theme::TEXT_BRIGHT);
    assert!(!stack.fades_creatures());
}

#[test]
fn species_base_under_regions_keeps_density_shade() {
    let world = two_region_world(20, 8);
    let creatures = vec![creature(1, 10, 4, SpeciesId(0)), creature(2, 3, 4, SpeciesId(1))];
    let stack = OverlayStack { base: Base::Species, species: SpeciesId(0), regions: true, ..OverlayStack::PLAIN };
    let opts = MapOptions { stack, species_color: theme::TAN, ..MapOptions::default() };
    let buf = draw_with(&world, creatures, &opts, 20, 8);
    // The density shade glyph survives the region tint on its background.
    let t = 1.0 / DENSITY_CAP * (1.0 - 0.5 / 4.0);
    assert_eq!(buf[(11, 4)].symbol(), glyphs::shade(t).to_string());
    let (_, _, density_bg) = density_cell(world.cell(11, 4), t, SpeciesId(0), theme::TAN);
    assert_eq!(buf[(11, 4)].bg, theme::lerp(density_bg, theme::region(1), REGION_TINT));
    // Own species full colour, the other faded; the region labels still draw.
    assert_eq!(buf[(10, 4)].fg, theme::TAN);
    assert_eq!(buf[(3, 4)].fg, theme::dim(theme::TAN, HEALTHY_FADE));
    let (lx, ly) = region_label_origin(&world, 0);
    assert_eq!(buf[(crate::cast!(lx => u16), crate::cast!(ly => u16))].symbol(), "A");
}
