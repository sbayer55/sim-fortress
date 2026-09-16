//! Relief: a seeded tectonic height field aged by a landscape-evolution
//! model. Each epoch of `age` routes drainage over the current surface,
//! incises channels with the stream-power law (implicit in elevation, so any
//! epoch length is stable) and relaxes hillslopes by diffusion. A fresh
//! seeded storm field modulates rainfall every epoch, so where the valleys
//! cut is a mix of the seed's noise and its accumulated history.

use crate::sim::params::WorldParams;
use crate::sim::rng::Rng;

use super::flow::{self, lowest, Flow, Grid};
use super::noise::{Fbm, Noise};

/// Stream-power coefficient per epoch, for a drainage area of one reference
/// cell at unit distance.
const INCISION_K: f32 = 0.09;
/// Reference cell count the drainage area is normalised to, so a bigger map
/// erodes at the same rate per cell rather than dramatically faster.
const REFERENCE_CELLS: f32 = 6000.0;
/// Hillslope diffusion per epoch (explicit; stable below 0.4 on this grid).
const DIFFUSION: f32 = 0.12;
/// Fine seeded microrelief laid down before every epoch's routing (slumps,
/// fans, fallen timber): the raw and diffused surfaces are otherwise so
/// smooth that channels lock into dead-straight rows and columns.
const ROUGHNESS: f32 = 0.015;
const ROUGHNESS_SCALE: f32 = 2.5;
/// Rise per cell along a filled lake surface so it still drains.
const FILL_EPS: f32 = 1.0e-5;
/// Share of the water target that is ocean base level during erosion.
const BASE_LEVEL_SHARE: f32 = 0.6;

/// The finished relief, normalised to 0..=1.
#[derive(Debug)]
pub(super) struct Relief {
    pub(super) height: Vec<f32>,
    /// Depth of standing water on the filled surface (0 outside depressions).
    pub(super) depth: Vec<f32>,
    /// Drainage area in cells on the filled surface.
    pub(super) acc: Vec<f32>,
    pub(super) slope: Vec<f32>,
    /// Long-run rainfall weight, 0.5..=1.5, from the seeded rain field.
    pub(super) rain: Vec<f32>,
}

/// Build the relief for `params` from the generation `rng`.
pub(super) fn build(rng: &mut Rng, grid: Grid, params: &WorldParams) -> Relief {
    let (mut height, rain) = tectonics(rng, grid);
    normalise(&mut height);
    let mut fixed = vec![false; grid.len()];
    mark_base_level(&height, params.water_pct, &mut fixed);
    for _ in 0..params.age {
        roughen(rng, grid, &mut height, &fixed);
        epoch(rng, grid, &mut height, &fixed, &rain);
    }
    roughen(rng, grid, &mut height, &fixed);
    normalise(&mut height);
    // The finished surface drains to the ocean and off the map's edges: the
    // world is a window on a larger land, so rivers may leave it.
    let mut outlet: Vec<bool> = (0..grid.len()).map(|i| grid.is_edge(i)).collect();
    mark_base_level(&height, params.water_pct, &mut outlet);
    let filled = flow::fill_depressions(grid, &height, &outlet, FILL_EPS);
    let routed = flow::route(grid, &filled, &outlet);
    let acc = flow::accumulate(&routed, &vec![1.0f32; grid.len()]);
    let depth = filled.iter().zip(&height).map(|(f, h)| f - h).collect();
    let slope = flow::slopes(grid, &height);
    Relief { height, depth, acc, slope, rain }
}

/// The unaged surface: warped continental noise with ridged mountain chains
/// where the continent is high, plus the long-run rain field.
fn tectonics(rng: &mut Rng, grid: Grid) -> (Vec<f32>, Vec<f32>) {
    let (sw, sh) = (crate::cast!(grid.w => f32), crate::cast!(grid.h => f32) * 2.0);
    let base = (sw.max(sh) / 5.0).max(22.0);
    let continent = Fbm::new(rng, base, 5, sw, sh);
    let ridges = Fbm::new(rng, base * 0.6, 4, sw, sh);
    // Low-frequency domain warp: bends coastlines and ridges into organic shapes.
    let warp_x = Noise::new(rng, base * 1.2, sw, sh);
    let warp_y = Noise::new(rng, base * 1.2, sw, sh);
    let warp_amp = base * 0.6;
    let rain_field = Fbm::new(rng, base * 0.9, 3, sw, sh);

    let n = grid.len();
    let mut height = Vec::with_capacity(n);
    let mut rain = Vec::with_capacity(n);
    for i in 0..n {
        let (nx, ny) = (crate::cast!(i % grid.w => f32), crate::cast!(i.div_euclid(grid.w) => f32) * 2.0);
        let wx = nx + (warp_x.at(nx, ny) - 0.5) * warp_amp;
        let wy = ny + (warp_y.at(nx, ny) - 0.5) * warp_amp;
        let c = continent.at(wx, wy);
        let r = ridges.ridged_at(wx, wy);
        // Ridges only bite into the uplands; lowlands stay gently rolling.
        let upland = ((c - 0.4) / 0.35).clamp(0.0, 1.0);
        height.push(c * 0.7 + r * upland * 0.5);
        rain.push(0.5 + rain_field.at(nx, ny));
    }
    (height, rain)
}

/// One epoch of geological time: seeded storms, drainage, incision, diffusion.
fn epoch(rng: &mut Rng, grid: Grid, height: &mut [f32], fixed: &[bool], rain: &[f32]) {
    let (sw, sh) = (crate::cast!(grid.w => f32), crate::cast!(grid.h => f32) * 2.0);
    let storms = Noise::new(rng, (sw.max(sh) / 6.0).max(12.0), sw, sh);
    let mut weights = Vec::with_capacity(grid.len());
    for i in 0..grid.len() {
        let (nx, ny) = (crate::cast!(i % grid.w => f32), crate::cast!(i.div_euclid(grid.w) => f32) * 2.0);
        weights.push(rain[i] * (0.5 + storms.at(nx, ny)));
    }
    let routed = flow::route(grid, height, fixed);
    let acc = flow::accumulate(&routed, &weights);
    let k = INCISION_K * (REFERENCE_CELLS / crate::cast!(grid.len() => f32)).sqrt();
    incise(height, &routed, &acc, k);
    diffuse(grid, height, fixed);
}

/// Lay a fresh seeded microrelief over the land.
fn roughen(rng: &mut Rng, grid: Grid, height: &mut [f32], fixed: &[bool]) {
    let (sw, sh) = (crate::cast!(grid.w => f32), crate::cast!(grid.h => f32) * 2.0);
    let rough = Noise::new(rng, ROUGHNESS_SCALE, sw, sh);
    for i in 0..grid.len() {
        if !fixed[i] {
            let (nx, ny) = (crate::cast!(i % grid.w => f32), crate::cast!(i.div_euclid(grid.w) => f32) * 2.0);
            height[i] += (rough.at(nx, ny) - 0.5) * ROUGHNESS;
        }
    }
}

/// Implicit stream-power incision (n = 1, m = ½): each cell settles between
/// its old height and its already-updated receiver, weighted by discharge.
fn incise(height: &mut [f32], routed: &Flow, acc: &[f32], k: f32) {
    for &i in &routed.order {
        let r = routed.recv[i];
        if r == i {
            continue;
        }
        let f = k * acc[i].sqrt() / routed.dist[i];
        height[i] = (height[i] + f * height[r]) / (1.0 + f);
    }
}

/// Explicit hillslope diffusion with no-flux edges; base-level cells hold.
fn diffuse(grid: Grid, height: &mut [f32], fixed: &[bool]) {
    let (w, h) = (grid.w, grid.h);
    let src = height.to_vec();
    let at = |x: usize, y: usize| src[y * w + x];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if fixed[i] {
                continue;
            }
            let hx = at(x.saturating_sub(1), y) + at((x + 1).min(w - 1), y) - 2.0 * src[i];
            let hy = at(x, y.saturating_sub(1)) + at(x, (y + 1).min(h - 1)) - 2.0 * src[i];
            height[i] = src[i] + DIFFUSION * (hx + hy * 0.25);
        }
    }
}

/// Rescale to exactly 0..=1 (a flat field becomes all zeros).
fn normalise(height: &mut [f32]) {
    let lo = height.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = height.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let span = hi - lo;
    for v in height.iter_mut() {
        *v = if span > 0.0 { (*v - lo) / span } else { 0.0 };
    }
}

/// Flag the lowest `BASE_LEVEL_SHARE` of the water target as base level; a
/// world with no water target uses its single lowest cell as the outlet.
fn mark_base_level(height: &[f32], water_pct: u8, out: &mut [bool]) {
    let n = height.len();
    let count = (crate::cast!(water_pct => usize) * n).div_euclid(100);
    let count = crate::cast!((crate::cast!(count => f32) * BASE_LEVEL_SHARE).floor() => usize).min(n);
    for i in lowest(0..n, height, count.max(1)) {
        out[i] = true;
    }
}
