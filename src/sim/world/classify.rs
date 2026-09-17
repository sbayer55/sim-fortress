//! Turn the relief into cells: ocean, lakes and rivers up to the water
//! target (rivers widened by tier, fanned into deltas where a trunk meets
//! the sea), moisture from rain, water proximity and altitude (with a bonus
//! along river corridors), then rock, marsh, sand, forest and grass by
//! quantile so the percentage targets hold on any seed. A river crossing
//! steep rock keeps the rock as a fall; a brook in an arid biome is a dry
//! wash the ecology refills.

use crate::sim::params::{Rainfall, WorldParams};
use crate::sim::rng::Rng;

use super::flow::{highest, lowest, Grid, Tier, Tiers};
use super::noise::Noise;
use super::relief::Relief;
use super::{biome, Biome, Cell, Terrain};

/// Share of the water target spent on lakes (at most) and on rivers.
const LAKE_SHARE: f32 = 0.25;
const RIVER_SHARE: f32 = 0.22;
/// Depressions shallower than this stay dry hollows.
const LAKE_MIN_DEPTH: f32 = 0.004;
/// A channel needs at least this many cells draining through it.
const RIVER_MIN_AREA: f32 = 6.0;
/// Share of the river budget held back for delta fans before the smallest
/// brooks are cut; whatever the deltas leave goes back to brooks.
const DELTA_SHARE: f32 = 0.15;
/// A lake needs this many cells before a trunk fans a delta into it.
const DELTA_LAKE_MIN: usize = 12;
/// Delta fan: land within this rise of the mouth becomes shallow water,
/// land up to `BAR_RISE` above it is a sand bar, and flat land up to
/// `PLAIN_RISE` in the wider ring is floodplain marsh.
const FAN_RISE: f32 = 0.015;
const BAR_RISE: f32 = 0.03;
const PLAIN_RISE: f32 = 0.05;
/// Fan reach in cells (columns, rows) and the floodplain ring beyond it.
const FAN_REACH: (i32, i32) = (2, 1);
const PLAIN_REACH: (i32, i32) = (4, 2);
/// A trunk fans a delta where the land around its mouth is in this
/// bottom share of the land's slopes (coasts are rarely in the flattest
/// decile, so the cut is the marsh quintile).
const DELTA_SLOPE_QUANTILE: f32 = 0.2;
/// A channel cell in this top share of the channels' slopes is a fall.
const FALL_SLOPE_QUANTILE: f32 = 0.95;
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
pub(super) enum Water {
    Land,
    Ocean,
    Lake,
    River,
}

/// What a river cell is: the channel itself (by tier), a widened bank
/// beside it, or the shallow fan of a delta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Channel {
    None,
    Course(Tier),
    /// A brook through arid country: its bed, generated dry.
    Wash,
    Bank,
    Fan,
}

/// Delta land: the sand bars inside a fan and the floodplain around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Delta {
    None,
    Bar,
    Plain,
}

/// The water layout before the land is cut.
#[derive(Debug)]
pub(super) struct Bodies {
    pub(super) water: Vec<Water>,
    pub(super) channel: Vec<Channel>,
    pub(super) delta: Vec<Delta>,
    /// Channel slope at `FALL_SLOPE_QUANTILE`: a river this steep falls.
    pub(super) steep: f32,
}

/// The cells, plus the row-major `(x, y)` positions of the waterfalls.
pub(super) fn cells(rng: &mut Rng, grid: Grid, relief: &Relief, params: &WorldParams) -> (Vec<Cell>, Vec<(usize, usize)>) {
    let bodies = water_bodies(grid, relief, params);
    // A dry wash waters nothing: its banks see the nearest real water.
    let wet: Vec<bool> = (0..grid.len()).map(|i| bodies.water[i] != Water::Land && bodies.channel[i] != Channel::Wash).collect();
    let distance = water_distance(grid, &wet);
    let mut moisture = moisture(relief, &wet, &distance, params.rainfall);
    riparian(grid, relief, &bodies, &wet, &mut moisture);
    let biomes = biome::label(grid, &relief.temperature, &moisture);
    let mut terrain = water_terrain(grid, &bodies);
    let falls = land_terrain(grid, relief, &bodies, &moisture, &biomes, params, &mut terrain);
    let veg = Noise::new(rng, 5.0, crate::cast!(grid.w => f32), crate::cast!(grid.h => f32) * 2.0);
    let cells = (0..grid.len())
        .map(|i| {
            let (x, y) = (crate::cast!(i % grid.w => f32), crate::cast!(i.div_euclid(grid.w) => f32) * 2.0);
            Cell {
                terrain: terrain[i],
                biome: biomes[i],
                elevation: relief.height[i],
                moisture: moisture[i],
                temperature: relief.temperature[i],
                vegetation: vegetation(terrain[i], veg.at(x, y), moisture[i], relief.temperature[i]) * biomes[i].vegetation_scale(),
                prey_pressure: 0.0,
                pred_pressure: 0.0,
                dried_from: (bodies.channel[i] == Channel::Wash).then_some(Terrain::ShallowWater),
                parasite_load: 0.0,
            }
        })
        .collect();
    (cells, falls.iter().map(|&i| (i % grid.w, i.div_euclid(grid.w))).collect())
}

/// Is the brook at `i` a seasonal wash? Arid is judged by the climate the
/// brook runs through (rain and altitude, as if the brook were not there):
/// its own water dampens its cell and banks, so the biome label there is
/// never desert or steppe. A wash is generated dry, as sand remembering the
/// water it carried, so the ecology's re-wet rule refills it when the
/// region's moisture recovers. It costs nothing from the river budget.
fn is_wash(relief: &Relief, i: usize, rainfall: Rainfall) -> bool {
    Biome::classify(relief.temperature[i], moisture_at(relief, i, 0.0, rainfall)).is_arid()
}

fn share(total: usize, pct: u8) -> usize {
    (crate::cast!(pct => usize) * total).div_euclid(100).min(total)
}

fn portion(count: usize, s: f32) -> usize {
    crate::cast!((crate::cast!(count => f32) * s).floor() => usize)
}

/// Lakes fill the deepest depressions, rivers the biggest channels, and the
/// ocean takes the lowest of what is left, so together they meet `water_pct`.
/// Rivers are cut from the largest drainage down: each channel cell costs
/// one from the river budget, a river adds a bank and a trunk two, and part
/// of the budget is held back for the deltas where trunks meet the sea.
pub(super) fn water_bodies(grid: Grid, relief: &Relief, params: &WorldParams) -> Bodies {
    let n = grid.len();
    let target = share(n, params.water_pct);
    let mut water = vec![Water::Land; n];
    let mut channel = vec![Channel::None; n];
    let mut delta = vec![Delta::None; n];

    let basins = highest((0..n).filter(|&i| relief.depth[i] > LAKE_MIN_DEPTH), &relief.depth, portion(target, LAKE_SHARE));
    for &i in &basins {
        water[i] = Water::Lake;
    }

    let rivers = portion(target, RIVER_SHARE);
    let ocean = target.saturating_sub(basins.len() + rivers);
    for &i in &lowest((0..n).filter(|&i| water[i] == Water::Land), &relief.height, ocean) {
        water[i] = Water::Ocean;
    }

    // Deltas fan onto the flattest land.
    let flat = quantile(DELTA_SLOPE_QUANTILE, (0..n).filter(|&i| water[i] == Water::Land).map(|i| relief.slope[i]));

    let mut course: Vec<usize> = (0..n).filter(|&i| water[i] == Water::Land && relief.acc[i] >= RIVER_MIN_AREA).collect();
    course.sort_by(|&a, &b| relief.acc[b].total_cmp(&relief.acc[a]).then(a.cmp(&b)));
    let tiers = Tiers::new(n, &course.iter().map(|&i| relief.acc[i]).collect::<Vec<f32>>());

    let reserve = portion(rivers, DELTA_SHARE);
    let mut budget = rivers - reserve;
    let mut next = 0;
    let cut = |next: &mut usize, budget: &mut usize, water: &mut Vec<Water>, channel: &mut Vec<Channel>| {
        cut_channels(grid, relief, tiers, params.rainfall, &course, next, budget, water, channel);
    };
    cut(&mut next, &mut budget, &mut water, &mut channel);
    budget += reserve;
    fan_deltas(grid, relief, flat, &mut budget, &mut water, &mut channel, &mut delta);
    cut(&mut next, &mut budget, &mut water, &mut channel);
    // Falls form on the steepest of the channels.
    let steep = quantile(FALL_SLOPE_QUANTILE, (0..n).filter(|&i| matches!(channel[i], Channel::Course(_))).map(|i| relief.slope[i]));
    Bodies { water, channel, delta, steep }
}

/// The value `q` (0..=1) of the way up the sorted `values` (0 when empty).
fn quantile(q: f32, values: impl Iterator<Item = f32>) -> f32 {
    let mut v: Vec<f32> = values.collect();
    v.sort_by(f32::total_cmp);
    let at = crate::cast!((crate::cast!(v.len() => f32) * q).floor() => usize);
    v.get(at.min(v.len().saturating_sub(1))).copied().unwrap_or(0.0)
}

/// Walk `course` (largest drainage first) from `next`, spending `budget` on
/// channel cells and their banks until it runs out or the course ends.
#[allow(clippy::too_many_arguments)]
fn cut_channels(grid: Grid, relief: &Relief, tiers: Tiers, rainfall: Rainfall, course: &[usize], next: &mut usize, budget: &mut usize, water: &mut [Water], channel: &mut [Channel]) {
    while let Some(&i) = course.get(*next) {
        let tier = tiers.of(relief.acc[i]);
        match (water[i], channel[i]) {
            (Water::Land, _) if tier == Tier::Brook && is_wash(relief, i, rainfall) => {
                water[i] = Water::River;
                channel[i] = Channel::Wash;
                *next += 1;
                continue;
            }
            (Water::Land, _) => {
                if *budget == 0 {
                    return;
                }
                water[i] = Water::River;
                *budget -= 1;
            }
            // A cell already claimed as a bank is upgraded to the channel.
            (Water::River, Channel::Bank) => {}
            _ => {
                *next += 1;
                continue;
            }
        }
        channel[i] = Channel::Course(tier);
        *next += 1;
        for j in bank_sites(grid, relief, i, water).into_iter().take(tier.banks()) {
            if *budget == 0 {
                return;
            }
            water[j] = Water::River;
            channel[j] = Channel::Bank;
            *budget -= 1;
        }
    }
}

/// The land cells either side of the channel at `i`, across the flow
/// direction, lowest first. A root cell (the river leaves the map or ends
/// in a pit) takes its two lowest land neighbours instead.
fn bank_sites(grid: Grid, relief: &Relief, i: usize, water: &[Water]) -> Vec<usize> {
    let (w, h) = (crate::cast!(grid.w => i32), crate::cast!(grid.h => i32));
    let pos = |c: usize| (crate::cast!(c % grid.w => i32), crate::cast!(c.div_euclid(grid.w) => i32));
    let (x, y) = pos(i);
    let r = relief.recv[i];
    let mut sites: Vec<usize> = if r == i {
        let mut around = Vec::with_capacity(8);
        grid.for_neighbours(i, |j, _| around.push(j));
        around
    } else {
        let (rx, ry) = pos(r);
        let (dx, dy) = (rx - x, ry - y);
        [(x - dy, y + dx), (x + dy, y - dx)]
            .into_iter()
            .filter(|&(sx, sy)| sx >= 0 && sy >= 0 && sx < w && sy < h)
            .map(|(sx, sy)| crate::cast!(sy => usize) * grid.w + crate::cast!(sx => usize))
            .collect()
    };
    sites.retain(|&j| water[j] == Water::Land);
    sites.sort_by(|&a, &b| relief.height[a].total_cmp(&relief.height[b]).then(a.cmp(&b)));
    sites.truncate(2);
    sites
}

/// Where a trunk reaches the ocean or a sizeable lake over flat ground, the
/// last stretch fans out: low land around the mouth becomes shallow water
/// with sand bars where it rises a little, and the flat ring beyond is
/// floodplain for the marsh pass.
fn fan_deltas(grid: Grid, relief: &Relief, flat: f32, budget: &mut usize, water: &mut [Water], channel: &mut [Channel], delta: &mut [Delta]) {
    let n = grid.len();
    let lake_size = lake_sizes(grid, water);
    // A mouth: the last trunk cell before the sea, on flat ground (the mean
    // slope of the land the fan would spread over).
    let flat_around = |m: usize| {
        let (mut sum, mut count) = (0.0f32, 0usize);
        window(grid, m, FAN_REACH, |j| {
            if water[j] == Water::Land {
                sum += relief.slope[j];
                count += 1;
            }
        });
        count > 0 && sum / crate::cast!(count => f32) <= flat
    };
    let mut mouths: Vec<usize> = (0..n)
        .filter(|&i| {
            let r = relief.recv[i];
            channel[i] == Channel::Course(Tier::Trunk)
                && r != i
                && (water[r] == Water::Ocean || (water[r] == Water::Lake && lake_size[r] >= DELTA_LAKE_MIN))
                && flat_around(i)
        })
        .collect();
    mouths.sort_by(|&a, &b| relief.acc[b].total_cmp(&relief.acc[a]).then(a.cmp(&b)));
    for m in mouths {
        let base = relief.height[m];
        window(grid, m, FAN_REACH, |j| {
            if water[j] != Water::Land {
                return;
            }
            let rise = relief.height[j] - base;
            if rise <= FAN_RISE {
                if *budget > 0 {
                    water[j] = Water::River;
                    channel[j] = Channel::Fan;
                    *budget -= 1;
                }
            } else if rise <= BAR_RISE {
                delta[j] = Delta::Bar;
            }
        });
        window(grid, m, PLAIN_REACH, |j| {
            if water[j] == Water::Land && delta[j] == Delta::None && relief.slope[j] <= flat && relief.height[j] - base <= PLAIN_RISE {
                delta[j] = Delta::Plain;
            }
        });
    }
}

/// Call `f` on every in-bounds cell within `reach` (columns, rows) of `c`.
fn window(grid: Grid, c: usize, reach: (i32, i32), mut f: impl FnMut(usize)) {
    let (w, h) = (crate::cast!(grid.w => i32), crate::cast!(grid.h => i32));
    let (cx, cy) = (crate::cast!(c % grid.w => i32), crate::cast!(c.div_euclid(grid.w) => i32));
    for dy in -reach.1..=reach.1 {
        for dx in -reach.0..=reach.0 {
            let (x, y) = (cx + dx, cy + dy);
            if x >= 0 && y >= 0 && x < w && y < h {
                f(crate::cast!(y => usize) * grid.w + crate::cast!(x => usize));
            }
        }
    }
}

/// Size of the 8-connected lake each lake cell belongs to (0 elsewhere).
fn lake_sizes(grid: Grid, water: &[Water]) -> Vec<usize> {
    let n = grid.len();
    let mut size = vec![0usize; n];
    let mut seen = vec![false; n];
    for start in 0..n {
        if seen[start] || water[start] != Water::Lake {
            continue;
        }
        let mut members = vec![start];
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            grid.for_neighbours(i, |j, _| {
                if !seen[j] && water[j] == Water::Lake {
                    seen[j] = true;
                    stack.push(j);
                    members.push(j);
                }
            });
        }
        for &i in &members {
            size[i] = members.len();
        }
    }
    size
}

/// Chamfer distance to the nearest `wet` cell in horizontal units (a
/// vertical or diagonal step is two, matching the 2:1 cell aspect).
fn water_distance(grid: Grid, wet: &[bool]) -> Vec<f32> {
    let (w, h) = (grid.w, grid.h);
    let far = crate::cast!(w + 2 * h => f32) * 2.0;
    let mut d: Vec<f32> = wet.iter().map(|&k| if k { 0.0 } else { far }).collect();
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
fn moisture(relief: &Relief, wet: &[bool], distance: &[f32], rainfall: Rainfall) -> Vec<f32> {
    (0..relief.height.len())
        .map(|i| {
            let near = if wet[i] { 1.0 } else { (-distance[i] / MOISTURE_REACH).exp() };
            moisture_at(relief, i, near, rainfall)
        })
        .collect()
}

/// Moisture at `i` given its nearness to water (1 on water, falling off to
/// 0 far from it).
fn moisture_at(relief: &Relief, i: usize, near: f32, rainfall: Rainfall) -> f32 {
    let bias = match rainfall {
        Rainfall::Dry => -0.03,
        Rainfall::Normal => 0.0,
        Rainfall::Wet => 0.03,
    };
    // Stretch the rain field's contrast so rain shadows leave bare dirt.
    let rain = ((relief.rain[i] - 1.0) * RAIN_CONTRAST + 0.5).clamp(0.0, 1.0);
    (0.02 + 0.40 * rain + 0.28 * near + 0.28 * (1.0 - relief.height[i]) + bias).clamp(0.0, 1.0)
}

/// Riparian corridors: land within reach of a river that drains enough
/// catchment gets wetter soil, so the forest and grass quantiles favour the
/// banks and a dry basin keeps a green thread along its trunk river.
fn riparian(grid: Grid, relief: &Relief, bodies: &Bodies, wet: &[bool], moisture: &mut [f32]) {
    let (w, h) = (grid.w, grid.h);
    let mut bonus = vec![0.0f32; grid.len()];
    for i in 0..grid.len() {
        if bodies.water[i] != Water::River || !wet[i] || relief.acc[i] < RIPARIAN_MIN_AREA {
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
                if wet[j] {
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

/// Water cells: interior ocean and lake cells and trunk channels are deep,
/// everything else (shores, brooks, banks, fans) is shallow. Land is
/// provisionally dirt.
fn water_terrain(grid: Grid, bodies: &Bodies) -> Vec<Terrain> {
    let water = &bodies.water;
    (0..grid.len())
        .map(|i| match water[i] {
            Water::Land => Terrain::Dirt,
            Water::River if bodies.channel[i] == Channel::Course(Tier::Trunk) => Terrain::DeepWater,
            Water::River if bodies.channel[i] == Channel::Wash => Terrain::Sand,
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

/// Land by quantile: rock on the highest, steepest ground (a river crossing
/// it over a steep drop keeps the rock as a fall); marsh on the flattest wet
/// ground beside water and on delta floodplains; sand on the lowest ocean
/// shores and delta bars; forest on the wettest remainder whose biome grows
/// trees; grass bands by moisture. Returns the fall cells, ascending.
fn land_terrain(grid: Grid, relief: &Relief, bodies: &Bodies, moisture: &[f32], biomes: &[Biome], params: &WorldParams, terrain: &mut [Terrain]) -> Vec<usize> {
    let n = grid.len();
    let water = &bodies.water;
    let is_land = |i: usize| water[i] == Water::Land;
    let max_slope = relief.slope.iter().copied().fold(0.0f32, f32::max).max(1.0e-6);
    let rock_score: Vec<f32> = (0..n).map(|i| relief.height[i] + 0.6 * relief.slope[i] / max_slope).collect();
    // Bedrock an event bared (a dome's core, ice-scoured tops) is rock
    // first, paid from the same budget; the quantile fills the rest.
    let rock_budget = share(n, params.rock_pct);
    let bedrock: Vec<usize> = (0..n).filter(|&i| is_land(i) && relief.bedrock[i]).take(rock_budget).collect();
    for &i in &bedrock {
        terrain[i] = Terrain::Rock;
    }
    for &i in &highest((0..n).filter(|&i| is_land(i) && !relief.bedrock[i]), &rock_score, rock_budget - bedrock.len()) {
        terrain[i] = Terrain::Rock;
    }
    let open = |i: usize, terrain: &[Terrain]| is_land(i) && terrain[i] == Terrain::Dirt;
    let touches = |i: usize, kind: Water| {
        let mut yes = false;
        grid.for_neighbours(i, |j, _| yes |= water[j] == kind);
        yes
    };
    // Falls: where a channel drops steepest it has not cut through the
    // bedrock, so the cell stays rock under the white water. A lone channel
    // head on the map's edge has no river to fall from and stays water.
    let wet_beside = |i: usize| {
        let mut yes = false;
        grid.for_neighbours(i, |j, _| yes |= water[j] != Water::Land);
        yes
    };
    let falls: Vec<usize> = (0..n).filter(|&i| matches!(bodies.channel[i], Channel::Course(_)) && relief.slope[i] >= bodies.steep && wet_beside(i)).collect();
    for &i in &falls {
        terrain[i] = Terrain::Rock;
    }

    // Marsh: the bottom slope quintile of the land and wet; delta
    // floodplains first, then cells fed by a catchment or rimming a lake.
    let marsh_cap = portion(n, MARSH_SHARE);
    let land_count = (0..n).filter(|&i| is_land(i)).count();
    let flat = lowest((0..n).filter(|&i| is_land(i)), &relief.slope, portion(land_count, MARSH_SLOPE_QUANTILE));
    let flat_slope = flat.iter().map(|&i| relief.slope[i]).fold(0.0f32, f32::max);
    let plain = |i: usize, terrain: &[Terrain]| open(i, terrain) && bodies.delta[i] == Delta::Plain && relief.slope[i] <= flat_slope && moisture[i] >= MARSH_MIN_MOISTURE;
    let plains = highest((0..n).filter(|&i| plain(i, terrain)), moisture, marsh_cap);
    for &i in &plains {
        terrain[i] = Terrain::Marsh;
    }
    let marsh_candidates = (0..n).filter(|&i| {
        open(i, terrain)
            && relief.slope[i] <= flat_slope
            && moisture[i] >= MARSH_MIN_MOISTURE
            && (relief.acc[i] >= MARSH_MIN_AREA || touches(i, Water::Lake))
    });
    for &i in &highest(marsh_candidates, moisture, marsh_cap - plains.len()) {
        terrain[i] = Terrain::Marsh;
    }

    let bars: Vec<usize> = (0..n).filter(|&i| open(i, terrain) && bodies.delta[i] == Delta::Bar).collect();
    for &i in &bars {
        terrain[i] = Terrain::Sand;
    }
    let shore: Vec<usize> = (0..n).filter(|&i| open(i, terrain) && touches(i, Water::Ocean)).collect();
    for &i in &lowest(shore.into_iter(), &relief.height, portion(n, SAND_SHARE)) {
        terrain[i] = Terrain::Sand;
    }

    // Forest goes to the wettest open land in a wooded biome; if the treeless
    // biomes hold so much of the land that the target cannot be met there,
    // the wettest of the rest make up the shortfall so `forest_pct` holds.
    let target = share(n, params.forest_pct);
    let wooded = highest((0..n).filter(|&i| open(i, terrain) && biomes[i].allows_forest()), moisture, target);
    for &i in &wooded {
        terrain[i] = Terrain::Forest;
    }
    for &i in &highest((0..n).filter(|&i| open(i, terrain)), moisture, target - wooded.len()) {
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
    falls
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
