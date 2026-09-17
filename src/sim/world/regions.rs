//! Regions: the world cut into eight drainage basins.
//!
//! Every cell drains to a root of the flow tree (an ocean cell, a map edge
//! or a pit), and the cells sharing a root form a basin. Basins are merged
//! smallest-first into the neighbour they share the longest border with
//! until `REGION_COUNT` remain, so region boundaries follow ridgelines and
//! coastlines rather than a fixed grid. Each region is named for where it
//! lies and the biome that covers most of its land.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use super::flow::Grid;
use super::{Biome, Cell, RegionRect};

/// How many regions every world has (the stats and drought tables are sized
/// for exactly this many).
pub const REGION_COUNT: usize = 8;
/// Longest region name the sidebars can show.
pub const NAME_MAX: usize = 16;
/// A centroid within this much of the world's middle (as a share of the
/// world's extent) makes a region "Central".
const CENTRAL: f32 = 0.16;
/// A region holding more than this share of the world stops taking basins
/// that have another neighbour to join.
const GROWTH_CAP: f32 = 0.25;

/// Region rectangles (name and bounding box) and the region index of every
/// cell. `basin` is each cell's flow-tree root; `sea` marks ocean base level.
pub(super) fn build(grid: Grid, basin: &[usize], sea: &[bool], cells: &[Cell]) -> (Vec<RegionRect>, Vec<u8>) {
    let ids = merge_basins(grid, basin, sea);
    let count = ids.iter().max().map_or(0, |m| m + 1);
    let regions = describe(grid, &ids, count, cells);
    (regions, ids.iter().map(|&r| crate::cast!(r => u8)).collect())
}

/// Disjoint-set forest with path halving.
struct Union {
    parent: Vec<usize>,
}

impl Union {
    fn new(n: usize) -> Self {
        Self { parent: (0..n).collect() }
    }

    fn find(&mut self, mut i: usize) -> usize {
        while self.parent[i] != i {
            self.parent[i] = self.parent[self.parent[i]];
            i = self.parent[i];
        }
        i
    }
}

/// Renumber `raw` labels (each below `raw.len()`) 0.. in order of first
/// appearance.
fn compact(raw: &[usize]) -> (Vec<usize>, usize) {
    let mut map = vec![usize::MAX; raw.len()];
    let mut count = 0usize;
    let mut out = Vec::with_capacity(raw.len());
    for &r in raw {
        if map[r] == usize::MAX {
            map[r] = count;
            count += 1;
        }
        out.push(map[r]);
    }
    (out, count)
}

/// The label every cell starts from: its flow-tree root on land, and for
/// ocean cells the nearest land basin (a multi-source flood from the shore,
/// in index order), so every region is anchored on land and owns the water
/// it touches. Drainage runs over eight neighbours, so a basin can hang
/// together by a diagonal alone; each label is split into its 4-connected
/// pieces so every region is contiguous the way the map walks it.
fn seed_labels(grid: Grid, basin: &[usize], sea: &[bool]) -> Vec<usize> {
    let label = drainage_labels(grid, basin, sea);
    let n = grid.len();
    let (w, h) = (grid.w, grid.h);
    let mut piece = vec![usize::MAX; n];
    let mut stack = Vec::new();
    for start in 0..n {
        if piece[start] != usize::MAX {
            continue;
        }
        piece[start] = start;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i.div_euclid(w));
            let around = [(y > 0).then(|| i - w), (x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y + 1 < h).then(|| i + w)];
            for j in around.into_iter().flatten() {
                if piece[j] == usize::MAX && label[j] == label[i] {
                    piece[j] = start;
                    stack.push(j);
                }
            }
        }
    }
    piece
}

/// Each cell's flow-tree root on land, and for ocean cells the nearest land
/// basin.
fn drainage_labels(grid: Grid, basin: &[usize], sea: &[bool]) -> Vec<usize> {
    let n = grid.len();
    let (w, h) = (grid.w, grid.h);
    let mut label: Vec<usize> = (0..n).map(|i| if sea[i] { usize::MAX } else { basin[i] }).collect();
    let mut queue: std::collections::VecDeque<usize> = (0..n).filter(|&i| !sea[i]).collect();
    while let Some(i) = queue.pop_front() {
        let (x, y) = (i % w, i.div_euclid(w));
        let around = [(y > 0).then(|| i - w), (x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y + 1 < h).then(|| i + w)];
        for j in around.into_iter().flatten() {
            if label[j] == usize::MAX {
                label[j] = label[i];
                queue.push_back(j);
            }
        }
    }
    // A world that is all sea: one region.
    for l in &mut label {
        if *l == usize::MAX {
            *l = 0;
        }
    }
    label
}

/// Merge the basins down to `REGION_COUNT` contiguous regions, smallest
/// first into the neighbour sharing the longest border. A neighbour that
/// already holds more than `GROWTH_CAP` of the world only takes a basin
/// that has nowhere else to go, so one big watershed cannot swallow the map.
fn merge_basins(grid: Grid, basin: &[usize], sea: &[bool]) -> Vec<usize> {
    let n = grid.len();
    let (w, h) = (grid.w, grid.h);
    let (ids, count) = compact(&seed_labels(grid, basin, sea));
    let cap = crate::cast!(crate::cast!(n => f32) * GROWTH_CAP => usize).max(1);

    let mut size = vec![0usize; count];
    let mut adj: Vec<BTreeMap<usize, u32>> = vec![BTreeMap::new(); count];
    for i in 0..n {
        size[ids[i]] += 1;
        let (x, y) = (i % w, i.div_euclid(w));
        for j in [(x + 1 < w).then(|| i + 1), (y + 1 < h).then(|| i + w)].into_iter().flatten() {
            let (a, b) = (ids[i], ids[j]);
            if a != b {
                *adj[a].entry(b).or_insert(0) += 1;
                *adj[b].entry(a).or_insert(0) += 1;
            }
        }
    }

    let mut merged = Union::new(count);
    let mut alive = count;
    let mut heap: BinaryHeap<Reverse<(usize, usize)>> = (0..count).map(|b| Reverse((size[b], b))).collect();
    while alive > REGION_COUNT {
        let Some(Reverse((sz, a))) = heap.pop() else { break };
        if merged.find(a) != a || size[a] != sz {
            continue;
        }
        // The neighbour sharing the longest border; on a tie the larger
        // one, so rim cells join the land behind them rather than each other.
        let pick = |below_cap: bool| adj[a].iter().filter(|&(&b, _)| !below_cap || size[b] < cap).max_by_key(|&(&b, &len)| (len, size[b], Reverse(b))).map(|(&b, _)| b);
        let Some(b) = pick(true).or_else(|| pick(false)) else { break };
        let edges = std::mem::take(&mut adj[a]);
        for (c, len) in edges {
            adj[c].remove(&a);
            if c != b {
                *adj[b].entry(c).or_insert(0) += len;
                *adj[c].entry(b).or_insert(0) += len;
            }
        }
        adj[b].remove(&a);
        size[b] += size[a];
        size[a] = 0;
        merged.parent[a] = b;
        alive -= 1;
        heap.push(Reverse((size[b], b)));
    }
    let resolved: Vec<usize> = ids.iter().map(|&b| merged.find(b)).collect();
    compact(&resolved).0
}

/// Per-region tallies used for the bounding box and the name.
#[derive(Clone, Debug)]
struct Tally {
    cells: usize,
    land: usize,
    water: usize,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    sum_x: usize,
    sum_y: usize,
    elevation: f32,
    biomes: [usize; 8],
}

impl Tally {
    const fn new() -> Self {
        Self { cells: 0, land: 0, water: 0, x0: usize::MAX, y0: usize::MAX, x1: 0, y1: 0, sum_x: 0, sum_y: 0, elevation: 0.0, biomes: [0; 8] }
    }

    /// The biome covering most of the region's land (ties to the first).
    fn dominant(&self) -> Option<Biome> {
        let best = self.biomes.iter().copied().max()?;
        if best == 0 {
            return None;
        }
        self.biomes.iter().position(|&n| n == best).map(|k| Biome::from_code(crate::cast!(k => u8)))
    }
}

fn describe(grid: Grid, ids: &[usize], count: usize, cells: &[Cell]) -> Vec<RegionRect> {
    let mut tallies = vec![Tally::new(); count];
    for (i, (&r, cell)) in ids.iter().zip(cells).enumerate() {
        let (x, y) = (i % grid.w, i.div_euclid(grid.w));
        let t = &mut tallies[r];
        t.cells += 1;
        t.x0 = t.x0.min(x);
        t.y0 = t.y0.min(y);
        t.x1 = t.x1.max(x + 1);
        t.y1 = t.y1.max(y + 1);
        t.sum_x += x;
        t.sum_y += y;
        if cell.terrain.is_water() {
            t.water += 1;
        } else {
            t.land += 1;
            t.elevation += cell.elevation;
            t.biomes[crate::cast!(cell.biome => usize)] += 1;
        }
    }
    let land_total: usize = tallies.iter().map(|t| t.land).sum();
    let mean_elevation = tallies.iter().map(|t| t.elevation).sum::<f32>() / crate::cast!(land_total.max(1) => f32);

    let mut used = BTreeSet::new();
    tallies
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let name = candidates(grid, i, t, mean_elevation)
                .into_iter()
                .find(|c| c.chars().count() <= NAME_MAX && !used.contains(c))
                .unwrap_or_else(|| format!("Region {}", i + 1));
            used.insert(name.clone());
            (name, t.x0, t.y0, t.x1, t.y1)
        })
        .collect()
}

/// Names for a region in order of preference: its compass position and
/// biome, then the other axis, its height, and finally a numeral.
fn candidates(grid: Grid, index: usize, t: &Tally, mean_elevation: f32) -> Vec<String> {
    let n = crate::cast!(t.cells.max(1) => f32);
    let fx = crate::cast!(t.sum_x => f32) / n / crate::cast!(grid.w.max(2) - 1 => f32) - 0.5;
    let fy = crate::cast!(t.sum_y => f32) / n / crate::cast!(grid.h.max(2) - 1 => f32) - 0.5;
    let ew = if fx >= 0.0 { "Eastern" } else { "Western" };
    let ns = if fy >= 0.0 { "Southern" } else { "Northern" };
    let (primary, secondary) = if fx.abs() >= fy.abs() { (ew, ns) } else { (ns, ew) };
    let central = fx.abs() < CENTRAL && fy.abs() < CENTRAL;
    let noun = if t.water * 2 > t.cells { "Coast" } else { t.dominant().map_or("Coast", Biome::region_noun) };
    let height = if t.land > 0 && t.elevation / crate::cast!(t.land => f32) >= mean_elevation { "Upper" } else { "Lower" };
    let mut out = Vec::with_capacity(6);
    if central {
        out.push(format!("Central {noun}"));
    }
    out.push(format!("{primary} {noun}"));
    out.push(format!("{secondary} {noun}"));
    out.push(format!("{height} {noun}"));
    out.push(format!("Far {primary} {noun}"));
    out.push(format!("{noun} {}", roman(index + 1)));
    out
}

const fn roman(n: usize) -> &'static str {
    match n {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        7 => "VII",
        _ => "VIII",
    }
}
