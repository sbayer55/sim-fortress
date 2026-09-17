#![allow(clippy::float_cmp)]

use super::*;
use crate::sim::params::Rainfall;

fn counts(world: &World) -> [usize; 10] {
    let mut c = [0usize; 10];
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
fn split_halves(world: &World, w: usize, h: usize) -> ([usize; 10], [usize; 10]) {
    let mut top = [0usize; 10];
    let mut bottom = [0usize; 10];
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
        // Exactly eight regions, every cell in one of them, each inside its box.
        assert_eq!(world.regions.len(), REGION_COUNT, "{w}x{h}");
        let sizes: usize = (0..REGION_COUNT).map(|ri| world.region_size(ri)).sum();
        assert_eq!(sizes, w * h, "{w}x{h}");
        for y in 0..h {
            for x in 0..w {
                assert_ne!(world.region_name(x, y), "The Wilds", "uncovered ({x},{y})");
                let r = &world.regions[world.region_index(x, y)];
                assert!(x >= r.1 && x < r.3 && y >= r.2 && y < r.4, "({x},{y}) outside the box of {}", r.0);
            }
        }
        for ri in 0..REGION_COUNT {
            assert!(world.region_size(ri) > 0, "{w}x{h}: region {ri} is empty");
            assert_eq!(world.region_cells(ri).count(), world.region_size(ri));
            let (cx, cy) = world.region_centre(ri);
            assert_eq!(world.region_index(cx, cy), ri, "{w}x{h}: centre of {ri} lies outside it");
        }
    }
}

#[test]
fn regions_are_contiguous_and_named() {
    for seed in 1..=10u64 {
        let world = World::generate(seed, &WorldParams::default());
        let (w, h) = (world.width, world.height);
        let mut names = std::collections::BTreeSet::new();
        for (ri, r) in world.regions.iter().enumerate() {
            assert!(r.0.chars().count() <= NAME_MAX, "seed {seed}: {:?} is too long", r.0);
            assert!(names.insert(r.0.clone()), "seed {seed}: duplicate region name {:?}", r.0);
            // Flood-fill from the centre reaches every cell of the region.
            let (cx, cy) = world.region_centre(ri);
            let reached = flood(w, h, cy * w + cx, &mut vec![false; w * h], |j| world.region_map[j] == crate::cast!(ri => u8));
            assert_eq!(reached, world.region_size(ri), "seed {seed}: region {} ({ri}) is not contiguous", r.0);
        }
        // Every region borders at least one other, so migrations have somewhere to go.
        for a in 0..world.regions.len() {
            assert!((0..world.regions.len()).any(|b| world.regions_adjacent(a, b)), "seed {seed}: region {a} has no neighbour");
        }
    }
}

/// Size of the 4-connected component of `start` over cells accepted by
/// `member`, marking what it visits in `seen`.
fn flood(w: usize, h: usize, start: usize, seen: &mut [bool], member: impl Fn(usize) -> bool) -> usize {
    let mut stack = vec![start];
    seen[start] = true;
    let mut size = 0usize;
    while let Some(i) = stack.pop() {
        size += 1;
        let (x, y) = (i % w, i.div_euclid(w));
        let around = [(x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y > 0).then(|| i - w), (y + 1 < h).then(|| i + w)];
        for j in around.into_iter().flatten() {
            if !seen[j] && member(j) {
                seen[j] = true;
                stack.push(j);
            }
        }
    }
    size
}

#[test]
fn biomes_follow_climate_in_patches() {
    for seed in 1..=10u64 {
        let world = World::generate(seed, &WorldParams::default());
        let (w, h) = (world.width, world.height);
        // Cold biomes sit on colder cells than hot ones; forest never grows
        // in the treeless biomes.
        let mean_t = |pred: &dyn Fn(Biome) -> bool| {
            let v: Vec<f32> = world.cells.iter().filter(|c| !c.terrain.is_water() && pred(c.biome)).map(|c| c.temperature).collect();
            (!v.is_empty()).then(|| v.iter().sum::<f32>() / crate::cast!(v.len() => f32))
        };
        let cold = mean_t(&|b| matches!(b, Biome::Tundra | Biome::Taiga));
        let hot = mean_t(&|b| matches!(b, Biome::Desert | Biome::Savanna));
        if let (Some(cold), Some(hot)) = (cold, hot) {
            assert!(cold < hot, "seed {seed}: cold biomes at {cold:.2} vs hot at {hot:.2}");
        }
        assert!(world.cells.iter().all(|c| c.terrain != Terrain::Forest || c.biome.allows_forest()), "seed {seed}: forest in a treeless biome");
        // No biome patch (4-connected) smaller than MIN_PATCH cells.
        let mut seen = vec![false; w * h];
        for start in 0..w * h {
            if seen[start] {
                continue;
            }
            let b = world.cells[start].biome;
            let size = flood(w, h, start, &mut seen, |j| world.cells[j].biome == b);
            assert!(size >= MIN_PATCH, "seed {seed}: a {b:?} patch of {size} cells at {start}");
        }
    }
}

#[test]
#[ignore = "diagnostic: prints biome shares, region sizes and names per seed"]
fn print_biomes_and_regions() {
    let params = WorldParams::default();
    let mut shares = [0usize; 8];
    for seed in 1..=10u64 {
        let started = std::time::Instant::now();
        let world = World::generate(seed, &params);
        let took = started.elapsed();
        for c in &world.cells {
            shares[crate::cast!(c.biome => usize)] += 1;
        }
        let regions: Vec<String> = (0..world.regions.len()).map(|ri| format!("{} {}", world.regions[ri].0, world.region_size(ri))).collect();
        println!("seed {seed} ({took:?}): {}", regions.join(" | "));
    }
    let total = crate::cast!(shares.iter().sum::<usize>() => f32);
    for (b, n) in Biome::ALL.iter().zip(shares) {
        println!("{:<17}{:>5.1}%", b.name(), crate::cast!(n => f32) / total * 100.0);
    }
    // Land climate deciles over the same seeds, for retuning the biome table.
    let (mut temps, mut moists) = (Vec::new(), Vec::new());
    for seed in 1..=10u64 {
        for c in World::generate(seed, &params).cells.iter().filter(|c| !c.terrain.is_water()) {
            temps.push(c.temperature);
            moists.push(c.moisture);
        }
    }
    temps.sort_by(f32::total_cmp);
    moists.sort_by(f32::total_cmp);
    let deciles = |v: &[f32]| (1..10).map(|d| format!("{:.2}", v[(v.len() * d).div_euclid(10)])).collect::<Vec<_>>().join(" ");
    println!("land temperature deciles: {}", deciles(&temps));
    println!("land moisture deciles:    {}", deciles(&moists));
    let big = WorldParams { width: 200, height: 60, ..params };
    let grid = flow::Grid { w: big.width, h: big.height };
    let started = std::time::Instant::now();
    let mut rng = Rng::new(3);
    let relief = relief::build(&mut rng, grid, &big);
    let t_relief = started.elapsed();
    let (cells, _, _) = classify::cells(&mut rng, grid, &relief, &big);
    let t_cells = started.elapsed();
    let moisture: Vec<f32> = cells.iter().map(|c| c.moisture).collect();
    let t0 = std::time::Instant::now();
    let _ = biome::label(grid, &relief.temperature, &moisture);
    println!("200x60 biome::label alone: {:?}", t0.elapsed());
    let _ = regions::build(grid, &relief.basin, &relief.sea, &cells);
    let t_regions = started.elapsed();
    println!("200x60 stages: relief {t_relief:?}, +classify {:?}, +regions {:?}", t_cells - t_relief, t_regions - t_cells);
    let started = std::time::Instant::now();
    let _ = World::generate(3, &big);
    println!("200x60 generate: {:?}", started.elapsed());
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
    // The regimes, not just the epoch count, set the character: a young
    // world keeps its ridges and an old one is worn to broad valleys.
    let sharp = World::generate(5, &WorldParams { age: 2, ..WorldParams::default() });
    let worn = World::generate(5, &WorldParams { age: 14, ..WorldParams::default() });
    assert_eq!(AgeRegime::for_age(2).name, "young");
    assert_eq!(AgeRegime::for_age(14).name, "old");
    assert!(roughness(&worn) < roughness(&sharp) * 0.8, "young {} old {}", roughness(&sharp), roughness(&worn));
    assert!(!sharp.history.is_empty() && !worn.history.is_empty() && !young.history.is_empty());
}

/// Horizontal-unit distance between two cells (rows count double).
fn span(a: (usize, usize), b: (usize, usize)) -> f32 {
    let dx = crate::cast!(a.0 => f32) - crate::cast!(b.0 => f32);
    let dy = (crate::cast!(a.1 => f32) - crate::cast!(b.1 => f32)) * 2.0;
    dx.hypot(dy)
}

#[test]
fn events_leave_traces() {
    let params = WorldParams::default();
    for seed in 1..=20u64 {
        let world = World::generate(seed, &params);
        assert!((1..=3).contains(&world.history.len()), "seed {seed}: {} events", world.history.len());
        assert!(world.history.iter().all(|e| e.epoch <= params.age));
        assert!(world.history.windows(2).all(|p| p[0].epoch <= p[1].epoch));
        assert!(world.history.iter().filter(|e| e.kind == HistoryKind::Glaciation).count() <= 1);
        let land: Vec<(usize, usize)> = (0..world.height).flat_map(|y| (0..world.width).map(move |x| (x, y))).filter(|&(x, y)| !world.cell(x, y).terrain.is_water()).collect();
        let rock_share = |cells: &[(usize, usize)]| {
            let rock = cells.iter().filter(|&&(x, y)| world.cell(x, y).terrain == Terrain::Rock).count();
            crate::cast!(rock => f32) / crate::cast!(cells.len().max(1) => f32)
        };
        let world_rock = rock_share(&land);
        // Bedrock an event bared is rock on the finished map.
        let grid = flow::Grid { w: params.width, h: params.height };
        let relief = relief::build(&mut Rng::new(seed), grid, &params);
        assert_eq!(relief.history, world.history);
        let bared = land.iter().filter(|&&(x, y)| relief.bedrock[y * grid.w + x]).count();
        let bared_rock = land.iter().filter(|&&(x, y)| relief.bedrock[y * grid.w + x] && world.cell(x, y).terrain == Terrain::Rock).count();
        assert!(bared_rock >= bared.min(share_of(world.cells.len(), params.rock_pct)), "seed {seed}: {bared_rock} of {bared} bedrock cells are rock");
        for e in &world.history {
            match e.kind {
                HistoryKind::VolcanicDome => {
                    let near: Vec<(usize, usize)> = land.iter().copied().filter(|&c| span(c, (e.x, e.y)) < crate::cast!(e.extent => f32)).collect();
                    assert!(!near.is_empty() && rock_share(&near) > world_rock, "seed {seed}: dome at ({}, {}) rock {} vs {world_rock}", e.x, e.y, rock_share(&near));
                }
                HistoryKind::Glaciation => {
                    assert!(e.extent > 0 && bared > 0, "seed {seed}: glaciation bared nothing");
                }
                HistoryKind::FaultScarp => {
                    // The thrown side still stands above the other, for a
                    // scarp the later epochs have not had long to wear down.
                    let theta = f32::from(e.angle_deg).to_radians();
                    let (dx, dy) = (theta.cos(), theta.sin());
                    let (cx, cy) = (crate::cast!(e.x => f32), crate::cast!(e.y => f32) * 2.0);
                    let across = |c: &(usize, usize)| {
                        let (rx, ry) = (crate::cast!(c.0 => f32) - cx, crate::cast!(c.1 => f32) * 2.0 - cy);
                        ((rx * dx + ry * dy).abs() < crate::cast!(e.extent => f32)).then(|| -rx * dy + ry * dx)
                    };
                    let mean_height = |side: std::ops::Range<f32>| {
                        let v: Vec<f32> = land.iter().filter_map(|&c| across(&c).filter(|a| side.contains(a)).map(|_| world.cell(c.0, c.1).elevation)).collect();
                        v.iter().sum::<f32>() / crate::cast!(v.len().max(1) => f32)
                    };
                    let step = mean_height(2.0..6.0) - mean_height(-6.0..-2.0);
                    if e.epoch * 2 >= params.age {
                        assert!(step > 0.02, "seed {seed}: scarp at ({}, {}) in epoch {} steps {step}", e.x, e.y, e.epoch);
                    }
                }
            }
        }
    }
}

fn share_of(total: usize, pct: u8) -> usize {
    (crate::cast!(pct => usize) * total).div_euclid(100)
}

#[test]
#[ignore = "diagnostic: prints the age regime, the events and the generation time per age"]
fn print_history() {
    for seed in 1..=10u64 {
        let params = WorldParams::default();
        let world = World::generate(seed, &params);
        println!("seed {seed} ({}):", AgeRegime::for_age(params.age).name);
        for e in &world.history {
            println!("  {}", e.describe(params.age));
        }
    }
    for age in [2u8, 8, 20] {
        let big = WorldParams { width: 200, height: 60, age, ..WorldParams::default() };
        let started = std::time::Instant::now();
        let world = World::generate(3, &big);
        println!("200x60 age {age} ({}): {:?}, {} events", AgeRegime::for_age(age).name, started.elapsed(), world.history.len());
    }
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

/// Downwind offset for `wind`, in cells.
const fn downwind(wind: Wind) -> (i64, i64) {
    match wind {
        Wind::Westerly => (1, 0),
        Wind::Easterly => (-1, 0),
        Wind::Northerly => (0, 1),
        Wind::Southerly => (0, -1),
    }
}

#[test]
fn rain_shadows_lie_downwind() {
    // On the steepest land, the cell a few steps upwind is wetter than the
    // cell a few steps downwind: ridges rain out the windward flank and
    // shade the lee.
    let params = WorldParams::default();
    let grid = flow::Grid { w: params.width, h: params.height };
    let mut seeds_with_shadow = 0;
    for seed in 1..=20u64 {
        let mut rng = Rng::new(seed);
        let relief = relief::build(&mut rng, grid, &params);
        let (dx, dy) = downwind(relief.wind);
        let mut order: Vec<usize> = (0..grid.len()).collect();
        order.sort_by(|&a, &b| relief.slope[b].total_cmp(&relief.slope[a]));
        let (mut windward, mut lee, mut n) = (0.0f32, 0.0f32, 0);
        for &i in order.iter().take(grid.len().div_euclid(20)) {
            let (x, y) = (crate::cast!(i % grid.w => i64), crate::cast!(i.div_euclid(grid.w) => i64));
            let (ux, uy) = (x - 3 * dx, y - 3 * dy);
            let (lx, ly) = (x + 3 * dx, y + 3 * dy);
            let inside = |x: i64, y: i64| x >= 0 && y >= 0 && x < crate::cast!(grid.w => i64) && y < crate::cast!(grid.h => i64);
            if inside(ux, uy) && inside(lx, ly) {
                let at = |x: i64, y: i64| relief.rain[crate::cast!(y => usize) * grid.w + crate::cast!(x => usize)];
                windward += at(ux, uy);
                lee += at(lx, ly);
                n += 1;
            }
        }
        assert!(n > 0, "seed {seed}");
        if windward > lee {
            seeds_with_shadow += 1;
        }
    }
    assert!(seeds_with_shadow >= 18, "{seeds_with_shadow} of 20 seeds have rain shadows downwind");
}

#[test]
fn temperature_falls_with_height_and_latitude() {
    for seed in 1..=10u64 {
        let world = World::generate(seed, &WorldParams::default());
        let (w, h) = (world.width, world.height);
        // Within a row latitude is constant, so the correlation of temperature
        // against elevation there is the lapse rate: clearly negative.
        let mut rows_r = 0.0f32;
        for y in 0..h {
            let n = crate::cast!(w => f32);
            let mean_t = (0..w).map(|x| world.cell(x, y).temperature).sum::<f32>() / n;
            let mean_e = (0..w).map(|x| world.cell(x, y).elevation).sum::<f32>() / n;
            let (mut cov, mut var_t, mut var_e) = (0.0f32, 0.0f32, 0.0f32);
            for x in 0..w {
                let (dt, de) = (world.cell(x, y).temperature - mean_t, world.cell(x, y).elevation - mean_e);
                cov += dt * de;
                var_t += dt * dt;
                var_e += de * de;
            }
            rows_r += cov / (var_t * var_e).sqrt().max(1.0e-6);
        }
        let r = rows_r / crate::cast!(h => f32);
        assert!(r < -0.5, "seed {seed}: mean per-row temperature/elevation correlation {r:.2}");
        // One edge of the world is the cold one; the other is much warmer.
        let row_mean = |y: usize| (0..w).map(|x| world.cell(x, y).temperature).sum::<f32>() / crate::cast!(w => f32);
        let (top, bottom) = (row_mean(0), row_mean(h - 1));
        assert!((top - bottom).abs() > 0.3, "seed {seed}: rows {top:.2} / {bottom:.2}");
    }
}

#[test]
fn rainfall_setting_scales_moisture() {
    let land_moisture = |rainfall: Rainfall, seed: u64| {
        let world = World::generate(seed, &WorldParams { rainfall, ..WorldParams::default() });
        let land: Vec<f32> = world.cells.iter().filter(|c| !c.terrain.is_water()).map(|c| c.moisture).collect();
        land.iter().sum::<f32>() / crate::cast!(land.len() => f32)
    };
    for seed in 1..=5u64 {
        let (dry, normal, wet) = (land_moisture(Rainfall::Dry, seed), land_moisture(Rainfall::Normal, seed), land_moisture(Rainfall::Wet, seed));
        assert!(dry < normal && normal < wet, "seed {seed}: dry {dry:.3} normal {normal:.3} wet {wet:.3}");
        assert!((normal - 0.5).abs() < 0.12, "seed {seed}: normal land moisture {normal:.3}");
    }
}

#[test]
#[ignore = "diagnostic: prints moisture and terrain bands per climate"]
fn print_climate_bands() {
    for rainfall in [Rainfall::Dry, Rainfall::Normal, Rainfall::Wet] {
        let mut acc = [0usize; 10];
        let mut moist = 0.0f32;
        let mut land = 0usize;
        let mut winds = std::collections::BTreeMap::new();
        for seed in 1..=10u64 {
            let world = World::generate(seed, &WorldParams { rainfall, ..WorldParams::default() });
            *winds.entry(world.wind.name()).or_insert(0) += 1;
            for (i, c) in counts(&world).iter().enumerate() {
                acc[i] += c;
            }
            for c in world.cells.iter().filter(|c| !c.terrain.is_water()) {
                moist += c.moisture;
                land += 1;
            }
        }
        let total = crate::cast!(acc.iter().sum::<usize>() => f32);
        let pct: Vec<String> = acc.iter().map(|&n| format!("{:.1}", crate::cast!(n => f32) / total * 100.0)).collect();
        println!("{rainfall:?}: land moisture {:.3}; water/shallow/sand/dirt/sparse/grass/dense/forest/rock/marsh = {} winds {winds:?}", moist / crate::cast!(land => f32), pct.join("/"));
    }
}

#[test]
fn marsh_lies_on_flat_wet_land() {
    // Marsh is the bottom slope quintile of the land and never bone dry; it
    // appears on every default seed and touches water or a catchment.
    let params = WorldParams::default();
    let grid = flow::Grid { w: params.width, h: params.height };
    for seed in 1..=10u64 {
        let world = World::generate(seed, &params);
        // `generate` draws the relief first from a fresh rng, so rebuilding it
        // from the seed reproduces the slope field the cells were cut from.
        let relief = relief::build(&mut Rng::new(seed), grid, &params);
        // Land here is what the marsh pass saw: not water, not a dry wash bed
        // (a river cell that happens to be sand) and not a fall (rock in a river).
        let is_land = |i: usize, c: &Cell| !c.terrain.is_water() && c.dried_from.is_none() && !world.is_fall(i % world.width, i.div_euclid(world.width));
        let mut land_slopes: Vec<f32> = world.cells.iter().enumerate().filter(|(i, c)| is_land(*i, c)).map(|(i, _)| relief.slope[i]).collect();
        land_slopes.sort_by(f32::total_cmp);
        let quintile = land_slopes[land_slopes.len().div_euclid(5)];
        let marsh: Vec<usize> = world.cells.iter().enumerate().filter(|(_, c)| c.terrain == Terrain::Marsh).map(|(i, _)| i).collect();
        assert!(!marsh.is_empty(), "seed {seed}: no marsh");
        assert!(marsh.len() * 100 <= world.cells.len() * 6, "seed {seed}: {} marsh cells", marsh.len());
        for &i in &marsh {
            assert!(relief.slope[i] <= quintile, "seed {seed}: marsh at {i} on slope {}", relief.slope[i]);
            assert!(world.cells[i].moisture >= 0.55, "seed {seed}: marsh at {i} with moisture {}", world.cells[i].moisture);
        }
        // Marsh is a drinking spot in itself.
        let (x, y) = (marsh[0] % world.width, marsh[0].div_euclid(world.width));
        assert!(world.is_shore(x, y));
    }
}

/// In-bounds cell index for `(x + dx, y + dy)`, if any.
fn offset(world: &World, x: usize, y: usize, dx: i64, dy: i64) -> Option<usize> {
    let (nx, ny) = (crate::cast!(x => i64) + dx, crate::cast!(y => i64) + dy);
    (nx >= 0 && ny >= 0 && nx < crate::cast!(world.width => i64) && ny < crate::cast!(world.height => i64))
        .then(|| crate::cast!(ny => usize) * world.width + crate::cast!(nx => usize))
}

/// A river cell: shallow water with no deep water within two cells.
fn is_river(world: &World, x: usize, y: usize) -> bool {
    if world.cell(x, y).terrain != Terrain::ShallowWater {
        return false;
    }
    let mut deep = false;
    for dy in -2i64..=2 {
        for dx in -2i64..=2 {
            deep |= offset(world, x, y, dx, dy).is_some_and(|j| world.cells[j].terrain == Terrain::DeepWater);
        }
    }
    !deep
}

/// Chebyshev distance from every cell to the nearest river cell.
fn river_distance(world: &World) -> Vec<usize> {
    let w = world.width;
    let mut dist = vec![usize::MAX; world.cells.len()];
    let mut queue = std::collections::VecDeque::new();
    for i in 0..world.cells.len() {
        if is_river(world, i % w, i.div_euclid(w)) {
            dist[i] = 0;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        let (x, y) = (i % w, i.div_euclid(w));
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                let Some(j) = offset(world, x, y, dx, dy) else { continue };
                if dist[j] == usize::MAX {
                    dist[j] = dist[i] + 1;
                    queue.push_back(j);
                }
            }
        }
    }
    dist
}

#[test]
fn rivers_carry_riparian_bands() {
    // Land beside a river is wetter and greener than land a few cells away,
    // even on a dry world: the corridor is where the forest and meadow go.
    let params = WorldParams { rainfall: Rainfall::Dry, ..WorldParams::default() };
    for seed in 1..=10u64 {
        let world = World::generate(seed, &params);
        let dist = river_distance(&world);
        let mean_veg = |lo: usize, hi: usize| {
            let cells: Vec<f32> = world.cells.iter().enumerate().filter(|(i, c)| !c.terrain.is_water() && c.terrain != Terrain::Rock && (lo..=hi).contains(&dist[*i])).map(|(_, c)| c.vegetation).collect();
            cells.iter().sum::<f32>() / crate::cast!(cells.len().max(1) => f32)
        };
        let (bank, away) = (mean_veg(1, 1), mean_veg(5, 7));
        assert!(bank > away * 1.1, "seed {seed}: bank vegetation {bank:.3} vs {away:.3} five cells out");
    }
}

#[test]
#[ignore = "diagnostic: prints river tiers, deltas, falls and washes per seed"]
fn print_river_morphology() {
    use classify::{Channel, Delta, Water};
    use flow::Tier;
    for rainfall in [Rainfall::Dry, Rainfall::Normal] {
        let params = WorldParams { rainfall, ..WorldParams::default() };
        let grid = flow::Grid { w: params.width, h: params.height };
        for seed in 1..=10u64 {
            let relief = relief::build(&mut Rng::new(seed), grid, &params);
            let bodies = classify::water_bodies(grid, &relief, &params);
            let above = |a: f32| (0..grid.len()).filter(|&i| !relief.sea[i] && relief.depth[i] <= 0.004 && relief.acc[i] >= a).count();
            println!("  acc>=6 {} >=24 {} >=50 {} >=100 {} >=200 {} >=400 {} >=800 {} max {:.0}", above(6.0), above(24.0), above(50.0), above(100.0), above(200.0), above(400.0), above(800.0), relief.acc.iter().copied().fold(0.0f32, f32::max));
            let count = |f: &dyn Fn(usize) -> bool| (0..grid.len()).filter(|&i| f(i)).count();
            let brook = count(&|i| bodies.channel[i] == Channel::Course(Tier::Brook));
            let river = count(&|i| bodies.channel[i] == Channel::Course(Tier::River));
            let trunk = count(&|i| bodies.channel[i] == Channel::Course(Tier::Trunk));
            let bank = count(&|i| bodies.channel[i] == Channel::Bank);
            let fan = count(&|i| bodies.channel[i] == Channel::Fan);
            let bar = count(&|i| bodies.delta[i] == Delta::Bar);
            let plain = count(&|i| bodies.delta[i] == Delta::Plain);
            let rivers = count(&|i| bodies.water[i] == Water::River && bodies.channel[i] != Channel::Wash);
            let course = |i: usize| matches!(bodies.channel[i], Channel::Course(_));
            let steep = count(&|i| course(i) && relief.slope[i] >= bodies.steep);
            let coast = count(&|i| bodies.channel[i] == Channel::Course(Tier::Trunk) && relief.recv[i] != i && bodies.water[relief.recv[i]] == Water::Ocean);
            let mut acc_desc: Vec<f32> = (0..grid.len()).filter(|&i| bodies.water[i] == Water::River).map(|i| relief.acc[i]).collect();
            acc_desc.sort_by(|a, b| b.total_cmp(a));
            let tiers = flow::Tiers::new(grid.len(), &acc_desc);
            println!("  tiers river {:.0} trunk {:.0}; steep channel cells {steep}; trunk at coast {coast}", tiers.river, tiers.trunk);
            let started = std::time::Instant::now();
            let world = World::generate(seed, &params);
            let took = started.elapsed();
            let wash = world.cells.iter().filter(|c| c.dried_from.is_some()).count();
            let water = world.cells.iter().filter(|c| c.terrain.is_water()).count();
            let c = counts(&world);
            println!(
                "{rainfall:?} seed {seed} ({took:?}): brook {brook} river {river} trunk {trunk} bank {bank} fan {fan} = {rivers}; bar {bar} plain {plain}; falls {}; wash {wash}; water {water} rock {} marsh {}",
                world.falls.len(),
                c[8],
                c[9]
            );
        }
    }
}

#[test]
fn trunks_are_deep_banked_and_continuous() {
    use classify::{Channel, Water};
    use flow::Tier;
    let params = WorldParams::default();
    let grid = flow::Grid { w: params.width, h: params.height };
    for seed in 1..=20u64 {
        let relief = relief::build(&mut Rng::new(seed), grid, &params);
        let bodies = classify::water_bodies(grid, &relief, &params);
        let world = World::generate(seed, &params);
        let trunk: Vec<usize> = (0..grid.len()).filter(|&i| bodies.channel[i] == Channel::Course(Tier::Trunk)).collect();
        assert!(!trunk.is_empty(), "seed {seed}: no trunk river");
        // The trunk runs deep, with shallow banks beside most of it.
        for &i in &trunk {
            let (x, y) = (i % grid.w, i.div_euclid(grid.w));
            assert!(world.cells[i].terrain == Terrain::DeepWater || world.is_fall(x, y), "seed {seed}: trunk cell {i} is {:?}", world.cells[i].terrain);
        }
        let banked = trunk
            .iter()
            .filter(|&&i| {
                let mut bank = false;
                grid.for_neighbours(i, |j, _| bank |= bodies.channel[j] == Channel::Bank);
                bank
            })
            .count();
        assert!(banked * 10 >= trunk.len() * 6, "seed {seed}: {banked} of {} trunk cells have a bank", trunk.len());
        // The trunk is continuous: every trunk cell drains into river water,
        // a lake, the sea or off the map, never onto land.
        for &i in &trunk {
            let r = relief.recv[i];
            assert!(r == i || bodies.water[r] != Water::Land, "seed {seed}: trunk cell {i} drains onto land");
        }
        // The trunk is the reach with the most drainage of any channel, and
        // at least `TRUNK_MIN_CELLS` long.
        let max_course = (0..grid.len()).filter(|&i| matches!(bodies.channel[i], Channel::Course(_))).map(|i| relief.acc[i]).fold(0.0f32, f32::max);
        assert!(trunk.iter().any(|&i| relief.acc[i] == max_course), "seed {seed}: the largest channel is not trunk");
        assert!(trunk.len() >= flow::TRUNK_MIN_CELLS, "seed {seed}: trunk of {} cells", trunk.len());
    }
}

#[test]
fn deltas_fan_where_trunks_meet_the_sea() {
    use classify::{Channel, Delta};
    let params = WorldParams::default();
    let grid = flow::Grid { w: params.width, h: params.height };
    let mut seeds_with_delta = 0;
    for seed in 1..=20u64 {
        let relief = relief::build(&mut Rng::new(seed), grid, &params);
        let bodies = classify::water_bodies(grid, &relief, &params);
        let world = World::generate(seed, &params);
        let fans: Vec<usize> = (0..grid.len()).filter(|&i| bodies.channel[i] == Channel::Fan).collect();
        seeds_with_delta += usize::from(!fans.is_empty());
        for &i in &fans {
            assert_eq!(world.cells[i].terrain, Terrain::ShallowWater, "seed {seed}: fan cell {i}");
            let mut wet = false;
            grid.for_neighbours(i, |j, _| wet |= world.cells[j].terrain.is_water());
            assert!(wet, "seed {seed}: fan cell {i} touches no other water");
        }
        for i in (0..grid.len()).filter(|&i| bodies.delta[i] == Delta::Bar) {
            assert!(matches!(world.cells[i].terrain, Terrain::Sand | Terrain::Marsh), "seed {seed}: bar {i} is {:?}", world.cells[i].terrain);
        }
    }
    assert!(seeds_with_delta >= 5, "{seeds_with_delta} of 20 seeds have a delta");
}

#[test]
fn falls_sit_on_the_steepest_channels() {
    use classify::Channel;
    let params = WorldParams::default();
    let grid = flow::Grid { w: params.width, h: params.height };
    for seed in 1..=20u64 {
        let relief = relief::build(&mut Rng::new(seed), grid, &params);
        let bodies = classify::water_bodies(grid, &relief, &params);
        let world = World::generate(seed, &params);
        assert!(!world.falls.is_empty(), "seed {seed}: no falls");
        let mut sorted = world.falls.clone();
        sorted.sort_by_key(|&(x, y)| (y, x));
        assert_eq!(sorted, world.falls, "seed {seed}: falls are not row-major");
        let course = (0..grid.len()).filter(|&i| matches!(bodies.channel[i], Channel::Course(_))).count();
        assert!(world.falls.len() * 10 <= course, "seed {seed}: {} falls on {course} channel cells", world.falls.len());
        for &(x, y) in &world.falls {
            let i = y * grid.w + x;
            assert!(world.is_fall(x, y));
            assert_eq!(world.cell(x, y).terrain, Terrain::Rock, "seed {seed}: fall at ({x},{y})");
            assert_eq!(world.terrain_name(x, y), "waterfall");
            assert!(matches!(bodies.channel[i], Channel::Course(_)), "seed {seed}: fall at {i} is not on a channel");
            assert!(relief.slope[i] >= bodies.steep, "seed {seed}: fall at {i} on slope {}", relief.slope[i]);
            // The river continues past the fall: another river cell (water,
            // a fall or a wash bed) or the sea it drops into.
            let mut water = false;
            grid.for_neighbours(i, |j, _| water |= bodies.water[j] != classify::Water::Land);
            assert!(water, "seed {seed}: fall at {i} has no water beside it");
        }
    }
}

#[test]
fn washes_are_dry_brooks_that_rewet_within_the_year() {
    use crate::sim::{Params, Sim};
    use classify::Channel;
    // A dry world has brooks through arid country generated as sand that
    // remembers its water.
    let mut params = Params::default();
    params.world.rainfall = Rainfall::Dry;
    params.species.clear_initial_counts();
    let grid = flow::Grid { w: params.world.width, h: params.world.height };
    let relief = relief::build(&mut Rng::new(3), grid, &params.world);
    let bodies = classify::water_bodies(grid, &relief, &params.world);
    let mut sim = Sim::new(3, params);
    let washes = |sim: &Sim| sim.world.cells.iter().filter(|c| c.dried_from == Some(Terrain::ShallowWater)).count();
    let at_start = washes(&sim);
    assert!(at_start > 0, "no washes on a dry seed");
    for (i, c) in sim.world.cells.iter().enumerate() {
        if c.dried_from.is_some() {
            assert_eq!(c.terrain, Terrain::Sand, "wash at {i}");
            assert_eq!(bodies.channel[i], Channel::Wash, "wash at {i} is not a brook");
        }
    }
    assert_eq!(sim.world.water_cells_at_generation, sim.world.cells.iter().filter(|c| c.terrain.is_water()).count());
    // The ecology's re-wet rule refills them as the regions' moisture recovers;
    // every wash is running again before the year is out.
    let mut refilled_on = None;
    for day in 1..=360u32 {
        for _ in 0..24 {
            sim.step();
        }
        if washes(&sim) == 0 {
            refilled_on = Some((day, sim.time.season()));
            break;
        }
    }
    let (day, season) = refilled_on.unwrap_or_else(|| panic!("{} of {at_start} washes still dry after a year", washes(&sim)));
    assert!(day <= 180, "washes refilled on day {day} ({season:?})");
    let wet_again = sim.world.cells.iter().filter(|c| c.terrain.is_water()).count();
    assert!(wet_again >= sim.world.water_cells_at_generation + at_start.div_euclid(2), "{wet_again} water cells after refilling {at_start} washes");
}
