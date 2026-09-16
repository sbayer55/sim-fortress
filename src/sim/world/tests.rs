#![allow(clippy::float_cmp)]

use super::*;

fn counts(world: &World) -> [usize; 9] {
    let mut c = [0usize; 9];
    for cell in &world.cells {
        c[crate::cast!(cell.terrain => usize)] += 1;
    }
    c
}

#[test]
fn size_bounds() {
    for (w, h) in [(100usize, 30usize), (200, 60), (150, 40)] {
        let world = World::generate(1, &WorldParams { width: w, height: h, ..WorldParams::default() });
        assert_eq!(world.width, w);
        assert_eq!(world.height, h);
        assert_eq!(world.cells.len(), w * h);
    }
}

#[test]
fn target_percentages_20_seeds() {
    let params = WorldParams::default();
    let total = params.width * params.height;
    for seed in 1..=20 {
        let world = World::generate(seed, &params);
        let c = counts(&world);
        let pct = |n: usize| crate::cast!(n => f32) / crate::cast!(total => f32) * 100.0;
        let water = pct(c[0] + c[1]);
        let forest = pct(c[7]);
        let rock = pct(c[8]);
        assert!((water - 20.0).abs() < 3.0, "seed {seed} water {water:.2}");
        assert!((forest - 15.0).abs() < 3.0, "seed {seed} forest {forest:.2}");
        assert!((rock - 5.0).abs() < 3.0, "seed {seed} rock {rock:.2}");
    }
}

/// Count each terrain type in the top and bottom halves of `world`.
fn split_halves(world: &World, w: usize, h: usize) -> ([usize; 9], [usize; 9]) {
    let mut top = [0usize; 9];
    let mut bottom = [0usize; 9];
    for y in 0..h {
        for x in 0..w {
            let t = crate::cast!(world.cell(x, y).terrain => usize);
            if y < h.div_euclid(2) {
                top[t] += 1;
            } else {
                bottom[t] += 1;
            }
        }
    }
    (top, bottom)
}

#[test]
fn rows_vary_vertically() {
    // Regression: the noise lattice used to be sized for `h` rows while sampling
    // at 2h, so the lower half of the world was a single repeated row.
    for (w, h) in [(150usize, 40usize), (1000, 1000)] {
        let world = World::generate(3, &WorldParams { width: w, height: h, ..WorldParams::default() });
        let mut identical_pairs = 0;
        for y in 1..h {
            let same = (0..w).all(|x| world.cell(x, y).elevation == world.cell(x, y - 1).elevation);
            if same {
                identical_pairs += 1;
            }
        }
        assert_eq!(identical_pairs, 0, "{w}x{h}: {identical_pairs} repeated rows");
        // Terrain in the top and bottom halves should differ in mix.
        let (top, bottom) = split_halves(&world, w, h);
        assert_ne!(top, bottom, "{w}x{h}");
    }
}

#[test]
fn regions_cover_world() {
    for (w, h) in [(150usize, 40usize), (200, 60), (100, 30)] {
        let world = World::generate(7, &WorldParams { width: w, height: h, ..WorldParams::default() });
        // Every cell is covered by exactly one region (no gaps, no overlap).
        let area: usize = world.regions.iter().map(|r| (r.3 - r.1) * (r.4 - r.2)).sum();
        assert_eq!(area, w * h, "{w}x{h}");
        for y in 0..h {
            for x in 0..w {
                assert_ne!(world.region_name(x, y), "The Wilds", "uncovered ({x},{y})");
            }
        }
    }
}

#[test]
fn generation_is_deterministic() {
    let params = WorldParams::default();
    assert_eq!(World::generate(11, &params), World::generate(11, &params));
    assert_ne!(World::generate(11, &params).cells, World::generate(12, &params).cells);
}

#[test]
fn age_erodes_the_relief() {
    // Older worlds are smoother: hillslope diffusion and channel incision
    // lower the mean steepest gradient, and the terrain itself changes.
    let young = World::generate(5, &WorldParams { age: 0, ..WorldParams::default() });
    let old = World::generate(5, &WorldParams { age: 30, ..WorldParams::default() });
    let roughness = |w: &World| {
        let mut sum = 0.0f32;
        for y in 0..w.height {
            for x in 1..w.width {
                sum += (w.cell(x, y).elevation - w.cell(x - 1, y).elevation).abs();
            }
        }
        sum / crate::cast!(w.cells.len() => f32)
    };
    assert!(roughness(&old) < roughness(&young) * 0.8, "young {} old {}", roughness(&young), roughness(&old));
    assert_ne!(young.cells, old.cells);
}

#[test]
fn zero_water_target_is_dry() {
    let world = World::generate(9, &WorldParams { water_pct: 0, ..WorldParams::default() });
    assert_eq!(world.water_cells_at_generation, 0);
    assert!(world.cells.iter().all(|c| !c.terrain.is_water()));
}

#[test]
fn water_forms_bodies_not_speckle() {
    // Rivers drain into lakes, the ocean or off the map, so every water cell
    // that is not a one-cell lake touches other water or the edge.
    let world = World::generate(4, &WorldParams::default());
    let mut lonely = 0;
    for y in 0..world.height {
        for x in 0..world.width {
            if !world.cell(x, y).terrain.is_water() {
                continue;
            }
            let on_edge = x == 0 || y == 0 || x + 1 == world.width || y + 1 == world.height;
            // `is_shore` is "8-adjacent to water", for water cells too.
            let neighbours = usize::from(world.is_shore(x, y));
            if neighbours == 0 && !on_edge {
                lonely += 1;
            }
        }
    }
    let water = world.water_cells_at_generation;
    assert!(lonely * 50 < water, "{lonely} isolated water cells of {water}");
}
