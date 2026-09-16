//! Turn the relief into cells: ocean, lakes and rivers up to the water
//! target, moisture from rain, water proximity and altitude (with a bonus
//! along river corridors), then rock, marsh, sand, forest and grass by
//! quantile so the percentage targets hold on any seed.

use crate::sim::params::{Rainfall, WorldParams};
use crate::sim::rng::Rng;

use super::flow::{highest, lowest, Grid};
use super::noise::Noise;
use super::relief::Relief;
use super::{Cell, Terrain};

/// Share of the water target spent on lakes (at most) and on rivers.
const LAKE_SHARE: f32 = 0.25;
const RIVER_SHARE: f32 = 0.15;
/// Depressions shallower than this stay dry hollows.
const LAKE_MIN_DEPTH: f32 = 0.004;
/// A channel needs at least this many cells draining through it.
const RIVER_MIN_AREA: f32 = 6.0;
/// Fixed beach band above the ocean's water line, as a share of all cells.
/// Lake shores are mud and reeds, not sand.
const SAND_SHARE: f32 = 0.04;
/// Marsh at most this share of all cells: the flattest, wettest land beside
/// water. Fewer appear on a dry or steep seed.
const MARSH_SHARE: f32 = 0.05;
/// Land counts as flat enough for marsh below this slope quantile.
const MARSH_SLOPE_QUANTILE: f32 = 0.2;
/// Moisture a flat cell needs before standing water collects on it.
const MARSH_MIN_MOISTURE: f32 = 0.55;
/// Drainage area (cells) that makes a flat wet cell marsh away from a lake.
const MARSH_MIN_AREA: f32 = 4.0;
/// Rivers draining at least this many cells water a riparian corridor.
const RIPARIAN_MIN_AREA: f32 = 24.0;
/// Trunk rivers with this much drainage water two cells out instead of one.
const RIPARIAN_TRUNK_AREA: f32 = 80.0;
/// Moisture added along a corridor (halved on the outer ring).
const RIPARIAN_BONUS: f32 = 0.18;
/// Chamfer distance (horizontal units) over which water dampens the land.
const MOISTURE_REACH: f32 = 5.0;
/// Contrast stretch on the rain field (0.5..=1.5) before it enters moisture.
const RAIN_CONTRAST: f32 = 1.8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Water {
    Land,
    Ocean,
    Lake,
    River,
}

pub(super) fn cells(rng: &mut Rng, grid: Grid, relief: &Relief, params: &WorldParams) -> Vec<Cell> {
    let water = water_bodies(relief, params);
    let distance = water_distance(grid, &water);
    let mut moisture = moisture(relief, &water, &distance, params.rainfall);
    riparian(grid, relief, &water, &mut moisture);
    let mut terrain = water_terrain(grid, &water);
    land_terrain(grid, relief, &water, &moisture, params, &mut terrain);
    let veg = Noise::new(rng, 5.0, crate::cast!(grid.w => f32), crate::cast!(grid.h => f32) * 2.0);
    (0..grid.len())
        .map(|i| {
            let (x, y) = (crate::cast!(i % grid.w => f32), crate::cast!(i.div_euclid(grid.w) => f32) * 2.0);
            Cell {
                terrain: terrain[i],
                elevation: relief.height[i],
                moisture: moisture[i],
                temperature: relief.temperature[i],
                vegetation: vegetation(terrain[i], veg.at(x, y), moisture[i], relief.temperature[i]),
                prey_pressure: 0.0,
                pred_pressure: 0.0,
                dried_from: None,
                parasite_load: 0.0,
            }
        })
        .collect()
}

fn share(total: usize, pct: u8) -> usize {
    (crate::cast!(pct => usize) * total).div_euclid(100).min(total)
}

fn portion(count: usize, s: f32) -> usize {
    crate::cast!((crate::cast!(count => f32) * s).floor() => usize)
}

/// Lakes fill the deepest depressions, rivers the biggest channels, and the
/// ocean takes the lowest of what is left, so together they meet `water_pct`.
fn water_bodies(relief: &Relief, params: &WorldParams) -> Vec<Water> {
    let n = relief.height.len();
    let target = share(n, params.water_pct);
    let mut water = vec![Water::Land; n];

    let basins = highest((0..n).filter(|&i| relief.depth[i] > LAKE_MIN_DEPTH), &relief.depth, portion(target, LAKE_SHARE));
    for &i in &basins {
        water[i] = Water::Lake;
    }

    let rivers = portion(target, RIVER_SHARE);
    let ocean = target.saturating_sub(basins.len() + rivers);
    for &i in &lowest((0..n).filter(|&i| water[i] == Water::Land), &relief.height, ocean) {
        water[i] = Water::Ocean;
    }

    let channels = (0..n).filter(|&i| water[i] == Water::Land && relief.acc[i] >= RIVER_MIN_AREA);
    for &i in &highest(channels, &relief.acc, rivers) {
        water[i] = Water::River;
    }
    water
}

/// Chamfer distance to the nearest water cell in horizontal units (a
/// vertical or diagonal step is two, matching the 2:1 cell aspect).
fn water_distance(grid: Grid, water: &[Water]) -> Vec<f32> {
    let (w, h) = (grid.w, grid.h);
    let far = crate::cast!(w + 2 * h => f32) * 2.0;
    let mut d: Vec<f32> = water.iter().map(|&k| if k == Water::Land { far } else { 0.0 }).collect();
    let relax = |d: &mut Vec<f32>, x: usize, y: usize, steps: &[(i32, i32, f32)]| {
        let i = y * w + x;
        for &(dx, dy, c) in steps {
            let (nx, ny) = (crate::cast!(x => i32) + dx, crate::cast!(y => i32) + dy);
            if nx >= 0 && ny >= 0 && crate::cast!(nx => usize) < w && crate::cast!(ny => usize) < h {
                let j = crate::cast!(ny => usize) * w + crate::cast!(nx => usize);
                d[i] = d[i].min(d[j] + c);
            }
        }
    };
    let forward = [(-1, -1, 2.0), (0, -1, 2.0), (1, -1, 2.0), (-1, 0, 1.0)];
    let backward = [(1, 1, 2.0), (0, 1, 2.0), (-1, 1, 2.0), (1, 0, 1.0)];
    for y in 0..h {
        for x in 0..w {
            relax(&mut d, x, y, &forward);
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            relax(&mut d, x, y, &backward);
        }
    }
    d
}

/// Moisture: the long-run rain field, nearness to water and altitude. The
/// climate setting already scaled the wind's moisture budget, so a dry world
/// keeps a wet windward coast; the small bias here only widens the spread.
/// Tuned so a normal world averages about 0.5 on land.
fn moisture(relief: &Relief, water: &[Water], distance: &[f32], rainfall: Rainfall) -> Vec<f32> {
    let bias = match rainfall {
        Rainfall::Dry => -0.03,
        Rainfall::Normal => 0.0,
        Rainfall::Wet => 0.03,
    };
    (0..relief.height.len())
        .map(|i| {
            // Stretch the rain field's contrast so rain shadows leave bare dirt.
            let rain = ((relief.rain[i] - 1.0) * RAIN_CONTRAST + 0.5).clamp(0.0, 1.0);
            let near = if water[i] == Water::Land { (-distance[i] / MOISTURE_REACH).exp() } else { 1.0 };
            (0.02 + 0.40 * rain + 0.28 * near + 0.28 * (1.0 - relief.height[i]) + bias).clamp(0.0, 1.0)
        })
        .collect()
}

/// Riparian corridors: land within reach of a river that drains enough
/// catchment gets wetter soil, so the forest and grass quantiles favour the
/// banks and a dry basin keeps a green thread along its trunk river.
fn riparian(grid: Grid, relief: &Relief, water: &[Water], moisture: &mut [f32]) {
    let (w, h) = (grid.w, grid.h);
    let mut bonus = vec![0.0f32; grid.len()];
    for i in 0..grid.len() {
        if water[i] != Water::River || relief.acc[i] < RIPARIAN_MIN_AREA {
            continue;
        }
        let reach: i32 = if relief.acc[i] >= RIPARIAN_TRUNK_AREA { 2 } else { 1 };
        let (x, y) = (crate::cast!(i % w => i32), crate::cast!(i.div_euclid(w) => i32));
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= crate::cast!(w => i32) || ny >= crate::cast!(h => i32) {
                    continue;
                }
                let j = crate::cast!(ny => usize) * w + crate::cast!(nx => usize);
                if water[j] != Water::Land {
                    continue;
                }
                let ring = dx.abs().max(dy.abs());
                let b = if ring <= 1 { RIPARIAN_BONUS } else { RIPARIAN_BONUS * 0.5 };
                bonus[j] = bonus[j].max(b);
            }
        }
    }
    for (m, b) in moisture.iter_mut().zip(&bonus) {
        *m = (*m + b).min(1.0);
    }
}

/// Water cells: interior ocean and lake cells are deep, everything else
/// (shores, rivers) is shallow. Land is provisionally dirt.
fn water_terrain(grid: Grid, water: &[Water]) -> Vec<Terrain> {
    (0..grid.len())
        .map(|i| match water[i] {
            Water::Land => Terrain::Dirt,
            Water::River => Terrain::ShallowWater,
            Water::Ocean | Water::Lake => {
                let mut interior = true;
                grid.for_neighbours(i, |j, _| interior &= water[j] == Water::Ocean || water[j] == Water::Lake);
                if interior {
                    Terrain::DeepWater
                } else {
                    Terrain::ShallowWater
                }
            }
        })
        .collect()
}

/// Land by quantile: rock on the highest, steepest ground; marsh on the
/// flattest wet ground beside water; sand on the lowest ocean shores;
/// forest on the wettest remainder; grass bands by moisture.
fn land_terrain(grid: Grid, relief: &Relief, water: &[Water], moisture: &[f32], params: &WorldParams, terrain: &mut [Terrain]) {
    let n = grid.len();
    let is_land = |i: usize| water[i] == Water::Land;
    let max_slope = relief.slope.iter().copied().fold(0.0f32, f32::max).max(1.0e-6);
    let rock_score: Vec<f32> = (0..n).map(|i| relief.height[i] + 0.6 * relief.slope[i] / max_slope).collect();
    for &i in &highest((0..n).filter(|&i| is_land(i)), &rock_score, share(n, params.rock_pct)) {
        terrain[i] = Terrain::Rock;
    }

    let open = |i: usize, terrain: &[Terrain]| is_land(i) && terrain[i] == Terrain::Dirt;
    let touches = |i: usize, kind: Water| {
        let mut yes = false;
        grid.for_neighbours(i, |j, _| yes |= water[j] == kind);
        yes
    };

    // Marsh: the bottom slope quintile of the land, wet, and either fed by a
    // catchment or rimming a lake.
    let land_count = (0..n).filter(|&i| is_land(i)).count();
    let flat = lowest((0..n).filter(|&i| is_land(i)), &relief.slope, portion(land_count, MARSH_SLOPE_QUANTILE));
    let flat_slope = flat.iter().map(|&i| relief.slope[i]).fold(0.0f32, f32::max);
    let marsh_candidates = (0..n).filter(|&i| {
        open(i, terrain)
            && relief.slope[i] <= flat_slope
            && moisture[i] >= MARSH_MIN_MOISTURE
            && (relief.acc[i] >= MARSH_MIN_AREA || touches(i, Water::Lake))
    });
    for &i in &highest(marsh_candidates, moisture, portion(n, MARSH_SHARE)) {
        terrain[i] = Terrain::Marsh;
    }

    let shore: Vec<usize> = (0..n).filter(|&i| open(i, terrain) && touches(i, Water::Ocean)).collect();
    for &i in &lowest(shore.into_iter(), &relief.height, portion(n, SAND_SHARE)) {
        terrain[i] = Terrain::Sand;
    }

    for &i in &highest((0..n).filter(|&i| open(i, terrain)), moisture, share(n, params.forest_pct)) {
        terrain[i] = Terrain::Forest;
    }

    for i in 0..n {
        if open(i, terrain) {
            let m = moisture[i];
            terrain[i] = if m < 0.28 {
                Terrain::Dirt
            } else if m < 0.42 {
                Terrain::GrassSparse
            } else if m < 0.58 {
                Terrain::Grass
            } else {
                Terrain::GrassDense
            };
        }
    }
}

/// Initial standing vegetation per terrain, shaded by the low-frequency noise
/// and scaled by the cell's climate: damp, warm ground starts lusher and cold
/// or parched ground thinner (both factors are 1 at the middle of their range).
fn vegetation(terrain: Terrain, v: f32, moisture: f32, temperature: f32) -> f32 {
    let base = match terrain {
        Terrain::DeepWater | Terrain::ShallowWater | Terrain::Rock => 0.0,
        Terrain::Sand => 0.05 * v,
        Terrain::Dirt => 0.15 * v + 0.05,
        Terrain::GrassSparse => 0.25 + 0.2 * v,
        Terrain::Grass => 0.45 + 0.25 * v,
        Terrain::GrassDense => 0.65 + 0.3 * v,
        Terrain::Forest => 0.55 + 0.25 * v,
        Terrain::Marsh => 0.6 + 0.3 * v,
    };
    let climate = (0.85 + 0.3 * (moisture - 0.5)) * (0.85 + 0.3 * (temperature - 0.5));
    (base * climate).clamp(0.0, 1.0)
}
