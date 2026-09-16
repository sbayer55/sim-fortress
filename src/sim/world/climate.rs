//! Climate: the prevailing wind, orographic rainfall and temperature.
//!
//! Rain is not a free noise field. Air parcels are marched across the grid
//! in wind order carrying a moisture budget: they pick up moisture over open
//! water, rain it out faster wherever the surface rises under them and slower
//! where it falls away, so the windward flanks of ridges are wet and their
//! lee sides lie in shadow. Temperature falls from the warm edge of the map
//! to the cold one and with altitude.

use serde::{Deserialize, Serialize};

use crate::sim::params::Rainfall;
use crate::sim::rng::Rng;

use super::flow::Grid;
use super::noise::Noise;

/// Moisture a parcel carries when it enters at the windward edge (the world
/// is a window on a larger land, so air arrives already damp).
const ENTRY: f32 = 0.6;
/// Uptake over open water per horizontal unit, toward saturation.
const PICKUP: f32 = 0.12;
/// Evapotranspiration returned from land per horizontal unit.
const RECHARGE: f32 = 0.006;
/// Share of the parcel that rains out per horizontal unit on level ground.
const BASE_RATE: f32 = 0.03;
/// Extra rain-out per unit of surface rise (height is 0..=1).
const UPLIFT: f32 = 2.5;
/// Rain-out suppressed per unit of surface fall (föhn drying).
const DESCENT: f32 = 1.5;
/// Rain-out per horizontal unit that saturates the field at 1.
const RAIN_REF: f32 = 0.024;
/// Weight of the orographic sweep against the seeded noise in the rain field.
const OROGRAPHIC_WEIGHT: f32 = 0.7;
/// Temperature drop from sea level to the highest cell.
const LAPSE: f32 = 0.45;
/// Seeded wobble on the isotherms so they are not straight lines.
const WOBBLE: f32 = 0.12;

/// The prevailing wind, named for the quarter it blows from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wind {
    Westerly,
    Easterly,
    Northerly,
    Southerly,
}

impl Wind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Westerly => "westerly",
            Self::Easterly => "easterly",
            Self::Northerly => "northerly",
            Self::Southerly => "southerly",
        }
    }

    /// Draw the prevailing wind: westerlies dominate, as on a mid-latitude world.
    pub(super) fn pick(rng: &mut Rng) -> Self {
        let r = rng.f32();
        if r < 0.5 {
            Self::Westerly
        } else if r < 0.7 {
            Self::Easterly
        } else if r < 0.85 {
            Self::Northerly
        } else {
            Self::Southerly
        }
    }

    /// Lanes the wind sweeps along, cells per lane, and the horizontal
    /// length of one step (rows are two units tall).
    const fn lanes(self, grid: Grid) -> (usize, usize, f32) {
        match self {
            Self::Westerly | Self::Easterly => (grid.h, grid.w, 1.0),
            Self::Northerly | Self::Southerly => (grid.w, grid.h, 2.0),
        }
    }

    /// Cell index of step `k` along lane `lane`, walking downwind.
    const fn index(self, grid: Grid, lane: usize, k: usize) -> usize {
        match self {
            Self::Westerly => lane * grid.w + k,
            Self::Easterly => lane * grid.w + (grid.w - 1 - k),
            Self::Northerly => k * grid.w + lane,
            Self::Southerly => (grid.h - 1 - k) * grid.w + lane,
        }
    }
}

/// Moisture budget multiplier for the climate setting.
pub(super) const fn budget(rainfall: Rainfall) -> f32 {
    match rainfall {
        Rainfall::Dry => 0.7,
        Rainfall::Normal => 1.0,
        Rainfall::Wet => 1.3,
    }
}

/// The long-run rain field, 0.5..=1.5: the orographic sweep over `height`
/// blended with the seeded `noise` (0..=1) so plains are not uniform.
pub(super) fn rain_field(grid: Grid, height: &[f32], water: &[bool], wind: Wind, budget: f32, noise: &[f32]) -> Vec<f32> {
    let sweep = orographic(grid, height, water, wind, budget);
    sweep.iter().zip(noise).map(|(o, n)| 0.5 + OROGRAPHIC_WEIGHT * o + (1.0 - OROGRAPHIC_WEIGHT) * n).collect()
}

/// March moisture parcels downwind along every lane; returns rain-out per
/// horizontal unit, scaled so `RAIN_REF` is 1 and capped there.
fn orographic(grid: Grid, height: &[f32], water: &[bool], wind: Wind, budget: f32) -> Vec<f32> {
    let n = grid.len();
    let (lanes, len, step) = wind.lanes(grid);
    let mut rain = vec![0.0f32; n];
    for lane in 0..lanes {
        let mut m = ENTRY * budget;
        let mut prev = height[wind.index(grid, lane, 0)];
        for k in 0..len {
            let i = wind.index(grid, lane, k);
            let h = height[i];
            if water[i] {
                m += PICKUP * step * (budget - m).max(0.0);
                let p = m * BASE_RATE * step;
                rain[i] = p;
                m -= p;
            } else {
                let dh = h - prev;
                let frac = (BASE_RATE * step + UPLIFT * dh.max(0.0) - DESCENT * (-dh).max(0.0)).clamp(BASE_RATE * step * 0.2, 0.9);
                let p = m * frac;
                rain[i] = p;
                m = m - p + RECHARGE * step * budget;
            }
            prev = h;
        }
    }
    // Blur across lanes so a narrow ridge does not leave a one-lane stripe.
    let src = rain.clone();
    for lane in 0..lanes {
        for k in 0..len {
            let i = wind.index(grid, lane, k);
            let a = if lane > 0 { src[wind.index(grid, lane - 1, k)] } else { src[i] };
            let b = if lane + 1 < lanes { src[wind.index(grid, lane + 1, k)] } else { src[i] };
            rain[i] = ((0.25 * a + 0.5 * src[i] + 0.25 * b) / step / RAIN_REF).min(1.0);
        }
    }
    rain
}

/// Temperature 0 (cold) ..= 1 (hot): a latitude gradient from the cold edge
/// to the warm one, a lapse rate with height, and a seeded wobble.
pub(super) fn temperature(rng: &mut Rng, grid: Grid, height: &[f32], pole_north: bool) -> Vec<f32> {
    let (sw, sh) = (crate::cast!(grid.w => f32), crate::cast!(grid.h => f32) * 2.0);
    let wobble = Noise::new(rng, (sw.max(sh) / 6.0).max(12.0), sw, sh);
    let rows = crate::cast!(grid.h.max(2) - 1 => f32);
    (0..grid.len())
        .map(|i| {
            let (x, y) = (i % grid.w, i.div_euclid(grid.w));
            let lat = crate::cast!(y => f32) / rows;
            let warmth = if pole_north { lat } else { 1.0 - lat };
            let w = wobble.at(crate::cast!(x => f32), crate::cast!(y => f32) * 2.0) - 0.5;
            (0.2 + 0.7 * warmth - LAPSE * height[i] + WOBBLE * w).clamp(0.0, 1.0)
        })
        .collect()
}
