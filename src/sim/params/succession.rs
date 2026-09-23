//! Succession and trampling tunables: how grazing reshapes the terrain.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::sim::world::Terrain;

/// The `[succession]` table (C2 FR12).
///
/// Trampling: sustained prey traffic lowers a cell's daily vegetation
/// target. Succession: a land cell that stays lush and lightly used climbs
/// the ladder Dirt → sparse grass → grassland → meadow → forest one rung at
/// a time; one grazed bare and trodden wears back down it. Sand, marsh, rock
/// and water are never on the ladder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SuccessionParams {
    /// Weight of prey pressure against the vegetation target: `target ×= 1 − this × pressure`.
    pub trample_w: f32,
    /// Vegetation at or above this share of today's target counts as thriving.
    pub climb_veg: f32,
    /// Vegetation below this share of the untrampled target counts as worn.
    pub wear_veg: f32,
    /// Prey pressure below this allows a thriving day.
    pub trample_low: f32,
    /// Prey pressure at or above this allows a worn day.
    pub trample_high: f32,
    /// How far both counters fall on a day that is neither thriving nor worn.
    pub relax_per_day: u16,
    /// Thriving days needed to climb *into* each rung above Dirt.
    pub climb_days: BTreeMap<Terrain, u32>,
    /// Cell moisture needed to climb *into* each rung above Dirt.
    pub climb_moisture: BTreeMap<Terrain, f32>,
    /// Worn days needed to drop one rung.
    pub wear_days_needed: u32,
    /// Daily chance a ripe cell flips (0 switches succession off).
    pub flip_chance: f32,
}

/// The rungs above Dirt, in climbing order.
const RUNGS_ABOVE_DIRT: [Terrain; 4] = [Terrain::GrassSparse, Terrain::Grass, Terrain::GrassDense, Terrain::Forest];

impl Default for SuccessionParams {
    fn default() -> Self {
        use Terrain::{Forest, Grass, GrassDense, GrassSparse};
        Self {
            // 0.5 in the plan; the six-seed sweep showed it starving the deer herds
            // (C2 FR12 result), and a quarter keeps them while the map still moves.
            trample_w: 0.25,
            climb_veg: 0.9,
            wear_veg: 0.5,
            trample_low: 0.10,
            trample_high: 0.30,
            relax_per_day: 1,
            climb_days: BTreeMap::from([(GrassSparse, 60), (Grass, 90), (GrassDense, 120), (Forest, 360)]),
            climb_moisture: BTreeMap::from([(GrassSparse, 0.15), (Grass, 0.25), (GrassDense, 0.35), (Forest, 0.40)]),
            wear_days_needed: 45,
            flip_chance: 0.10,
        }
    }
}

impl SuccessionParams {
    /// D4: the trampling factor on the vegetation target, `1 − trample_w × pressure`,
    /// clamped to 0..=1.
    pub fn trample_factor(&self, prey_pressure: f32) -> f32 {
        (1.0 - self.trample_w * prey_pressure).clamp(0.0, 1.0)
    }

    /// True when the overlay switches both mechanics off: the target is
    /// untouched and no cell ever rolls to flip.
    pub fn is_neutral(&self) -> bool {
        self.trample_w <= 0.0 && self.flip_chance <= 0.0
    }

    /// The overlay that reproduces pre-succession behaviour. The control for
    /// tests and sweeps.
    pub const fn neutral(&mut self) {
        self.trample_w = 0.0;
        self.flip_chance = 0.0;
    }

    /// Reject values the mechanic cannot run with.
    pub fn validate(&self) -> Result<(), String> {
        for (name, v) in [
            ("succession.trample_w", self.trample_w),
            ("succession.climb_veg", self.climb_veg),
            ("succession.wear_veg", self.wear_veg),
            ("succession.trample_low", self.trample_low),
            ("succession.trample_high", self.trample_high),
            ("succession.flip_chance", self.flip_chance),
        ] {
            if !(0.0..=1.0).contains(&v) {
                return Err(format!("{name} = {v} must be within 0..=1"));
            }
        }
        if self.climb_veg <= self.wear_veg {
            return Err(format!("succession.climb_veg = {} must exceed wear_veg = {}", self.climb_veg, self.wear_veg));
        }
        if self.trample_high < self.trample_low {
            return Err(format!("succession.trample_high = {} must be at least trample_low = {}", self.trample_high, self.trample_low));
        }
        for rung in RUNGS_ABOVE_DIRT {
            if !self.climb_days.contains_key(&rung) {
                return Err(format!("succession.climb_days has no entry for {}", rung.name()));
            }
            match self.climb_moisture.get(&rung) {
                None => return Err(format!("succession.climb_moisture has no entry for {}", rung.name())),
                Some(m) if !(0.0..=1.0).contains(m) => return Err(format!("succession.climb_moisture for {} = {m} must be within 0..=1", rung.name())),
                Some(_) => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn trample_factor_is_one_at_zero_pressure() {
        let s = SuccessionParams::default();
        assert_eq!(s.trample_factor(0.0), 1.0);
        assert!((s.trample_factor(0.5) - 0.875).abs() < 1e-6);
        assert!((s.trample_factor(1.0) - 0.75).abs() < 1e-6);
        let heavy = SuccessionParams { trample_w: 1.0, ..SuccessionParams::default() };
        assert_eq!(heavy.trample_factor(1.0), 0.0, "clamped at zero");
    }

    #[test]
    fn neutral_overlay_is_detected() {
        let mut s = SuccessionParams::default();
        assert!(!s.is_neutral());
        s.neutral();
        assert!(s.is_neutral());
        assert!(s.validate().is_ok());
        assert_eq!(s.trample_factor(1.0), 1.0, "neutral: the target is untouched");
    }

    #[test]
    fn climb_tables_cover_every_rung_above_dirt() {
        let s = SuccessionParams::default();
        assert!(s.validate().is_ok());
        let mut missing = SuccessionParams::default();
        missing.climb_days.remove(&Terrain::Forest);
        assert!(missing.validate().unwrap_err().contains("climb_days"));
        let mut missing = SuccessionParams::default();
        missing.climb_moisture.remove(&Terrain::Grass);
        assert!(missing.validate().unwrap_err().contains("climb_moisture"));
        let bad = SuccessionParams { climb_veg: 0.4, ..SuccessionParams::default() };
        assert!(bad.validate().unwrap_err().contains("climb_veg"));
        let bad = SuccessionParams { trample_high: 0.05, ..SuccessionParams::default() };
        assert!(bad.validate().unwrap_err().contains("trample_high"));
        let bad = SuccessionParams { flip_chance: 1.5, ..SuccessionParams::default() };
        assert!(bad.validate().unwrap_err().contains("flip_chance"));
    }
}
