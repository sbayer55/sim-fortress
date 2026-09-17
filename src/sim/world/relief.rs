//! Relief: a seeded tectonic height field aged by a landscape-evolution
//! model. Each epoch of `age` routes drainage over the current surface,
//! incises channels with the stream-power law (implicit in elevation, so any
//! epoch length is stable) and relaxes hillslopes by diffusion. Long-run rain
//! comes from the orographic sweep in `climate`, and a fresh seeded storm
//! field modulates it every epoch, so where the valleys cut is a mix of the
//! wind, the seed's noise and its accumulated history. The `AgeRegime` sets
//! how hard each epoch cuts and smooths, so a young world keeps sharp ridges
//! and an old one wears down to broad valleys; the `history` events strike
//! between epochs and are eroded by the ones that follow.

use crate::sim::params::WorldParams;
use crate::sim::rng::Rng;

use super::climate::{self, Wind};
use super::flow::{self, lowest, Flow, Grid};
use super::history::{Draft, HistoryEvent};
use super::noise::{Fbm, Noise};

/// Reference cell count the drainage area is normalised to, so a bigger map
/// erodes at the same rate per cell rather than dramatically faster.
const REFERENCE_CELLS: f32 = 6000.0;
/// Fine seeded microrelief laid down before every epoch's routing (slumps,
/// fans, fallen timber): the raw and diffused surfaces are otherwise so
/// smooth that channels lock into dead-straight rows and columns.
const ROUGHNESS: f32 = 0.015;
const ROUGHNESS_SCALE: f32 = 2.5;
/// Rise per cell along a filled lake surface so it still drains.
const FILL_EPS: f32 = 1.0e-5;
/// Share of the water target that is ocean base level during erosion.
const BASE_LEVEL_SHARE: f32 = 0.6;
/// Depressions at least this deep feed moisture to the wind (matches the
/// lake threshold in `classify`).
const LAKE_SOURCE_DEPTH: f32 = 0.004;
/// Odds that the cold edge of the world is the north.
const POLE_NORTH_CHANCE: f32 = 0.7;

/// How one epoch of geological time treats the land.
///
/// The stream-power coefficient (per epoch, for a drainage area of one
/// reference cell at unit distance), the explicit hillslope diffusion
/// (stable below 0.4 on this grid), and how much sediment fills the
/// depressions every `fill_every` epochs (0 never fills).
#[derive(Debug)]
pub struct AgeRegime {
    pub name: &'static str,
    incision_k: f32,
    diffusion: f32,
    fill_share: f32,
    fill_every: u8,
}

/// Young worlds cut fast and diffuse little, so ridges stay sharp; old
/// worlds cut slowly, diffuse hard and silt their hollows into peneplains.
const YOUNG: AgeRegime = AgeRegime { name: "young", incision_k: 0.11, diffusion: 0.06, fill_share: 0.0, fill_every: 0 };
const MATURE: AgeRegime = AgeRegime { name: "mature", incision_k: 0.09, diffusion: 0.12, fill_share: 0.0, fill_every: 0 };
const OLD: AgeRegime = AgeRegime { name: "old", incision_k: 0.06, diffusion: 0.2, fill_share: 0.5, fill_every: 4 };
/// Ages at or below this are young; at or above `OLD_AGE` are old.
const YOUNG_AGE: u8 = 3;
const OLD_AGE: u8 = 12;

impl AgeRegime {
    pub const fn for_age(age: u8) -> &'static Self {
        if age <= YOUNG_AGE {
            &YOUNG
        } else if age >= OLD_AGE {
            &OLD
        } else {
            &MATURE
        }
    }
}

/// The finished relief, normalised to 0..=1.
#[derive(Debug)]
pub(super) struct Relief {
    pub(super) height: Vec<f32>,
    /// Depth of standing water on the filled surface (0 outside depressions).
    pub(super) depth: Vec<f32>,
    /// Drainage area in cells on the filled surface.
    pub(super) acc: Vec<f32>,
    pub(super) slope: Vec<f32>,
    /// Long-run rainfall weight, 0.5..=1.5, from the orographic sweep over
    /// the finished surface.
    pub(super) rain: Vec<f32>,
    /// Temperature 0 (cold) ..= 1 (hot).
    pub(super) temperature: Vec<f32>,
    pub(super) wind: Wind,
    /// Steepest-descent receiver of each cell on the finished surface (a
    /// root is its own receiver): the direction a river flows.
    pub(super) recv: Vec<usize>,
    /// Root of each cell's drainage tree on the finished surface (an ocean
    /// cell, a map edge or a pit): cells sharing a root share a basin.
    pub(super) basin: Vec<usize>,
    /// Ocean base level on the finished surface.
    pub(super) sea: Vec<bool>,
    /// Cells an event left as bare rock (a dome's core, ice-scoured tops).
    pub(super) bedrock: Vec<bool>,
    /// The events that struck during the epochs, in epoch order.
    pub(super) history: Vec<HistoryEvent>,
}

/// Build the relief for `params` from the generation `rng`.
pub(super) fn build(rng: &mut Rng, grid: Grid, params: &WorldParams) -> Relief {
    let wind = Wind::pick(rng);
    let pole_north = rng.chance(POLE_NORTH_CHANCE);
    let budget = climate::budget(params.rainfall);
    let (mut height, rain_noise) = tectonics(rng, grid);
    normalise(&mut height);
    let mut fixed = vec![false; grid.len()];
    mark_base_level(&height, params.water_pct, &mut fixed);
    // The wind sweeps the raw uplands, so erosion cuts hardest on the
    // windward flanks and the lee stays dry and gentle.
    let rain = climate::rain_field(grid, &height, &fixed, wind, budget, &rain_noise);
    let regime = AgeRegime::for_age(params.age);
    let mut draft = Draft::draw(rng, grid, params.age, &fixed, pole_north);
    let mut bedrock = vec![false; grid.len()];
    for e in 0..params.age {
        draft.strike(e, grid, &mut height, &fixed, &mut bedrock);
        roughen(rng, grid, &mut height, &fixed);
        epoch(rng, grid, &mut height, &fixed, &rain, regime);
        if regime.fill_every > 0 && (e + 1) % regime.fill_every == 0 {
            silt(grid, &mut height, &fixed, regime.fill_share);
        }
    }
    draft.strike(params.age, grid, &mut height, &fixed, &mut bedrock);
    roughen(rng, grid, &mut height, &fixed);
    normalise(&mut height);
    // The finished surface drains to the ocean and off the map's edges: the
    // world is a window on a larger land, so rivers may leave it.
    let mut outlet: Vec<bool> = (0..grid.len()).map(|i| grid.is_edge(i)).collect();
    mark_base_level(&height, params.water_pct, &mut outlet);
    let filled = flow::fill_depressions(grid, &height, &outlet, FILL_EPS);
    let routed = flow::route(grid, &filled, &outlet);
    let acc = flow::accumulate(&routed, &vec![1.0f32; grid.len()]);
    let basin = basins(&routed);
    let depth: Vec<f32> = filled.iter().zip(&height).map(|(f, h)| f - h).collect();
    let slope = flow::slopes(grid, &height);
    // Re-sweep the finished surface: the ocean and the lakes it now holds
    // are the moisture sources the living world sees.
    let mut sea = vec![false; grid.len()];
    mark_base_level(&height, params.water_pct, &mut sea);
    let mut source = sea.clone();
    for (s, &d) in source.iter_mut().zip(&depth) {
        *s |= d >= LAKE_SOURCE_DEPTH;
    }
    let rain = climate::rain_field(grid, &height, &source, wind, budget, &rain_noise);
    let temperature = climate::temperature(rng, grid, &height, pole_north);
    Relief { height, depth, acc, slope, rain, temperature, wind, recv: routed.recv, basin, sea, bedrock, history: draft.events }
}

/// The root each cell drains to. `order` lists every root before the cells
/// that drain into it, so one downstream-first walk labels the whole tree.
fn basins(routed: &Flow) -> Vec<usize> {
    let mut basin = vec![0usize; routed.recv.len()];
    for &i in &routed.order {
        let r = routed.recv[i];
        basin[i] = if r == i { i } else { basin[r] };
    }
    basin
}

/// The unaged surface: warped continental noise with ridged mountain chains
/// where the continent is high, plus the seeded rain noise (0..=1) that the
/// orographic sweep is blended with.
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
        rain.push(rain_field.at(nx, ny));
    }
    (height, rain)
}

/// One epoch of geological time: seeded storms, drainage, incision, diffusion.
fn epoch(rng: &mut Rng, grid: Grid, height: &mut [f32], fixed: &[bool], rain: &[f32], regime: &AgeRegime) {
    let (sw, sh) = (crate::cast!(grid.w => f32), crate::cast!(grid.h => f32) * 2.0);
    let storms = Noise::new(rng, (sw.max(sh) / 6.0).max(12.0), sw, sh);
    let mut weights = Vec::with_capacity(grid.len());
    for i in 0..grid.len() {
        let (nx, ny) = (crate::cast!(i % grid.w => f32), crate::cast!(i.div_euclid(grid.w) => f32) * 2.0);
        weights.push(rain[i] * (0.5 + storms.at(nx, ny)));
    }
    let routed = flow::route(grid, height, fixed);
    let acc = flow::accumulate(&routed, &weights);
    let k = regime.incision_k * (REFERENCE_CELLS / crate::cast!(grid.len() => f32)).sqrt();
    incise(height, &routed, &acc, k);
    diffuse(grid, height, fixed, regime.diffusion);
}

/// Sediment settles in the hollows: every cell rises `share` of the way to
/// the depression-filled surface, so an old world's basins become plains.
fn silt(grid: Grid, height: &mut [f32], fixed: &[bool], share: f32) {
    let filled = flow::fill_depressions(grid, height, fixed, FILL_EPS);
    for (h, f) in height.iter_mut().zip(&filled) {
        *h += share * (f - *h);
    }
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
fn diffuse(grid: Grid, height: &mut [f32], fixed: &[bool], diffusion: f32) {
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
            height[i] = src[i] + diffusion * (hx + hy * 0.25);
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
