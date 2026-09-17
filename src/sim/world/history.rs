//! Geological history: one to three seeded events that strike the relief
//! during its erosion epochs and are remembered on the finished world. A
//! fault scarp throws one side of a warped line up, a volcanic dome raises a
//! cone whose core stays bare rock, and a glaciation scours the cold half's
//! uplands into broad valleys dammed by moraine. Everything an event needs is
//! drawn from the generation `Rng` before the first epoch, so applying one is
//! pure and the epochs that follow erode it deterministically.

use serde::{Deserialize, Serialize};

use crate::sim::rng::Rng;

use super::flow::{self, Grid};
use super::noise::Noise;

/// Fewest and most events a world remembers.
const EVENTS_MIN: usize = 1;
const EVENTS_MAX: usize = 3;
/// An event's centre keeps this many columns and rows from the map edge.
const MARGIN_X: usize = 6;
const MARGIN_Y: usize = 3;
/// Candidate lines drawn per fault; the one crossing the most land wins.
const SCARP_TRIES: usize = 6;
/// Fault scarp: throw across the line, half-width of the ramp, half-length
/// taper and lateral warp, all in horizontal cell units of a 0..=1 surface.
const SCARP_RISE: f32 = 0.12;
const SCARP_WIDTH: f32 = 1.5;
const SCARP_TAPER: f32 = 6.0;
const SCARP_WARP: f32 = 4.0;
const SCARP_WARP_SCALE: f32 = 9.0;
/// Volcanic dome: cone height, radius range (cells) and the share of the
/// radius that is bare rock however long it erodes.
const DOME_RISE: f32 = 0.15;
const DOME_RADIUS_MIN: usize = 3;
const DOME_RADIUS_MAX: usize = 6;
const DOME_CORE: f32 = 0.5;
/// Glaciation: the upland share of the cold half under ice, smoothing
/// passes and strength, the drainage a valley floor needs to be scoured, the
/// scour depth, the moraine height at the ice margin and the share of the
/// cold half left as bare rock.
const ICE_SHARE: f32 = 0.25;
const ICE_SMOOTH_PASSES: usize = 4;
const ICE_SMOOTH: f32 = 0.2;
const SCOUR_MIN_ACC: f32 = 8.0;
const SCOUR: f32 = 0.02;
const MORAINE: f32 = 0.012;
const ICE_BARE_SHARE: f32 = 0.02;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryKind {
    FaultScarp,
    VolcanicDome,
    Glaciation,
}

impl HistoryKind {
    const ALL: [Self; 3] = [Self::FaultScarp, Self::VolcanicDome, Self::Glaciation];

    pub const fn name(self) -> &'static str {
        match self {
            Self::FaultScarp => "fault scarp",
            Self::VolcanicDome => "volcanic dome",
            Self::Glaciation => "glaciation",
        }
    }
}

/// One remembered event.
///
/// `(x, y)` is the dome's centre, the scarp's midpoint or the ice's
/// centroid; `extent` the dome radius, the scarp's half-length or the count
/// of cells the ice covered; `angle_deg` the scarp's strike, clockwise from
/// east.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub kind: HistoryKind,
    /// The epoch it struck, `0..=age`; `age` means after the last epoch.
    pub epoch: u8,
    pub x: usize,
    pub y: usize,
    pub extent: usize,
    pub angle_deg: i16,
}

impl HistoryEvent {
    /// A plain sentence for the generation log and diagnostics.
    pub fn describe(&self, age: u8) -> String {
        let when = if age == 0 {
            "in the unweathered world".to_string()
        } else if self.epoch >= age {
            "after the last epoch".to_string()
        } else {
            format!("in epoch {} of {age}", self.epoch + 1)
        };
        match self.kind {
            HistoryKind::FaultScarp => format!("A fault throws up a scarp {} cells long at ({}, {}) {when}", self.extent * 2, self.x, self.y),
            HistoryKind::VolcanicDome => format!("A volcano domes the land {} cells around ({}, {}) {when}", self.extent, self.x, self.y),
            HistoryKind::Glaciation => format!("Ice scours {} cells of the cold uplands about ({}, {}) {when}", self.extent, self.x, self.y),
        }
    }
}

/// The events drawn for a world, ready to strike in epoch order.
pub(super) struct Draft {
    pub(super) events: Vec<HistoryEvent>,
    warp: Noise,
    pole_north: bool,
}

impl Draft {
    /// Draw the events for a world of `age` epochs. `fixed` cells (base
    /// level) never host an event's centre.
    pub(super) fn draw(rng: &mut Rng, grid: Grid, age: u8, fixed: &[bool], pole_north: bool) -> Self {
        let (sw, sh) = (crate::cast!(grid.w => f32), crate::cast!(grid.h => f32) * 2.0);
        let count = EVENTS_MIN + rng.below(EVENTS_MAX - EVENTS_MIN + 1);
        let inside = |i: usize| {
            let (x, y) = (i % grid.w, i.div_euclid(grid.w));
            (MARGIN_X..grid.w.saturating_sub(MARGIN_X)).contains(&x) && (MARGIN_Y..grid.h.saturating_sub(MARGIN_Y)).contains(&y)
        };
        let mut land: Vec<usize> = (0..grid.len()).filter(|&i| !fixed[i] && inside(i)).collect();
        if land.is_empty() {
            land = (0..grid.len()).filter(|&i| !fixed[i]).collect();
        }
        let mut events = Vec::with_capacity(count);
        let mut iced = false;
        for _ in 0..count {
            let mut kind = *rng.pick(&HistoryKind::ALL);
            if kind == HistoryKind::Glaciation && iced {
                kind = HistoryKind::FaultScarp;
            }
            iced |= kind == HistoryKind::Glaciation;
            let epoch = crate::cast!(rng.below(usize::from(age) + 1) => u8);
            let mut e = HistoryEvent { kind, epoch, x: 0, y: 0, extent: 0, angle_deg: 0 };
            match kind {
                HistoryKind::FaultScarp => place_scarp(rng, grid, fixed, &land, &mut e),
                HistoryKind::VolcanicDome => {
                    let centre = land.get(rng.below(land.len())).copied().unwrap_or(0);
                    (e.x, e.y) = (centre % grid.w, centre.div_euclid(grid.w));
                    e.extent = DOME_RADIUS_MIN + rng.below(DOME_RADIUS_MAX - DOME_RADIUS_MIN + 1);
                }
                HistoryKind::Glaciation => {}
            }
            events.push(e);
        }
        events.sort_by_key(|e| e.epoch);
        let warp = Noise::new(rng, SCARP_WARP_SCALE, sw, sh);
        Self { events, warp, pole_north }
    }

    /// Strike every event drawn for `epoch`, in order.
    pub(super) fn strike(&mut self, epoch: u8, grid: Grid, height: &mut [f32], fixed: &[bool], bedrock: &mut [bool]) {
        for e in self.events.iter_mut().filter(|e| e.epoch == epoch) {
            match e.kind {
                HistoryKind::FaultScarp => scarp(e, &self.warp, grid, height, fixed),
                HistoryKind::VolcanicDome => dome(e, grid, height, fixed, bedrock),
                HistoryKind::Glaciation => glaciate(e, self.pole_north, grid, height, fixed, bedrock),
            }
        }
    }
}

/// Draw a fault's half-length, then `SCARP_TRIES` centres and strikes,
/// keeping the line that crosses the most land.
fn place_scarp(rng: &mut Rng, grid: Grid, fixed: &[bool], land: &[usize], e: &mut HistoryEvent) {
    e.extent = grid.w.div_euclid(6) + rng.below(grid.w.div_euclid(6).max(1));
    let mut best = 0;
    for _ in 0..SCARP_TRIES {
        let centre = land.get(rng.below(land.len())).copied().unwrap_or(0);
        let angle_deg = crate::cast!(rng.below(180) => i16);
        let crossed = land_along(grid, fixed, centre, angle_deg, e.extent);
        if crossed >= best {
            best = crossed;
            (e.x, e.y, e.angle_deg) = (centre % grid.w, centre.div_euclid(grid.w), angle_deg);
        }
    }
}

/// How many unfixed, in-map cells a line of `half` units either way of
/// `centre` at `angle_deg` passes through.
fn land_along(grid: Grid, fixed: &[bool], centre: usize, angle_deg: i16, half: usize) -> usize {
    let (cx, cy) = at(grid, centre);
    let theta = f32::from(angle_deg).to_radians();
    let (dx, dy) = (theta.cos(), theta.sin());
    let half = crate::cast!(half => i32);
    (-half..=half)
        .filter(|&t| {
            let t = crate::cast!(t => f32);
            let (x, y) = ((cx + t * dx).round(), ((cy + t * dy) * 0.5).round());
            x >= 0.0 && y >= 0.0 && x < crate::cast!(grid.w => f32) && y < crate::cast!(grid.h => f32) && !fixed[crate::cast!(y => usize) * grid.w + crate::cast!(x => usize)]
        })
        .count()
}

/// Sample-space coordinates of cell `i`: rows count double.
fn at(grid: Grid, i: usize) -> (f32, f32) {
    (crate::cast!(i % grid.w => f32), crate::cast!(i.div_euclid(grid.w) => f32) * 2.0)
}

/// Throw the side of a warped line up through a tanh ramp, tapering past
/// the half-length.
fn scarp(e: &HistoryEvent, warp: &Noise, grid: Grid, height: &mut [f32], fixed: &[bool]) {
    let (cx, cy) = at(grid, e.y * grid.w + e.x);
    let theta = f32::from(e.angle_deg).to_radians();
    let (dx, dy) = (theta.cos(), theta.sin());
    let half = crate::cast!(e.extent => f32);
    for i in 0..grid.len() {
        if fixed[i] {
            continue;
        }
        let (x, y) = at(grid, i);
        let (rx, ry) = (x - cx, y - cy);
        let along = rx * dx + ry * dy;
        let across = -rx * dy + ry * dx + (warp.at(x, y) - 0.5) * SCARP_WARP;
        let taper = (1.0 - (along.abs() - half) / SCARP_TAPER).clamp(0.0, 1.0);
        height[i] += SCARP_RISE * taper * 0.5 * (1.0 + (across / SCARP_WIDTH).tanh());
    }
}

/// Raise a cone; its core is bedrock.
fn dome(e: &HistoryEvent, grid: Grid, height: &mut [f32], fixed: &[bool], bedrock: &mut [bool]) {
    let (cx, cy) = at(grid, e.y * grid.w + e.x);
    let radius = crate::cast!(e.extent => f32);
    for i in 0..grid.len() {
        if fixed[i] {
            continue;
        }
        let (x, y) = at(grid, i);
        let d = (x - cx).hypot(y - cy);
        if d < radius {
            height[i] += DOME_RISE * (1.0 - d / radius);
            bedrock[i] |= d <= radius * DOME_CORE;
        }
    }
}

/// Ice over the cold half's uplands: smooth them into broad valleys, scour
/// the valley floors, bank moraine where the ice ended and bare the
/// highest ground. Records the ice's centroid and extent on the event.
fn glaciate(e: &mut HistoryEvent, pole_north: bool, grid: Grid, height: &mut [f32], fixed: &[bool], bedrock: &mut [bool]) {
    let n = grid.len();
    let cold = |i: usize| (i.div_euclid(grid.w) < grid.h.div_euclid(2)) == pole_north;
    let cold_land = (0..n).filter(|&i| cold(i) && !fixed[i]);
    let cold_count = cold_land.clone().count();
    let mut ice = vec![false; n];
    let iced = flow::highest(cold_land.clone(), height, crate::cast!((crate::cast!(cold_count => f32) * ICE_SHARE).round() => usize));
    for &i in &iced {
        ice[i] = true;
    }
    for &i in &flow::highest(cold_land, height, crate::cast!((crate::cast!(cold_count => f32) * ICE_BARE_SHARE).round() => usize)) {
        bedrock[i] = true;
    }
    let routed = flow::route(grid, height, fixed);
    let acc = flow::accumulate(&routed, &vec![1.0f32; n]);
    for _ in 0..ICE_SMOOTH_PASSES {
        smooth(grid, height, &ice);
    }
    let (mut sx, mut sy) = (0usize, 0usize);
    for &i in &iced {
        sx += i % grid.w;
        sy += i.div_euclid(grid.w);
        let r = routed.recv[i];
        if r != i && !ice[r] {
            height[i] += MORAINE;
        } else if acc[i] >= SCOUR_MIN_ACC {
            height[i] -= SCOUR;
        }
    }
    if !iced.is_empty() {
        e.x = sx.div_euclid(iced.len());
        e.y = sy.div_euclid(iced.len());
    }
    e.extent = iced.len();
}

/// One explicit smoothing pass over the masked cells (no-flux edges).
fn smooth(grid: Grid, height: &mut [f32], mask: &[bool]) {
    let (w, h) = (grid.w, grid.h);
    let src = height.to_vec();
    for i in 0..grid.len() {
        if !mask[i] {
            continue;
        }
        let (x, y) = (i % w, i.div_euclid(w));
        let hx = src[y * w + x.saturating_sub(1)] + src[y * w + (x + 1).min(w - 1)] - 2.0 * src[i];
        let hy = src[y.saturating_sub(1) * w + x] + src[(y + 1).min(h - 1) * w + x] - 2.0 * src[i];
        height[i] = src[i] + ICE_SMOOTH * (hx + hy * 0.25);
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    const GRID: Grid = Grid { w: 60, h: 30 };

    /// A gently sloped surface with a seeded ripple, open everywhere.
    fn surface(seed: u64) -> (Vec<f32>, Vec<bool>) {
        let mut rng = Rng::new(seed);
        let ripple = Noise::new(&mut rng, 4.0, 60.0, 60.0);
        let height = (0..GRID.len()).map(|i| {
            let (x, y) = at(GRID, i);
            0.3 + x / 200.0 + (ripple.at(x, y) - 0.5) * 0.1
        }).collect();
        (height, vec![false; GRID.len()])
    }

    fn mean(v: impl Iterator<Item = f32>) -> f32 {
        let (sum, n) = v.fold((0.0f32, 0usize), |(s, n), x| (s + x, n + 1));
        sum / crate::cast!(n.max(1) => f32)
    }

    #[test]
    fn draw_is_bounded_and_epoch_ordered() {
        for seed in 1..=30u64 {
            let (_, fixed) = surface(seed);
            let draft = Draft::draw(&mut Rng::new(seed), GRID, 8, &fixed, true);
            assert!((EVENTS_MIN..=EVENTS_MAX).contains(&draft.events.len()));
            assert!(draft.events.windows(2).all(|p| p[0].epoch <= p[1].epoch));
            assert!(draft.events.iter().all(|e| e.epoch <= 8));
            assert!(draft.events.iter().filter(|e| e.kind == HistoryKind::Glaciation).count() <= 1);
            let none = Draft::draw(&mut Rng::new(seed), GRID, 0, &fixed, true);
            assert!(none.events.iter().all(|e| e.epoch == 0));
        }
    }

    #[test]
    fn dome_raises_a_cone_with_a_rock_core() {
        let (mut height, fixed) = surface(1);
        let before = height.clone();
        let mut bedrock = vec![false; GRID.len()];
        let e = HistoryEvent { kind: HistoryKind::VolcanicDome, epoch: 0, x: 30, y: 15, extent: 5, angle_deg: 0 };
        dome(&e, GRID, &mut height, &fixed, &mut bedrock);
        let (cx, cy) = at(GRID, 15 * 60 + 30);
        let inside = |i: usize| { let (x, y) = at(GRID, i); (x - cx).hypot(y - cy) < 5.0 };
        let rise = mean((0..GRID.len()).filter(|&i| inside(i)).map(|i| height[i] - before[i]));
        assert!(rise > 0.03, "rise {rise}");
        assert!((0..GRID.len()).filter(|&i| !inside(i)).all(|i| height[i] == before[i]));
        assert!(bedrock[15 * 60 + 30]);
        assert!(bedrock.iter().filter(|&&b| b).count() < 20);
    }

    #[test]
    fn scarp_steps_across_the_line_not_along_it() {
        let (mut height, fixed) = surface(2);
        let before = height.clone();
        let mut rng = Rng::new(9);
        let warp = Noise::new(&mut rng, SCARP_WARP_SCALE, 60.0, 60.0);
        let e = HistoryEvent { kind: HistoryKind::FaultScarp, epoch: 0, x: 30, y: 15, extent: 12, angle_deg: 0 };
        scarp(&e, &warp, GRID, &mut height, &fixed);
        // Strike east: the south side (rows below 15) is thrown up.
        let south = mean((20..25).flat_map(|y| (25..35).map(move |x| y * 60 + x)).map(|i| height[i] - before[i]));
        let north = mean((5..10).flat_map(|y| (25..35).map(move |x| y * 60 + x)).map(|i| height[i] - before[i]));
        assert!(south > north + SCARP_RISE * 0.8, "south {south} north {north}");
        let far = mean((0..30).flat_map(|y| (0..5).map(move |x| y * 60 + x)).map(|i| (height[i] - before[i]).abs()));
        assert!(far < SCARP_RISE * 0.05, "far {far}");
    }

    #[test]
    fn glaciation_smooths_the_cold_uplands_and_dams_them() {
        let (mut height, fixed) = surface(3);
        let before = height.clone();
        let mut bedrock = vec![false; GRID.len()];
        let mut e = HistoryEvent { kind: HistoryKind::Glaciation, epoch: 0, x: 0, y: 0, extent: 0, angle_deg: 0 };
        glaciate(&mut e, true, GRID, &mut height, &fixed, &mut bedrock);
        assert!(e.extent > 0 && e.y < 15, "extent {} centroid ({}, {})", e.extent, e.x, e.y);
        let warm_untouched = (15 * 60..GRID.len()).all(|i| height[i] == before[i]);
        assert!(warm_untouched);
        let dammed = (0..GRID.len()).filter(|&i| height[i] - before[i] >= MORAINE * 0.5).count();
        let scoured = (0..GRID.len()).filter(|&i| before[i] - height[i] >= SCOUR * 0.5).count();
        assert!(dammed > 0 && scoured > 0, "dammed {dammed} scoured {scoured}");
        assert!(bedrock.iter().any(|&b| b) && bedrock.iter().filter(|&&b| b).count() <= 30);
    }

    #[test]
    fn smoothing_flattens_the_masked_ripple() {
        let (mut height, _) = surface(4);
        let before = height.clone();
        let mask: Vec<bool> = (0..GRID.len()).map(|i| i.div_euclid(60) < 15).collect();
        for _ in 0..ICE_SMOOTH_PASSES {
            smooth(GRID, &mut height, &mask);
        }
        let ripple = |h: &[f32]| mean((1..14).flat_map(|y| (1..59).map(move |x| y * 60 + x)).map(|i| (h[i] - h[i - 1]).abs()));
        assert!(ripple(&height) < ripple(&before) * 0.8, "after {} before {}", ripple(&height), ripple(&before));
        assert!((15 * 60..GRID.len()).all(|i| height[i] == before[i]));
    }
}
